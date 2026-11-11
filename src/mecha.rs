//! Transformaciones del Valkyrie: CAZA ↔ GERWALK ↔ BATTROID.
//! - Cerca del suelo (atmósfera, < 500 m sobre el terreno): las tres formas.
//! - En el espacio: CAZA y BATTROID.
//! - En el cielo (atmósfera alta): sólo CAZA; si subes transformado, se reconvierte solo.
//! También: Jefe Invid (enemigo pesado) y Colmena Invid (base terrestre).

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use rand::Rng;

use crate::audio::{play, Sounds};
use crate::components::*;
use crate::gameplay::{spawn_explosion, spawn_puff};
use crate::world::*;

/// Altura máxima sobre el terreno para las formas GERWALK / BATTROID en la atmósfera.
pub const LOW_ALT: f32 = 500.0;
const REVERT_ALT: f32 = 650.0;
const MORPH_TIME: f32 = 1.1;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    #[default]
    Fighter,
    Gerwalk,
    Battroid,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Fighter => "CAZA",
            Mode::Gerwalk => "GERWALK",
            Mode::Battroid => "BATTROID",
        }
    }
    /// Multiplicador del daño recibido (el robot está más blindado).
    pub fn armor(self) -> f32 {
        match self {
            Mode::Fighter => 1.0,
            Mode::Gerwalk => 0.85,
            Mode::Battroid => 0.65,
        }
    }
}

/// Estado de transformación del jugador.
#[derive(Resource, Default)]
pub struct Mecha {
    pub mode: Mode,
    pub pending: Option<Mode>,
    /// Segundos restantes de la animación de transformación.
    pub morph: f32,
    pub hint_shown: bool,
}

/// Formas disponibles según dónde está la nave.
pub fn allowed(zone: Zone, p: Vec3) -> Vec<Mode> {
    match zone {
        Zone::Space => vec![Mode::Fighter, Mode::Battroid],
        Zone::Atmosphere => {
            if p.y - ground_height(p.x, p.z) < LOW_ALT {
                vec![Mode::Fighter, Mode::Gerwalk, Mode::Battroid]
            } else {
                vec![Mode::Fighter]
            }
        }
    }
}

/// Modelo de cada forma (hijo de PlayerModel).
#[derive(Component)]
pub struct FormModel(pub Mode);

pub fn gerwalk_transform() -> Transform {
    // nariz hacia +Z en el archivo → la giramos; origen del jugador = cabina
    let s = 8.0;
    let r = Quat::from_rotation_y(std::f32::consts::PI);
    Transform { translation: -(r * (Vec3::new(0.0, 0.42, 0.15) * s)), rotation: r, scale: Vec3::splat(s) }
}
pub fn battroid_transform() -> Transform {
    // mira a -Z; origen del jugador = cabina en el pecho
    let s = 7.0;
    Transform { translation: -(Vec3::new(0.0, 0.35, 0.0) * s), rotation: Quat::IDENTITY, scale: Vec3::splat(s) }
}
/// Distancia de la cabina a los pies (para apoyarse en el suelo).
pub fn feet(mode: Mode) -> f32 {
    match mode {
        Mode::Fighter => 4.0,
        Mode::Gerwalk => (0.42 + 0.74) * 8.0,
        Mode::Battroid => (0.35 + 0.95) * 7.0,
    }
}

/// Tecla G: siguiente forma permitida. También reconvierte solo si la forma ya no está permitida.
pub fn mecha_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut mecha: ResMut<Mecha>,
    mut game: ResMut<Game>,
    mut dir: ResMut<Director>,
    mut commands: Commands,
    sounds: Res<Sounds>,
    a: Res<GameAssets>,
    player: Single<&Transform, With<Player>>,
) {
    let dt = time.delta_secs();
    let p = player.translation;
    if game.dead {
        mecha.mode = Mode::Fighter;
        mecha.pending = None;
        mecha.morph = 0.0;
        game.armor = 1.0;
        return;
    }
    let ok = allowed(game.zone, p);
    // aviso la primera vez que se puede transformar
    if ok.len() > 1 && !mecha.hint_shown && game.msg_timer <= 0.0 {
        mecha.hint_shown = true;
        game.message("Transformación disponible: pulsa G", 3.0);
    }
    let mut start = |mecha: &mut Mecha, to: Mode, game: &mut Game| {
        mecha.pending = Some(to);
        mecha.morph = MORPH_TIME;
        play(&mut commands, &sounds.whoosh, 0.5);
        play(&mut commands, &sounds.lock, 0.4);
        crate::sfx::sfx(&mut commands, crate::sfx::SfxKind::Transform, None, 1.0);
        crate::sfx::bark(&mut commands, match to {
            Mode::Gerwalk => "#p_gerwalk",
            Mode::Battroid => "#p_battroid",
            Mode::Fighter => "#p_fighter",
        });
        for k in 0..10 {
            let ang = k as f32 / 10.0 * std::f32::consts::TAU;
            spawn_puff(&mut commands, &a, p + Vec3::new(ang.cos(), 0.0, ang.sin()) * 9.0, 3.0, 0.9);
        }
        game.message(format!("TRANSFORMACIÓN: {}", to.name()), 2.0);
        if dir.auto && dir.cut.is_none() {
            dir.cut = Some((CamView::Wing, 1.8));
        }
    };
    if mecha.morph <= 0.0 {
        if !ok.contains(&mecha.mode) {
            let reason = if game.zone == Zone::Atmosphere && p.y - ground_height(p.x, p.z) > REVERT_ALT {
                "Altura excesiva"
            } else {
                "Forma no disponible"
            };
            start(&mut mecha, Mode::Fighter, &mut game);
            game.message(format!("{reason}: reconversión a CAZA"), 2.5);
        } else if keys.just_pressed(KeyCode::KeyG) {
            if ok.len() == 1 {
                game.message("En el cielo sólo es posible la forma CAZA (baja a menos de 500 m)", 2.5);
            } else {
                let i = ok.iter().position(|m| *m == mecha.mode).unwrap_or(0);
                let next = ok[(i + 1) % ok.len()];
                start(&mut mecha, next, &mut game);
            }
        }
    }
    if mecha.morph > 0.0 {
        let before = mecha.morph;
        mecha.morph -= dt;
        if before > MORPH_TIME * 0.5 && mecha.morph <= MORPH_TIME * 0.5 {
            if let Some(m) = mecha.pending.take() {
                mecha.mode = m;
            }
        }
    }
    game.armor = mecha.mode.armor();
}

/// Movimiento en GERWALK / BATTROID: puede quedarse suspendido (velocidad 0), desplazarse de
/// lado con A/D y, cerca del terreno, apoyarse en el suelo sin dañarse.
#[allow(clippy::too_many_arguments)]
pub fn mecha_flight(tf: &mut Transform, pl: &mut Player, mode: Mode, zone: Zone, pitch_in: f32, yaw_in: f32, side_in: f32, thr_pad: f32, keys: &ButtonInput<KeyCode>, dt: f32) {
    let k = |c: KeyCode| if keys.pressed(c) { 1.0 } else { 0.0 };
    let (vmax, vboost, turn, strafe) = match mode {
        Mode::Gerwalk => (190.0, 330.0, 1.5, 70.0),
        _ => (130.0, 240.0, 2.3, 110.0),
    };
    let target = Vec3::new(pitch_in * 1.3, yaw_in * turn * 0.55, 0.0);
    let s = 1.0 - (-8.0 * dt).exp();
    pl.ang_vel = pl.ang_vel.lerp(target, s);
    if zone == Zone::Atmosphere {
        // guiñada respecto a la vertical y alabeo que se nivela solo
        tf.rotate_y(pl.ang_vel.y * dt);
        tf.rotate_local_x(pl.ang_vel.x * dt);
        let f = tf.forward().as_vec3();
        let pitch = f.y.clamp(-1.0, 1.0).asin().clamp(-1.2, 1.2);
        let yaw = f.x.atan2(-f.z);
        let level = Quat::from_euler(EulerRot::YXZ, -yaw, pitch, side_in * -0.12);
        tf.rotation = tf.rotation.slerp(level, (4.0 * dt).min(1.0));
    } else {
        tf.rotate_local_x(pl.ang_vel.x * dt);
        tf.rotate_local_y(pl.ang_vel.y * dt);
    }
    pl.throttle = (pl.throttle + ((k(KeyCode::KeyW) - k(KeyCode::KeyS)) * 0.6 + thr_pad) * dt).clamp(0.0, 1.0);
    pl.boosting = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let target_speed = if pl.boosting { vboost } else { vmax * pl.throttle };
    pl.speed += (target_speed - pl.speed) * (1.0 - (-2.2 * dt).exp());
    let fwd = tf.forward().as_vec3();
    let right = tf.right().as_vec3();
    tf.translation += fwd * pl.speed * dt - right * side_in * strafe * dt;
}

/// Muestra el modelo de la forma actual con un "pulso" durante la transformación.
pub fn form_models(mecha: Res<Mecha>, mut q: Query<(&FormModel, &mut Visibility, &mut Transform)>) {
    let k = if mecha.morph > 0.0 {
        let t = 1.0 - mecha.morph / MORPH_TIME; // 0..1
        1.0 - 0.35 * (t * std::f32::consts::PI).sin()
    } else {
        1.0
    };
    for (f, mut v, mut tf) in &mut q {
        *v = if f.0 == mecha.mode { Visibility::Inherited } else { Visibility::Hidden };
        tf.scale = Vec3::splat(k);
    }
}

pub fn spawn_form_models(p: &mut ChildSpawnerCommands, gerwalk: &Handle<bevy::world_serialization::WorldAsset>, battroid: &Handle<bevy::world_serialization::WorldAsset>) {
    for (mode, h, tf) in [(Mode::Gerwalk, gerwalk, gerwalk_transform()), (Mode::Battroid, battroid, battroid_transform())] {
        p.spawn((Transform::default(), Visibility::Hidden, FormModel(mode))).with_children(|c| {
            c.spawn((WorldAssetRoot(h.clone()), tf));
        });
    }
}

// ---------------------------------------------------------------- Jefe Invid

/// Jefe Invid: más grande, más blindado, láseres dobles y ráfagas de plasma.
#[derive(Component)]
pub struct Boss {
    pub volley_cd: f32,
}

pub const BOSS_SCALE: f32 = 12.5;

pub fn boss_model_transform() -> Transform {
    Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)).with_scale(Vec3::splat(BOSS_SCALE))
}

pub fn spawn_boss(commands: &mut Commands, a: &GameAssets, pos: Vec3, look: Vec3, media: bool) -> Entity {
    let hp = if media { 900.0 } else { 600.0 };
    commands
        .spawn((
            Transform::from_translation(pos).looking_at(look, Vec3::Y),
            Visibility::default(),
            Enemy {
                kind: EnemyKind::Fighter,
                hp,
                max_hp: hp,
                radius: 15.0,
                half_extents: None,
                cd: 2.0,
                speed: 150.0,
                vel: Vec3::ZERO,
                evade: 0.0,
                evade_dir: Vec3::ZERO,
                score: 1500,
                hit_flash: 0.0,
            },
            Boss { volley_cd: 6.0 },
        ))
        .with_children(|p| {
            p.spawn((WorldAssetRoot(a.boss.clone()), boss_model_transform()));
        })
        .id()
}

/// Ráfagas de plasma guiado del Jefe (en ambas dificultades).
#[allow(clippy::too_many_arguments)]
pub fn boss_attacks(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    ma: Res<crate::missions::MissionAssets>,
    sounds: Res<Sounds>,
    diff: Res<crate::missions::Difficulty>,
    game: Res<Game>,
    player: Single<&Transform, With<Player>>,
    mut bosses: Query<(&Transform, &mut Boss, &Enemy), Without<Player>>,
) {
    if game.dead {
        return;
    }
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    for (tf, mut b, en) in &mut bosses {
        b.volley_cd -= dt;
        let to = player.translation - tf.translation;
        let d = to.length();
        if b.volley_cd <= 0.0 && d < 1800.0 && en.hp > 0.0 {
            b.volley_cd = if diff.media() { rng.gen_range(7.0..10.0) } else { rng.gen_range(10.0..14.0) };
            let n = if diff.media() { 3 } else { 2 };
            for i in 0..n {
                let side = tf.right().as_vec3() * (i as f32 - (n - 1) as f32 * 0.5) * 0.6;
                let dirv = (to.normalize() + side + Vec3::Y * 0.15).normalize();
                commands.spawn((
                    Mesh3d(a.plain_sphere.clone()),
                    MeshMaterial3d(ma.plasma.clone()),
                    Transform::from_translation(tf.translation + dirv * 14.0).with_scale(Vec3::splat(2.6)),
                    crate::missions::EnemyPlasma { vel: dirv * 230.0, life: 8.0, trail: 0.0 },
                    NotShadowCaster,
                    ZoneEntity,
                ));
            }
            play(&mut commands, &sounds.missile, 0.6);
            play(&mut commands, &sounds.alarm, 0.25);
        }
    }
}

// ---------------------------------------------------------------- Colmena Invid

/// Colmena Invid: base terrestre que lanza exploradores mientras exista.
#[derive(Component)]
pub struct Hive {
    pub spawn_cd: f32,
}

pub const HIVE_SCALE: f32 = 260.0;
pub const HIVE_XZ: Vec2 = Vec2::new(3600.0, -5000.0);

/// Posición del centro de la colmena (sobre el terreno).
pub fn hive_pos() -> Vec3 {
    let mut sum = 0.0;
    let mut n = 0.0;
    for x in -4..=4 {
        for z in -4..=4 {
            sum += ground_height(HIVE_XZ.x + x as f32 * 50.0, HIVE_XZ.y + z as f32 * 50.0);
            n += 1.0;
        }
    }
    Vec3::new(HIVE_XZ.x, sum / n - 15.0 + 0.68 * HIVE_SCALE, HIVE_XZ.y)
}

pub fn spawn_hive(commands: &mut Commands, a: &GameAssets, media: bool) {
    let pos = hive_pos();
    let hp = if media { 2600.0 } else { 1800.0 };
    commands
        .spawn((
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(0.8)),
            Visibility::default(),
            Enemy {
                kind: EnemyKind::Hive,
                hp,
                max_hp: hp,
                radius: 0.95 * HIVE_SCALE,
                half_extents: Some(Vec3::new(0.80, 0.66, 0.95) * HIVE_SCALE),
                cd: 3.0,
                speed: 0.0,
                vel: Vec3::ZERO,
                evade: 0.0,
                evade_dir: Vec3::ZERO,
                score: 3000,
                hit_flash: 0.0,
            },
            Hive { spawn_cd: 6.0 },
            ZoneEntity,
        ))
        .with_children(|p| {
            p.spawn((WorldAssetRoot(a.hive.clone()), Transform::from_scale(Vec3::splat(HIVE_SCALE))));
            // núcleo brillante dentro de la cúpula
            p.spawn((
                Mesh3d(a.plain_sphere.clone()),
                MeshMaterial3d(a.m_eye.clone()),
                Transform::from_xyz(-0.25 * HIVE_SCALE, -0.25 * HIVE_SCALE, 0.0).with_scale(Vec3::splat(0.12 * HIVE_SCALE)),
                NotShadowCaster,
            ));
        });
}

/// La colmena lanza exploradores cuando el jugador se acerca (o en la misión de la colmena).
#[allow(clippy::too_many_arguments)]
pub fn hive_spawner(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    sounds: Res<Sounds>,
    diff: Res<crate::missions::Difficulty>,
    mission: Res<crate::missions::Mission>,
    mut game: ResMut<Game>,
    player: Single<&Transform, With<Player>>,
    mut hives: Query<(&Transform, &mut Hive, &Enemy), Without<Player>>,
    fighters: Query<&Enemy, Without<Hive>>,
) {
    if game.dead || game.zone != Zone::Atmosphere {
        return;
    }
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    let alive = fighters.iter().filter(|e| e.kind == EnemyKind::Fighter).count();
    let hive_mission = mission.idx == 2 && mission.phase == crate::missions::Phase::Active;
    for (tf, mut h, en) in &mut hives {
        h.spawn_cd -= dt;
        let near = tf.translation.distance(player.translation) < 3500.0;
        let cap = if diff.media() { 7 } else { 4 };
        if h.spawn_cd <= 0.0 && (near || hive_mission) && alive < cap && en.hp > 0.0 {
            h.spawn_cd = if diff.media() { rng.gen_range(9.0..13.0) } else { rng.gen_range(13.0..18.0) };
            let n = if diff.media() { 2 } else { 1 };
            for _ in 0..n {
                let out = tf.translation + Vec3::new(rng.gen_range(-80.0..80.0), 0.75 * HIVE_SCALE, rng.gen_range(-80.0..80.0));
                spawn_fighter(&mut commands, &a, out, player.translation, game.wave.max(1));
                spawn_puff(&mut commands, &a, out, 12.0, 1.2);
            }
            play(&mut commands, &sounds.flak, 0.4);
            if game.msg_timer <= 0.0 {
                game.message("La colmena Invid lanza exploradores", 2.0);
            }
        }
    }
}

/// Explosión en cadena al destruir la colmena.
pub fn hive_destroyed(commands: &mut Commands, a: &GameAssets, sounds: &Sounds, tf: &Transform) {
    let mut rng = rand::thread_rng();
    for _ in 0..14 {
        let o = Vec3::new(rng.gen_range(-0.7..0.7), rng.gen_range(-0.5..0.6), rng.gen_range(-0.8..0.8)) * HIVE_SCALE;
        spawn_explosion(commands, a, tf.translation + tf.rotation * o, rng.gen_range(40.0..90.0), Vec3::ZERO);
    }
    play(commands, &sounds.big_explosion, 1.0);
    play(commands, &sounds.kick, 1.0);
}

// ---------------------------------------------------------------- Soldado de Choque Invid

/// Nuevo jefe: Soldado de Choque Invid. Más blindado que el Jefe, dispara lluvias de plasma
/// desde los hombros y hace embestidas a gran velocidad.
#[derive(Component)]
pub struct ShockTrooper {
    pub charge_cd: f32,
    pub charging: f32,
    pub rain_cd: f32,
    pub base_speed: f32,
}

pub const SHOCK_SCALE: f32 = 15.0;
/// Giro del modelo para que mire hacia -Z.
pub const SHOCK_YAW: f32 = std::f32::consts::PI;

#[derive(Component)]
pub struct ShockAura;

pub fn spawn_shock(commands: &mut Commands, a: &GameAssets, pos: Vec3, look: Vec3, media: bool) -> Entity {
    let hp = if media { 1400.0 } else { 950.0 };
    commands
        .spawn((
            Transform::from_translation(pos).looking_at(look, Vec3::Y),
            Visibility::default(),
            Enemy {
                kind: EnemyKind::Fighter,
                hp,
                max_hp: hp,
                radius: 18.0,
                half_extents: None,
                cd: 1.5,
                speed: 165.0,
                vel: Vec3::ZERO,
                evade: 0.0,
                evade_dir: Vec3::ZERO,
                score: 2500,
                hit_flash: 0.0,
            },
            Boss { volley_cd: 9.0 },
            ShockTrooper { charge_cd: 8.0, charging: 0.0, rain_cd: 5.0, base_speed: 165.0 },
        ))
        .with_children(|p| {
            p.spawn((
                WorldAssetRoot(a.shock.clone()),
                Transform::from_rotation(Quat::from_rotation_y(SHOCK_YAW)).with_scale(Vec3::splat(SHOCK_SCALE)),
            ));
            // aura roja que se enciende durante la embestida
            p.spawn((Mesh3d(a.plain_sphere.clone()), MeshMaterial3d(a.m_laser_enemy.clone()), Transform::from_xyz(0.0, 0.0, -6.0).with_scale(Vec3::new(9.0, 9.0, 16.0)), Visibility::Hidden, ShockAura, NotShadowCaster));
        })
        .id()
}

/// Embestidas y lluvia de plasma del Soldado de Choque.
#[allow(clippy::too_many_arguments)]
pub fn shock_attacks(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    ma: Res<crate::missions::MissionAssets>,
    sounds: Res<Sounds>,
    diff: Res<crate::missions::Difficulty>,
    mut game: ResMut<Game>,
    player: Single<&Transform, With<Player>>,
    mut q: Query<(&Transform, &mut ShockTrooper, &mut Enemy, &Children), Without<Player>>,
    mut auras: Query<&mut Visibility, With<ShockAura>>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    for (tf, mut st, mut en, children) in &mut q {
        st.charge_cd -= dt;
        st.rain_cd -= dt;
        let to = player.translation - tf.translation;
        let d = to.length();
        if st.charging > 0.0 {
            st.charging -= dt;
            en.speed = 560.0;
            if st.charging <= 0.0 {
                en.speed = st.base_speed;
            }
        } else if st.charge_cd <= 0.0 && d < 1600.0 && d > 250.0 && !game.dead {
            st.charge_cd = if diff.media() { rng.gen_range(9.0..13.0) } else { rng.gen_range(13.0..18.0) };
            st.charging = 1.8;
            play(&mut commands, &sounds.alarm, 0.45);
            play(&mut commands, &sounds.whoosh, 0.7);
            game.message("¡EL SOLDADO DE CHOQUE EMBISTE! ¡Rompe el rumbo!", 2.0);
        }
        for c in children.iter() {
            if let Ok(mut v) = auras.get_mut(c) {
                *v = if st.charging > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
            }
        }
        // lluvia de plasma desde los hombros: abanico de proyectiles pequeños
        if st.rain_cd <= 0.0 && d < 1400.0 && !game.dead {
            st.rain_cd = if diff.media() { rng.gen_range(6.0..8.0) } else { rng.gen_range(8.0..11.0) };
            let n = if diff.media() { 7 } else { 5 };
            for i in 0..n {
                let k = i as f32 / (n - 1) as f32 - 0.5;
                let dirv = (to.normalize() + tf.right().as_vec3() * k * 0.9 + Vec3::Y * rng.gen_range(0.05..0.35)).normalize();
                commands.spawn((
                    Mesh3d(a.plain_sphere.clone()),
                    MeshMaterial3d(ma.plasma.clone()),
                    Transform::from_translation(tf.translation + Vec3::Y * 10.0 + dirv * 12.0).with_scale(Vec3::splat(1.6)),
                    crate::missions::EnemyPlasma { vel: dirv * 280.0, life: 6.0, trail: 0.0 },
                    NotShadowCaster,
                    ZoneEntity,
                ));
            }
            play(&mut commands, &sounds.missile, 0.5);
        }
    }
}

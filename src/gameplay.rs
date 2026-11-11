use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rand::Rng;

use crate::audio::{play, EngineSound, Sounds};
use crate::components::*;
use crate::world::*;

pub const LASER_SPEED: f32 = 1500.0;

// ---------------------------------------------------------------- jugador + cabina

/// Tonemapping: AgX suaviza las luces fuertes de los modelos PBR sin apagar los colores.
fn tonemap_choice() -> bevy::core_pipeline::tonemapping::Tonemapping {
    use bevy::core_pipeline::tonemapping::Tonemapping;
    match std::env::var("NAVE_TONEMAP").as_deref() {
        Ok("none") => Tonemapping::None,
        Ok("tony") => Tonemapping::TonyMcMapface,
        Ok("aces") => Tonemapping::AcesFitted,
        _ => Tonemapping::None,
    }
}

pub fn setup_player(
    mut commands: Commands,
    a: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    models: Res<crate::models::Models>,
) {
    use bevy::camera::Hdr;
    use bevy::post_process::bloom::Bloom;

    // Cámara independiente: el director la coloca cada fotograma.
    commands.spawn((
        WorldCam,
        Camera3d::default(),
        Hdr,
        tonemap_choice(),
        bevy::core_pipeline::tonemapping::DebandDither::Disabled,
        Bloom { intensity: 0.22, ..Bloom::NATURAL },
        Projection::from(PerspectiveProjection {
            fov: 74.0_f32.to_radians(),
            near: 0.03,
            far: 160000.0,
            ..default()
        }),
        DistanceFog {
            color: Color::srgb(0.95, 0.58, 0.52),
            directional_light_color: Color::NONE,
            directional_light_exponent: 8.0,
            falloff: FogFalloff::Linear { start: 2500.0, end: 13000.0 },
        },
        Transform::default(),
        bevy::camera::visibility::RenderLayers::from_layers(&[0, 1]),
    ));
    let mut screens = None;
    let toon = a.toon.clone();
    commands
        .spawn((
            Transform::from_xyz(0.0, 700.0, 5200.0),
            Visibility::default(),
            Player {
                speed: 220.0,
                throttle: 0.55,
                ang_vel: Vec3::ZERO,
                boosting: false,
                input: Vec3::ZERO,
            },
        ))
        .with_children(|p| {
            spawn_player_model(p, &a, &a.valk_flight);
            p.spawn((CockpitRoot, Transform::default(), Visibility::default())).with_children(|c| {
                screens = Some(crate::cockpit::build_cockpit(c, &toon, &mut meshes, &mut mats, &mut images, &models.pilot));
            });
        });
    let screens = screens.unwrap();
    // cámara de puntería (renderiza a la textura de la pantalla izquierda)
    commands.spawn((
        crate::cockpit::TargetCam,
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.05, 0.06, 0.08)),
            is_active: false,
            ..default()
        },
        bevy::camera::RenderTarget::Image(screens.tgt_cam.clone().into()),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        bevy::core_pipeline::tonemapping::DebandDither::Disabled,
        Projection::from(PerspectiveProjection { fov: 20f32.to_radians(), near: 0.5, far: 160000.0, ..default() }),
        DistanceFog {
            color: Color::srgb(0.95, 0.58, 0.52),
            directional_light_color: Color::NONE,
            directional_light_exponent: 8.0,
            falloff: FogFalloff::Linear { start: 2500.0, end: 13000.0 },
        },
        Transform::default(),
    ));
    commands.insert_resource(screens);
    commands.spawn((
        Mesh3d(a.stars.clone()),
        MeshMaterial3d(a.m_stars.clone()),
        Transform::default(),
        Visibility::Hidden,
        StarField,
        NotShadowCaster,
    ));
    spawn_atmosphere(&mut commands, &a, true);
}

// ---------------------------------------------------------------- utilidades

pub fn seg_hit(a: Vec3, b: Vec3, c: Vec3, r: f32) -> bool {
    let ab = b - a;
    let t = ((c - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t).distance_squared(c) < r * r
}

pub fn box_contains(tf: &Transform, half: Vec3, p: Vec3, pad: f32) -> bool {
    let l = tf.rotation.inverse() * (p - tf.translation);
    l.x.abs() < half.x + pad && l.y.abs() < half.y + pad && l.z.abs() < half.z + pad
}

pub fn enemy_hit(tf: &Transform, e: &Enemy, a: Vec3, b: Vec3, pad: f32) -> bool {
    match e.half_extents {
        Some(h) => (0..=8).any(|i| box_contains(tf, h, a.lerp(b, i as f32 / 8.0), pad)),
        None => seg_hit(a, b, tf.translation, e.radius + pad),
    }
}

pub fn spawn_bullet(commands: &mut Commands, a: &GameAssets, pos: Vec3, vel: Vec3, dmg: f32, from_player: bool, flak: bool) {
    let dir = vel.normalize_or(Vec3::NEG_Z);
    let mat = if from_player {
        a.m_laser_player.clone()
    } else if flak {
        a.m_flak.clone()
    } else {
        a.m_laser_enemy.clone()
    };
    let scale = if flak { Vec3::new(3.0, 3.0, 0.25) } else if from_player { Vec3::ONE } else { Vec3::new(1.3, 1.3, 1.8) };
    commands.spawn((
        Mesh3d(a.laser.clone()),
        MeshMaterial3d(mat),
        Transform::from_translation(pos)
            .with_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, dir))
            .with_scale(scale),
        Bullet {
            vel,
            prev: pos,
            life: if flak { 3.5 } else { 2.2 },
            damage: dmg,
            from_player,
        },
        NotShadowCaster,
    ));
}

/// Bocanada de humo blanca con contorno de tinta (estilo anime).
pub fn spawn_puff(commands: &mut Commands, a: &GameAssets, pos: Vec3, size: f32, life: f32) {
    commands
        .spawn((
            Mesh3d(a.toon.sphere.clone()),
            MeshMaterial3d(a.m_smoke.clone()),
            Transform::from_translation(pos).with_scale(Vec3::splat(0.05)),
            Effect { kind: EffectKind::Smoke { size }, t: 0.0, life },
            NotShadowCaster,
        ))
        .with_children(|c| {
            c.spawn((Mesh3d(a.toon.sphere.clone()), MeshMaterial3d(a.toon.ink.clone()), Transform::from_scale(Vec3::splat(1.10)), NotShadowCaster));
        });
}

/// Explosión por capas (la construye `fx::booms`, que también lanza su sonido posicional).
pub fn spawn_explosion(commands: &mut Commands, _a: &GameAssets, pos: Vec3, size: f32, vel: Vec3) {
    commands.spawn(crate::fx::Boom { pos, size, vel });
}

// ---------------------------------------------------------------- entrada general

pub fn input_misc(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut game: ResMut<Game>,
    mut cursor: Single<&mut bevy::window::CursorOptions>,
    mut invert: ResMut<InvertY>,
) {
    use bevy::window::CursorGrabMode;
    let want_play = mouse.just_pressed(MouseButton::Left) || keys.just_pressed(KeyCode::Enter);
    if game.started && game.paused && want_play {
        game.started = true;
        game.paused = false;
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
        if game.show_help {
            game.message("¡Despega, piloto! Elimina a los cazas enemigos", 4.0);
        }
        return;
    }
    if keys.just_pressed(KeyCode::Escape) && game.started {
        game.paused = !game.paused;
        let free = game.paused;
        cursor.grab_mode = if free { CursorGrabMode::None } else { CursorGrabMode::Locked };
        cursor.visible = free;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        game.show_help = !game.show_help;
    }
    if keys.just_pressed(KeyCode::KeyI) {
        invert.0 = !invert.0;
        let s = if invert.0 { "Eje Y invertido: SÍ" } else { "Eje Y invertido: NO" };
        game.message(s, 2.0);
    }
    if game.dead && keys.just_pressed(KeyCode::KeyR) && game.pending_zone.is_none() {
        game.pending_zone = Some(Zone::Atmosphere);
        game.reset_on_switch = true;
        game.fade_dir = 1.0;
    }
}

#[derive(Resource, Default)]
pub struct InvertY(pub bool);

/// Solo para pruebas automáticas: apunta y dispara solo.
#[derive(Resource, Default)]
pub struct AutoPilot(pub bool);

pub fn playing(game: Res<Game>) -> bool {
    game.started && !game.paused
}

// ---------------------------------------------------------------- vuelo

pub fn player_flight(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    invert: Res<InvertY>,
    mut game: ResMut<Game>,
    mut q: Single<(&mut Transform, &mut Player)>,
    mut commands: Commands,
    sounds: Res<Sounds>,
    auto: Res<AutoPilot>,
    enemies: Query<(&Transform, &Enemy), Without<Player>>,
    mecha: Res<crate::mecha::Mecha>,
    pad: Res<crate::mando::Mando>,
) {
    let dt = time.delta_secs();
    let (ref mut tf, ref mut pl) = *q;
    let was_boosting = pl.boosting;
    if game.dead {
        // caída libre girando
        let f = tf.forward();
        tf.translation += f * pl.speed * 0.5 * dt;
        if game.zone == Zone::Atmosphere {
            tf.translation.y -= 60.0 * dt;
        }
        tf.rotate_local_z(1.5 * dt);
        return;
    }
    let k = |c: KeyCode| if keys.pressed(c) { 1.0 } else { 0.0 };
    let md = motion.delta;
    let inv = if invert.0 { -1.0 } else { 1.0 };
    let pitch_in = ((-md.y * 0.085 * inv).clamp(-1.0, 1.0) + k(KeyCode::ArrowUp) - k(KeyCode::ArrowDown) + pad.pitch).clamp(-1.0, 1.0);
    let yaw_in = ((-md.x * 0.085).clamp(-1.0, 1.0) + k(KeyCode::ArrowLeft) - k(KeyCode::ArrowRight) + k(KeyCode::KeyQ) - k(KeyCode::KeyE) + pad.yaw).clamp(-1.0, 1.0);
    let roll_in = (k(KeyCode::KeyA) - k(KeyCode::KeyD) + pad.roll).clamp(-1.0, 1.0);
    let thr_pad = (pad.acel - pad.freno) * 0.8;
    pl.input = Vec3::new(pitch_in, yaw_in, roll_in);
    if auto.0 {
        // piloto automático de prueba: girar hacia el enemigo más cercano
        if let Some(t) = enemies.iter().filter(|e| e.1.kind == EnemyKind::Fighter).map(|e| e.0.translation).min_by(|a, b| a.distance(tf.translation).total_cmp(&b.distance(tf.translation))) {
            let desired = (t - tf.translation).normalize_or(Vec3::NEG_Z);
            let rot = Transform::IDENTITY.looking_to(desired, Vec3::Y).rotation;
            tf.rotation = tf.rotation.slerp(rot, (2.0 * dt).min(1.0));
        }
    }
    let mecha_mode = if mecha.morph > 0.0 { mecha.pending.unwrap_or(mecha.mode) } else { mecha.mode };
    if mecha_mode != crate::mecha::Mode::Fighter || mecha.morph > 0.0 {
        let m = if mecha_mode == crate::mecha::Mode::Fighter { mecha.mode } else { mecha_mode };
        crate::mecha::mecha_flight(tf, pl, if m == crate::mecha::Mode::Fighter { crate::mecha::Mode::Gerwalk } else { m }, game.zone, pitch_in, yaw_in, roll_in, thr_pad, &keys, dt);
    } else {
    let target = Vec3::new(pitch_in * 1.5, yaw_in * 0.95, roll_in * 2.6);
    let s = 1.0 - (-8.0 * dt).exp();
    pl.ang_vel = pl.ang_vel.lerp(target, s);
    tf.rotate_local_x(pl.ang_vel.x * dt);
    tf.rotate_local_y(pl.ang_vel.y * dt);
    tf.rotate_local_z(pl.ang_vel.z * dt);

    if game.zone == Zone::Atmosphere {
        // viraje coordinado: al inclinarse la nave también gira
        let r = tf.right();
        tf.rotate_y(r.y * 0.75 * dt);
    }

    pl.throttle = (pl.throttle + ((k(KeyCode::KeyW) - k(KeyCode::KeyS)) * 0.6 + thr_pad) * dt).clamp(0.0, 1.0);
    pl.boosting = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let (min, max) = if game.zone == Zone::Atmosphere { (70.0, 340.0) } else { (25.0, 360.0) };
    let mut target_speed = min + (max - min) * pl.throttle;
    if pl.boosting {
        target_speed = 600.0;
    }
    let fwd = tf.forward();
    if game.zone == Zone::Atmosphere {
        target_speed -= fwd.y * 90.0; // subir cuesta, bajar acelera
    }
    let accel = if pl.boosting { 1.4 } else { 0.8 };
    pl.speed += (target_speed - pl.speed) * (1.0 - (-accel * dt).exp());
    tf.translation += fwd * pl.speed * dt;
    }

    // grito al encender el turbo
    if pl.boosting && !was_boosting && rand::thread_rng().gen_bool(0.2) {
        crate::sfx::bark(&mut commands, "#p_turbo");
    }

    // Límites de la zona
    let p = tf.translation;
    let center_dir = match game.zone {
        Zone::Atmosphere if p.x.abs() > ATMOS_LIMIT || p.z.abs() > ATMOS_LIMIT => Some(Vec3::new(-p.x, 0.0, -p.z).normalize()),
        Zone::Space if p.length() > SPACE_LIMIT => Some((-p).normalize()),
        _ => None,
    };
    if let Some(d) = center_dir {
        let target_rot = Transform::IDENTITY.looking_to(d, Vec3::Y).rotation;
        tf.rotation = tf.rotation.slerp(target_rot, (0.9 * dt).min(1.0));
        if game.msg_timer <= 0.0 {
            game.message("REGRESANDO A LA ZONA DE COMBATE", 2.0);
        }
    }

    // Suelo
    if game.zone == Zone::Atmosphere && mecha.mode != crate::mecha::Mode::Fighter {
        // GERWALK / BATTROID: se apoyan en el suelo (golpe sólo si bajan muy rápido)
        let g = ground_height(tf.translation.x, tf.translation.z);
        let floor = g + crate::mecha::feet(mecha.mode);
        if tf.translation.y < floor {
            let fast = pl.speed > 200.0 && tf.forward().y < -0.5;
            tf.translation.y = floor;
            if fast {
                game.damage(10.0);
                play(&mut commands, &sounds.hit, 0.6);
                game.message("¡Aterrizaje forzoso!", 1.5);
            }
        }
    } else if game.zone == Zone::Atmosphere {
        let g = ground_height(tf.translation.x, tf.translation.z);
        if tf.translation.y < g + 4.0 {
            let dmg = if pl.speed > 120.0 { 34.0 } else { 12.0 };
            game.damage(dmg);
            tf.translation.y = g + 10.0;
            let f = tf.forward();
            let flat = Vec3::new(f.x, 0.0, f.z).normalize_or(Vec3::NEG_Z);
            let up_dir = (flat + Vec3::Y * 0.5).normalize();
            tf.rotation = Transform::IDENTITY.looking_to(up_dir, Vec3::Y).rotation;
            pl.speed *= 0.6;
            play(&mut commands, &sounds.hit, 0.9);
            play(&mut commands, &sounds.cymbal, 0.27);
            game.message("¡IMPACTO CONTRA EL TERRENO!", 2.0);
        }
    }
}

// ---------------------------------------------------------------- armas

pub fn player_weapons(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut game: ResMut<Game>,
    player: Single<(Entity, &Transform, &Player)>,
    enemies: Query<(Entity, &Transform, &Enemy), Without<Player>>,
    mut commands: Commands,
    a: Res<GameAssets>,
    sounds: Res<Sounds>,
    auto: Res<AutoPilot>,
    diff: Res<crate::missions::Difficulty>,
    mecha: Res<crate::mecha::Mecha>,
    mut cluster: ResMut<crate::fx::ClusterQueue>,
) {
    let dt = time.delta_secs();
    let (pe, tf, pl) = *player;
    game.gun_cd -= dt;
    game.missile_cd -= dt;
    game.heat = (game.heat - 0.42 * dt).max(0.0);
    if game.overheated && game.heat < 0.3 {
        game.overheated = false;
    }
    if game.missiles < 8 {
        game.missile_reload += dt;
        if game.missile_reload > 10.0 {
            game.missile_reload = 0.0;
            game.missiles += 1;
        }
    }
    if game.cluster < 3 {
        game.cluster_reload += dt;
        if game.cluster_reload > 20.0 {
            game.cluster_reload = 0.0;
            game.cluster += 1;
        }
    }
    if game.dead {
        game.lock = None;
        return;
    }
    let fwd = tf.forward().as_vec3();

    // Fijación de blanco (lock-on)
    let mut best: Option<(Entity, f32)> = None;
    for (e, etf, en) in &enemies {
        let to = etf.translation - tf.translation;
        let d = to.length();
        if d > 4000.0 || en.hp <= 0.0 {
            continue;
        }
        let ang = fwd.angle_between(to);
        let lim = if matches!(en.kind, EnemyKind::Mothership | EnemyKind::Hive) { 0.35 } else { 0.24 };
        if ang < lim && best.is_none_or(|(_, b)| ang < b) {
            best = Some((e, ang));
        }
    }
    let new_lock = best.map(|b| b.0);
    if new_lock == game.lock && new_lock.is_some() {
        game.lock_time += dt;
    } else {
        game.lock = new_lock;
        game.lock_time = 0.0;
        game.locked = false;
    }
    if !game.locked && game.lock.is_some() && game.lock_time > diff.lock_time() {
        game.locked = true;
        game.ev_lock = true;
        if rand::thread_rng().gen_bool(0.25) {
            crate::sfx::bark(&mut commands, if rand::thread_rng().gen_bool(0.5) { "#p_caer" } else { "#p_escapa" });
        }
        play(&mut commands, &sounds.lock, 0.5);
    }

    // Láseres
    let firing = mouse.pressed(MouseButton::Left) || keys.pressed(KeyCode::Space) || (auto.0 && game.lock.is_some());
    if firing && game.gun_cd <= 0.0 && !game.overheated {
        let robot = mecha.mode != crate::mecha::Mode::Fighter;
        game.gun_cd = if robot { 0.06 } else { 0.075 };
        game.gun_side = !game.gun_side;
        game.heat += if robot { 0.022 } else { 0.03 };
        if game.heat >= 1.0 {
            game.overheated = true;
            game.message("¡LÁSERES SOBRECALENTADOS!", 1.5);
        }
        let side = if game.gun_side { 1.0 } else { -1.0 };
        let gun = if robot { tf.transform_point(Vec3::new(3.2, -1.5, -4.5)) } else { tf.transform_point(Vec3::new(side * 3.4, -1.7, -3.0)) };
        let aim = tf.translation + fwd * 800.0;
        let dir = (aim - gun).normalize();
        spawn_bullet(&mut commands, &a, gun, dir * LASER_SPEED + fwd * pl.speed, 12.0, true, false);
        play(&mut commands, &sounds.laser, 0.22);
        if game.gun_side {
            crate::sfx::sfx(&mut commands, crate::sfx::SfxKind::LaserLayer, None, 1.0);
        }
    }

    // Misiles
    let fire_missile = mouse.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::KeyF);
    if fire_missile && game.missile_cd <= 0.0 {
        if game.missiles > 0 {
            game.missiles -= 1;
            game.missile_cd = 0.5;
            let pos = tf.transform_point(Vec3::new(if game.gun_side { 2.0 } else { -2.0 }, -2.6, -2.0));
            let target = if game.locked { game.lock } else { None };
            commands.spawn((
                Mesh3d(a.toon.cube.clone()),
                MeshMaterial3d(a.m_missile.clone()),
                Transform::from_translation(pos).with_rotation(tf.rotation).with_scale(Vec3::new(0.5, 0.5, 4.0)),
                Missile::direct(target, fwd * (pl.speed + 120.0)),
                crate::fx::NeedsTrail { width: 1.2, life: 2.8 },
                NotShadowCaster,
            ));
            spawn_puff(&mut commands, &a, pos, 2.5, 1.0);
            play(&mut commands, &sounds.missile, 0.6);
            crate::sfx::sfx(&mut commands, crate::sfx::SfxKind::MissileLaunch, Some(pos), 1.0);
            if rand::thread_rng().gen_bool(0.4) {
                crate::sfx::bark(&mut commands, "#p_misil");
            }
            game.ev_missile = true;
        } else {
            game.message("SIN MISILES — recargando", 1.2);
        }
    }

    // Misiles en racimo (V / botón central): 12 micromisiles con trayectorias curvas
    // (pruebas: NAVE_CLUSTER=segundos dispara una salva automática en ese instante)
    let test_salvo = std::env::var("NAVE_CLUSTER").ok().and_then(|v| v.parse::<f32>().ok()).is_some_and(|t0| {
        let e = time.elapsed_secs();
        e >= t0 && e - dt < t0
    });
    let fire_cluster = mouse.just_pressed(MouseButton::Middle) || keys.just_pressed(KeyCode::KeyV) || test_salvo;
    if fire_cluster && game.missile_cd <= 0.0 {
        if game.cluster > 0 {
            game.cluster -= 1;
            game.missile_cd = 1.2;
            // blancos: el fijado primero y luego los Invid más centrados delante
            let mut cands: Vec<(Entity, f32)> = enemies
                .iter()
                .filter(|(_, _, en)| en.hp > 0.0)
                .filter_map(|(e, etf, _)| {
                    let to = etf.translation - tf.translation;
                    let d = to.length();
                    let ang = fwd.angle_between(to);
                    (d < 3200.0 && ang < 0.9).then_some((e, ang + d / 8000.0))
                })
                .collect();
            cands.sort_by(|x, y| x.1.total_cmp(&y.1));
            let mut targets: Vec<Entity> = Vec::new();
            if let Some(l) = game.lock {
                targets.extend([l, l]);
            }
            targets.extend(cands.iter().map(|c| c.0).filter(|e| Some(*e) != game.lock).take(4));
            cluster.salvo(pe, &targets, 12, 0.045);
            crate::sfx::sfx(&mut commands, crate::sfx::SfxKind::ClusterLaunch, None, 1.0);
            crate::sfx::bark(&mut commands, "#p_racimo");
            game.ev_cluster = true;
            let left = game.cluster;
            game.message(format!("¡MISILES EN RACIMO!  ×12  ·  salvas {left}/3"), 1.5);
        } else {
            game.message("SIN SALVAS DE RACIMO — recargando (o pasa por el anillo)", 1.4);
        }
    }
}

pub fn missiles(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    sounds: Res<Sounds>,
    game: Res<Game>,
    dir: Res<Director>,
    mut ms: Query<(Entity, &mut Transform, &mut Missile), Without<Enemy>>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy), Without<Missile>>,
) {
    let dt = time.delta_secs();
    let cam = dir.cam.translation;
    for (e, mut tf, mut m) in &mut ms {
        m.life -= dt;
        let mut tinfo = None;
        if let Some(t) = m.target {
            match enemies.get(t) {
                Ok((_, ttf, en)) if en.hp > 0.0 => tinfo = Some((ttf.translation, en.vel)),
                _ => m.target = None,
            }
        }
        // los micromisiles sin blanco buscan el Invid más cercano
        if m.cluster && m.target.is_none() && m.age > 0.3 {
            let p = tf.translation;
            let best = enemies
                .iter()
                .filter(|(_, _, en)| en.hp > 0.0)
                .map(|(ent, etf, en)| (ent, etf.translation, en.vel, etf.translation.distance(p)))
                .filter(|x| x.3 < 1600.0)
                .min_by(|x, y| x.3.total_cmp(&y.3));
            if let Some((ent, tp, tv, _)) = best {
                m.target = Some(ent);
                tinfo = Some((tp, tv));
            }
        }
        let (dir_v, speed) = if m.cluster {
            let pos = tf.translation;
            crate::fx::steer_cluster(&mut m, pos, tinfo, dt)
        } else {
            let speed = (m.vel.length() + 500.0 * dt).min(950.0);
            let mut dir_v = m.vel.normalize_or(Vec3::NEG_Z);
            if let Some((tp, tv)) = tinfo {
                let to = tp - tf.translation;
                let tt = to.length() / speed;
                let desired = (to + tv * tt).normalize_or(dir_v);
                let ang = dir_v.angle_between(desired);
                let max = 3.2 * dt;
                if ang > 1e-4 {
                    dir_v = dir_v.slerp(desired, (max / ang).min(1.0)).normalize();
                }
            }
            (dir_v, speed)
        };
        m.vel = dir_v * speed;
        let prev = tf.translation;
        tf.translation += m.vel * dt;
        tf.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, dir_v);
        // pasada cerca de la cámara
        if m.smoke == 0.0 && m.age > 0.4 && tf.translation.distance(cam) < 45.0 {
            m.smoke = 1.0;
            crate::sfx::sfx(&mut commands, crate::sfx::SfxKind::Flyby, Some(tf.translation), 1.0);
        }
        let mut boom = m.life <= 0.0;
        if game.zone == Zone::Atmosphere && tf.translation.y < ground_height(tf.translation.x, tf.translation.z) {
            boom = true;
        }
        let pad = if m.cluster { 6.0 } else { 4.0 };
        for (_, etf, mut en) in &mut enemies {
            if en.hp > 0.0 && enemy_hit(etf, &en, prev, tf.translation, pad) {
                if !(en.kind == EnemyKind::Mothership && game.mother_shield) {
                    en.hp -= m.damage;
                }
                en.hit_flash = 0.2;
                boom = true;
                break;
            }
        }
        if boom {
            if m.cluster {
                spawn_explosion(&mut commands, &a, tf.translation, 5.0, m.vel * 0.05);
            } else {
                spawn_explosion(&mut commands, &a, tf.translation, 14.0, Vec3::ZERO);
                play(&mut commands, &sounds.explosion, 0.45);
                play(&mut commands, &sounds.kick, 0.41);
            }
            commands.entity(e).try_despawn();
        }
    }
}

// ---------------------------------------------------------------- IA enemiga

pub fn enemy_ai(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    sounds: Res<Sounds>,
    mut game: ResMut<Game>,
    player: Single<(&Transform, &Player)>,
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy, Option<&AttackCarrier>, Option<&mut crate::maniobras::Maniobra>, Option<&crate::spartan::AsaltoTerrestre>), Without<Player>>,
    carrier: Query<&Transform, (With<CarrierShip>, Without<Player>, Without<Enemy>)>,
    allies: Query<(&Transform, &crate::allies::Ally), (Without<Player>, Without<Enemy>, Without<CarrierShip>)>,
    diff: Res<crate::missions::Difficulty>,
    elites: Query<(), With<crate::missions::Elite>>,
    bosses: Query<(), With<crate::mecha::Boss>>,
    missiles_q: Query<(&Transform, &Missile), (Without<Enemy>, Without<Player>, Without<crate::allies::Ally>)>,
    spartans: Query<(&Transform, &crate::spartan::Spartan), (Without<Enemy>, Without<Player>, Without<crate::allies::Ally>, Without<Missile>)>,
) {
    use crate::maniobras::{en_la_cola, Amenaza};
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    let aggr = if diff.0 == crate::missions::Level::Media { 1.2 } else { 0.75 };
    let missile_list: Vec<(Vec3, Option<Entity>)> = missiles_q.iter().map(|(t, m)| (t.translation, m.target)).collect();
    let spartan_list: Vec<Vec3> = spartans.iter().filter(|s| s.1.hp > 0.0).map(|s| s.0.translation + Vec3::Y * 9.0).collect();
    let (dmg_k, cd_k) = (diff.enemy_dmg(), diff.fire_cd());
    let ally_list: Vec<(Vec3, Vec3)> = allies.iter().map(|(t, a)| (t.translation, a.vel)).collect();
    let (ptf, pl) = *player;
    let ppos = ptf.translation;
    let pvel = ptf.forward() * pl.speed;
    let ctf = carrier.iter().next().copied();
    let mut alarm = false;
    for (ent, mut tf, mut e, attack_carrier, mut man, asalto) in &mut enemies {
        e.hit_flash -= dt;
        e.cd -= dt;
        if elites.get(ent).is_ok() {
            continue;
        }
        let to = ppos - tf.translation;
        let dist = to.length();
        match e.kind {
            EnemyKind::Fighter => {
                // objetivo: la nave nodriza terrestre, salvo que el jugador se acerque a pelear
                let pdist = dist;
                let (mut tpos, mut tvel, mut vs_carrier) = match (attack_carrier, ctf) {
                    (Some(ac), Some(c)) if pdist > 450.0 || game.dead => (c.transform_point(ac.aim), Vec3::ZERO, true),
                    _ => (ppos, pvel, false),
                };
                // los Invid de asalto terrestre atacan a los Spartan de su sector
                // (salvo que el jugador se les acerque)
                if let Some(asl) = asalto {
                    if pdist > 600.0 || game.dead {
                        let best = spartan_list.iter().min_by(|a, b| a.distance(tf.translation).total_cmp(&b.distance(tf.translation))).copied();
                        let (tp, tv) = match best {
                            Some(sp) => (sp, Vec3::ZERO),
                            None => (asl.centro + Vec3::Y * 60.0, Vec3::ZERO),
                        };
                        tpos = tp;
                        tvel = tv;
                        vs_carrier = true;
                    }
                }
                // también pelean contra el Escuadrón Azul si un aliado está más cerca
                if !vs_carrier {
                    let mut best = if game.dead { f32::MAX } else { pdist };
                    for (ap, av) in &ally_list {
                        let d = ap.distance(tf.translation);
                        if d < best * 0.8 {
                            best = d;
                            tpos = *ap;
                            tvel = *av;
                        }
                    }
                }
                let to = tpos - tf.translation;
                let dist = to.length();
                e.evade -= dt;
                let evade_at = if vs_carrier { 260.0 } else { 160.0 };
                if dist < evade_at && e.evade <= 0.0 {
                    e.evade = rng.gen_range(1.5..3.0);
                    let r = Vec3::new(rng.gen_range(-1.0..1.0), rng.gen_range(-0.5..1.0), rng.gen_range(-1.0..1.0));
                    e.evade_dir = (r - to.normalize() * 0.6).normalize_or(Vec3::Y);
                }
                let mut desired = if e.evade > 0.0 {
                    e.evade_dir
                } else {
                    (to + tvel * (dist / 900.0)).normalize_or(Vec3::NEG_Z)
                };
                if game.zone == Zone::Atmosphere {
                    let g = ground_height(tf.translation.x, tf.translation.z);
                    let clr = tf.translation.y - g;
                    if clr < 220.0 {
                        desired = (desired + Vec3::Y * ((220.0 - clr) / 60.0)).normalize();
                    }
                    if tf.translation.y > CEILING - 300.0 {
                        desired = (desired - Vec3::Y * 0.8).normalize();
                    }
                }
                // maniobras evasivas (Top Gun): alguien a la cola, un misil detrás o cruce de frente
                let mut guia = None;
                if let Some(m) = man.as_mut() {
                    if !m.activa() && m.cd <= 0.0 {
                        let f = tf.forward().as_vec3();
                        let mut am = None;
                        if !game.dead && en_la_cola(tf.translation, f, ppos, ptf.forward().as_vec3(), 950.0) {
                            am = Some(Amenaza { desde: ppos, misil: false });
                        }
                        for (ap, av) in &ally_list {
                            if am.is_none() && en_la_cola(tf.translation, f, *ap, av.normalize_or(Vec3::Z), 800.0) {
                                am = Some(Amenaza { desde: *ap, misil: false });
                            }
                        }
                        for (mp, mt) in &missile_list {
                            if *mt == Some(ent) && mp.distance(tf.translation) < 600.0 {
                                am = Some(Amenaza { desde: *mp, misil: true });
                            }
                        }
                        if am.is_none() && dist < 150.0 {
                            am = Some(Amenaza { desde: tpos, misil: false });
                        }
                        // tiempo de reacción variable
                        if let Some(am) = am {
                            if rng.gen_bool(((5.0 * aggr) * dt).min(1.0) as f64) {
                                let alt = if game.zone == Zone::Atmosphere { tf.translation.y - ground_height(tf.translation.x, tf.translation.z) } else { 1e5 };
                                m.empezar(&tf, am, alt, &mut rng, aggr);
                            }
                        }
                    }
                    guia = m.guiar(&tf, dt);
                }
                let fwd = tf.forward().as_vec3();
                let mut speed_k = 1.0;
                if let Some((d, u)) = guia {
                    // en maniobra: la evasión manda, con alabeo marcado y algo más de velocidad
                    let mut d = d;
                    if game.zone == Zone::Atmosphere {
                        let clr = tf.translation.y - ground_height(tf.translation.x, tf.translation.z);
                        if clr < 160.0 {
                            d = (d + Vec3::Y * ((160.0 - clr) / 40.0)).normalize();
                        }
                    }
                    let target_rot = Transform::IDENTITY.looking_to(d, u).rotation;
                    tf.rotation = tf.rotation.slerp(target_rot, (3.4 * dt).min(1.0));
                    speed_k = 1.18;
                } else {
                    let up = if desired.y.abs() > 0.98 { Vec3::Z } else { Vec3::Y };
                    let target_rot = Transform::IDENTITY.looking_to(desired, up).rotation;
                    tf.rotation = tf.rotation.slerp(target_rot, (1.7 * dt).min(1.0));
                    let turn = fwd.cross(desired).y;
                    tf.rotate_local_z(turn.clamp(-1.0, 1.0) * 3.0 * dt);
                }
                let fwd = tf.forward().as_vec3();
                e.vel = fwd * e.speed * speed_k;
                tf.translation += e.vel * dt;
                if game.zone == Zone::Atmosphere {
                    let g = ground_height(tf.translation.x, tf.translation.z);
                    tf.translation.y = tf.translation.y.max(g + 30.0);
                }
                let ang = fwd.angle_between(to);
                if (!game.dead || vs_carrier) && ang < 0.13 && dist < 1200.0 && e.cd <= 0.0 {
                    e.cd = rng.gen_range(0.45..1.1) * cd_k;
                    for s in [-1.0f32, 1.0] {
                        let gun = tf.transform_point(Vec3::new(s * 3.0, 0.0, -5.0));
                        let dir = (tpos + tvel * (dist / 900.0) - gun).normalize();
                        let spread = Vec3::new(rng.gen_range(-0.02..0.02), rng.gen_range(-0.02..0.02), rng.gen_range(-0.02..0.02));
                        let boss_k = if bosses.get(ent).is_ok() { 1.8 } else { 1.0 };
                        spawn_bullet(&mut commands, &a, gun, (dir + spread).normalize() * 900.0 + e.vel * 0.3, 6.0 * dmg_k * boss_k, false, false);
                    }
                    play(&mut commands, &sounds.enemy_laser, (0.35 * (1.0 - pdist / 1400.0)).max(0.03));
                }
                if !vs_carrier && tpos == ppos && dist < 900.0 && ang < 0.3 {
                    alarm = true;
                }
            }
            EnemyKind::Turret => {
                let range = if game.zone == Zone::Atmosphere { 2300.0 } else { 2600.0 };
                if dist < range && !game.dead {
                    let flak_speed = 600.0;
                    let aim = (to + pvel * (dist / flak_speed)).normalize_or(Vec3::NEG_Z);
                    let up = if aim.y.abs() > 0.98 { Vec3::Z } else { Vec3::Y };
                    let target_rot = Transform::IDENTITY.looking_to(aim, up).rotation;
                    tf.rotation = tf.rotation.slerp(target_rot, (2.5 * dt).min(1.0));
                    if e.cd <= 0.0 && tf.forward().angle_between(aim) < 0.1 {
                        e.cd = rng.gen_range(1.1..2.2) * cd_k;
                        let spread = Vec3::new(rng.gen_range(-0.03..0.03), rng.gen_range(-0.03..0.03), rng.gen_range(-0.03..0.03));
                        let muzzle = tf.transform_point(Vec3::new(0.0, 0.5, -9.0));
                        spawn_bullet(&mut commands, &a, muzzle, (aim + spread).normalize() * flak_speed, 8.0 * dmg_k, false, true);
                        play(&mut commands, &sounds.flak, (0.3 * (1.0 - dist / range)).max(0.04));
                    }
                }
            }
            EnemyKind::Mothership | EnemyKind::Hive => {
                e.vel = Vec3::ZERO;
            }
        }
    }
    game.alarm_cd -= dt;
    if alarm && game.alarm_cd <= 0.0 && !game.dead {
        game.alarm_cd = 2.5;
        play(&mut commands, &sounds.alarm, 0.25);
    }
}

// ---------------------------------------------------------------- proyectiles y colisiones

pub fn bullets(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    sounds: Res<Sounds>,
    mut game: ResMut<Game>,
    mut bq: Query<(Entity, &mut Transform, &mut Bullet), (Without<Enemy>, Without<Player>, Without<Rock>)>,
    mut enemies: Query<(&Transform, &mut Enemy), (Without<Player>, Without<Bullet>)>,
    rocks: Query<(&Transform, &Rock), (Without<Bullet>, Without<Enemy>)>,
    player: Single<&Transform, With<Player>>,
    carrier: Query<(&Transform, &CarrierShip), (Without<Bullet>, Without<Enemy>, Without<Rock>, Without<Player>)>,
    mut allies: Query<(&Transform, &mut crate::allies::Ally), (Without<Bullet>, Without<Enemy>, Without<Rock>, Without<Player>, Without<CarrierShip>)>,
    diff: Res<crate::missions::Difficulty>,
    mut spartans: Query<(&Transform, &mut crate::spartan::Spartan), (Without<Bullet>, Without<Enemy>, Without<Rock>, Without<Player>, Without<CarrierShip>, Without<crate::allies::Ally>)>,
) {
    let dt = time.delta_secs();
    let carrier_k = diff.carrier_dmg();
    let ppos = player.translation;
    let ctf = carrier.iter().next().map(|(t, c)| (*t, c.half));
    for (e, mut tf, mut b) in &mut bq {
        b.life -= dt;
        b.prev = tf.translation;
        tf.translation += b.vel * dt;
        let (p0, p1) = (b.prev, tf.translation);
        let mut dead = b.life <= 0.0;
        if !dead && game.zone == Zone::Atmosphere && p1.y < ground_height(p1.x, p1.z) {
            dead = true;
            if b.from_player {
                spawn_spark(&mut commands, &a, p1);
            }
        }
        if !dead {
            for (rtf, r) in &rocks {
                if seg_hit(p0, p1, rtf.translation, r.radius) {
                    dead = true;
                    spawn_spark(&mut commands, &a, p1);
                    break;
                }
            }
        }
        if !dead && b.from_player {
            for (etf, mut en) in &mut enemies {
                if en.hp > 0.0 && enemy_hit(etf, &en, p0, p1, 1.5) {
                    if !(en.kind == EnemyKind::Mothership && game.mother_shield) {
                        en.hp -= b.damage;
                    }
                    en.hit_flash = 0.12;
                    dead = true;
                    spawn_spark(&mut commands, &a, p1);
                    break;
                }
            }
        }
        if !dead && !b.from_player {
            if let Some((ct, half)) = ctf {
                if box_contains(&ct, half, p1, 0.0) {
                    dead = true;
                    game.carrier_hp -= b.damage * carrier_k;
                    spawn_spark(&mut commands, &a, p1);
                }
            }
        }
        if !dead && !b.from_player {
            for (at, mut al) in &mut allies {
                if seg_hit(p0, p1, at.translation, 7.0) {
                    al.hp -= b.damage;
                    if al.slot == 1 && rand::thread_rng().gen_bool(0.12) {
                        crate::sfx::bark(&mut commands, "#a2_impacto");
                    }
                    dead = true;
                    spawn_spark(&mut commands, &a, p1);
                    break;
                }
            }
        }
        if !dead && !b.from_player {
            for (st, mut sp) in &mut spartans {
                if sp.hp > 0.0 && seg_hit(p0, p1, st.translation + Vec3::Y * 8.0, 8.0) {
                    sp.hp -= b.damage * 1.5;
                    dead = true;
                    spawn_spark(&mut commands, &a, p1);
                    break;
                }
            }
        }
        if !dead && !b.from_player && !game.dead && seg_hit(p0, p1, ppos, 6.0) {
            dead = true;
            game.damage(b.damage);
            play(&mut commands, &sounds.hit, 0.5);
            play(&mut commands, &sounds.cymbal, 0.15);
            crate::sfx::sfx(&mut commands, crate::sfx::SfxKind::Hit, None, 1.0);
            let mut rng = rand::thread_rng();
            if game.hull < 35.0 && rng.gen_bool(0.25) {
                crate::sfx::bark(&mut commands, "#p_aguanta");
            } else if b.damage >= 10.0 && rng.gen_bool(0.35) {
                crate::sfx::bark(&mut commands, ["#p_impacto1", "#p_impacto2", "#a1_aviso", "#a1_izq"][rng.gen_range(0..4)]);
            }
        }
        if dead {
            commands.entity(e).try_despawn();
        }
    }
}

fn spawn_spark(commands: &mut Commands, a: &GameAssets, p: Vec3) {
    commands.spawn((
        Mesh3d(a.plain_sphere.clone()),
        MeshMaterial3d(a.m_fire.clone()),
        Transform::from_translation(p).with_scale(Vec3::splat(0.1)),
        Effect { kind: EffectKind::Fireball { size: 2.5 }, t: 0.0, life: 0.18 },
        NotShadowCaster,
    ));
}

pub fn physical_collisions(
    mut commands: Commands,
    sounds: Res<Sounds>,
    mut game: ResMut<Game>,
    mut player: Single<(&mut Transform, &mut Player)>,
    mut enemies: Query<(&Transform, &mut Enemy), Without<Player>>,
    rocks: Query<(&Transform, &Rock), (Without<Player>, Without<Enemy>)>,
    carrier: Query<(&Transform, &CarrierShip), (Without<Player>, Without<Enemy>, Without<Rock>)>,
) {
    if game.dead {
        return;
    }
    for (ct, c) in &carrier {
        let (ref mut ptf, ref mut pl) = *player;
        if box_contains(ct, c.half, ptf.translation, 5.0) {
            let local = ct.rotation.inverse() * (ptf.translation - ct.translation);
            // empujar hacia la cara más cercana de la caja
            let pen = c.half + Vec3::splat(5.0) - local.abs();
            let push = if pen.x < pen.y && pen.x < pen.z {
                Vec3::X * local.x.signum() * (pen.x + 8.0)
            } else if pen.y < pen.z {
                Vec3::Y * local.y.signum() * (pen.y + 8.0)
            } else {
                Vec3::Z * local.z.signum() * (pen.z + 8.0)
            };
            ptf.translation += ct.rotation * push;
            pl.speed *= 0.5;
            game.damage(15.0);
            play(&mut commands, &sounds.hit, 0.8);
            play(&mut commands, &sounds.cymbal, 0.24);
            game.message("¡CUIDADO CON EL CASCO DE LA NAVE NODRIZA!", 2.0);
        }
    }
    let (ref mut ptf, ref mut pl) = *player;
    for (etf, mut en) in &mut enemies {
        if en.hp <= 0.0 {
            continue;
        }
        match en.half_extents {
            Some(h) => {
                if box_contains(etf, h, ptf.translation, 6.0) {
                    let away = (ptf.translation - etf.translation).normalize_or(Vec3::Y);
                    ptf.translation += away * 40.0;
                    pl.speed *= 0.3;
                    game.damage(25.0);
                    play(&mut commands, &sounds.hit, 0.9);
                    play(&mut commands, &sounds.cymbal, 0.27);
                    game.message("¡COLISIÓN!", 2.0);
                }
            }
            None => {
                if etf.translation.distance(ptf.translation) < en.radius + 5.0 && en.radius >= 12.0 {
                    // choque con un jefe: no lo destruye, te empuja y te daña mucho
                    let away = (ptf.translation - etf.translation).normalize_or(Vec3::Y);
                    ptf.translation = etf.translation + away * (en.radius + 14.0);
                    pl.speed *= 0.4;
                    en.hp -= 40.0;
                    game.damage(35.0);
                    game.shake = (game.shake + 0.8).min(1.5);
                    play(&mut commands, &sounds.hit, 1.0);
                    play(&mut commands, &sounds.cymbal, 0.3);
                    game.message("¡IMPACTO CON EL JEFE INVID!", 2.0);
                } else if etf.translation.distance(ptf.translation) < en.radius + 5.0 {
                    en.hp = 0.0;
                    game.damage(40.0);
                    play(&mut commands, &sounds.hit, 0.9);
                    play(&mut commands, &sounds.cymbal, 0.27);
                }
            }
        }
    }
    for (rtf, r) in &rocks {
        let d = ptf.translation.distance(rtf.translation);
        if d < r.radius + 5.0 {
            let away = (ptf.translation - rtf.translation).normalize_or(Vec3::Y);
            ptf.translation = rtf.translation + away * (r.radius + 12.0);
            pl.speed *= 0.4;
            game.damage(25.0);
            play(&mut commands, &sounds.hit, 0.9);
            play(&mut commands, &sounds.cymbal, 0.27);
            game.message("¡IMPACTO CON ASTEROIDE!", 2.0);
        }
    }
}

pub fn enemy_deaths(
    mut commands: Commands,
    a: Res<GameAssets>,
    sounds: Res<Sounds>,
    mut game: ResMut<Game>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform, Has<MothershipTurret>, Has<crate::spartan::AsaltoTerrestre>)>,
    bosses: Query<(), With<crate::mecha::Boss>>,
    player: Single<&Transform, (With<Player>, Without<Enemy>)>,
) {
    let ppos = player.translation;
    let mut mother_died = false;
    for (e, en, tf, _, asalto) in &enemies {
        if en.hp > 0.0 {
            continue;
        }
        game.score += en.score;
        match en.kind {
            EnemyKind::Mothership => {
                mother_died = true;
                game.mothership_alive = false;
                for i in 0..14 {
                    let off = tf.rotation * Vec3::new(((i * 37) % 7) as f32 * 20.0 - 60.0, ((i * 13) % 5) as f32 * 15.0 - 30.0, i as f32 * 45.0 - 300.0);
                    spawn_explosion(&mut commands, &a, tf.translation + off, 70.0, Vec3::ZERO);
                }
                play(&mut commands, &sounds.big_explosion, 1.0);
                play(&mut commands, &sounds.kick, 1.0);
                play(&mut commands, &sounds.cymbal, 0.5);
                game.message("¡¡NAVE NODRIZA DESTRUIDA!!  +5000", 5.0);
                game.shake = 1.5;
            }
            EnemyKind::Turret => {
                game.kills += 1;
                game.turret_kills += 1;
                spawn_explosion(&mut commands, &a, tf.translation, 22.0, Vec3::ZERO);
                play(&mut commands, &sounds.explosion, 0.6);
                play(&mut commands, &sounds.kick, 0.54);
            }
            EnemyKind::Hive => {
                game.hive_alive = false;
                crate::mecha::hive_destroyed(&mut commands, &a, &sounds, tf);
                game.message("¡¡COLMENA INVID DESTRUIDA!!  +3000", 5.0);
                game.shake = 1.5;
            }
            EnemyKind::Fighter if bosses.get(e).is_ok() => {
                game.kills += 1;
                game.ev_kill = true;
                for k in 0..5 {
                    let o = Vec3::new((k as f32 * 2.1).sin(), (k as f32 * 1.3).cos() * 0.6, (k as f32 * 3.7).sin()) * 10.0;
                    spawn_explosion(&mut commands, &a, tf.translation + o, 26.0, en.vel * 0.3);
                }
                game.ev_kill_pos = Some((tf.translation, en.vel * 0.1));
                play(&mut commands, &sounds.big_explosion, 0.9);
                play(&mut commands, &sounds.kick, 1.0);
                crate::sfx::bark(&mut commands, "#p_jefe");
                game.message("¡JEFE INVID DERRIBADO!  +1500", 3.5);
            }
            EnemyKind::Fighter if asalto && game.lock != Some(e) && tf.translation.distance(ppos) > 900.0 => {
                // derribado por los Spartan en su sector: no cuenta para la misión
                game.score = game.score.saturating_sub(en.score) + en.score / 2;
                spawn_explosion(&mut commands, &a, tf.translation, 9.0, en.vel);
                crate::models::spawn_wreck(&mut commands, &a, tf, en.vel);
                if tf.translation.distance(ppos) < 2500.0 {
                    play(&mut commands, &sounds.explosion, 0.3);
                }
            }
            EnemyKind::Fighter => {
                game.kills += 1;
                game.ev_kill = true;
                spawn_explosion(&mut commands, &a, tf.translation, 9.0, en.vel);
                crate::models::spawn_wreck(&mut commands, &a, tf, en.vel);
                game.ev_kill_pos = Some((tf.translation, en.vel * 0.15));
                play(&mut commands, &sounds.explosion, 0.7);
                play(&mut commands, &sounds.kick, 0.63);
                let mut rng = rand::thread_rng();
                let mine = game.lock == Some(e) || tf.translation.distance(ppos) < 700.0;
                if mine && rng.gen_bool(0.35) {
                    crate::sfx::bark(&mut commands, ["#p_derribo1", "#p_derribo2", "#p_derribo3"][rng.gen_range(0..3)]);
                } else if !mine && rng.gen_bool(0.2) {
                    crate::sfx::bark(&mut commands, ["#a1_derribo", "#a2_derribo", "#a3_derribo", "#a4_derribo"][rng.gen_range(0..4)]);
                }
            }
        }
        commands.entity(e).try_despawn();
    }
    if mother_died {
        for (_, mut en, _, is_mt, _) in &mut enemies {
            if is_mt {
                en.hp = 0.0;
            }
        }
    }
}

pub fn player_status(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    sounds: Res<Sounds>,
    mut game: ResMut<Game>,
    player: Single<&Transform, With<Player>>,
    diff: Res<crate::missions::Difficulty>,
) {
    let dt = time.delta_secs();
    let regen = diff.shield_regen();
    game.msg_timer -= dt;
    game.shake = (game.shake - 2.0 * dt).max(0.0);
    game.hit_flash = (game.hit_flash - 1.5 * dt).max(0.0);
    if game.dead {
        return;
    }
    game.shield_delay -= dt;
    if game.shield_delay <= 0.0 {
        game.shield = (game.shield + regen * dt).min(100.0);
    }
    if game.hull <= 0.0 {
        game.hull = 0.0;
        game.dead = true;
        spawn_explosion(&mut commands, &a, player.translation, 25.0, Vec3::ZERO);
        play(&mut commands, &sounds.big_explosion, 0.9);
        play(&mut commands, &sounds.kick, 1.0);
        play(&mut commands, &sounds.cymbal, 0.5);
        game.message("DERRIBADO — pulsa R para volver a despegar", 999.0);
    }
}

// ---------------------------------------------------------------- efectos

pub fn effects(
    time: Res<Time>,
    game: Res<Game>,
    dir: Res<Director>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Effect, &mut Transform, &mut Visibility, Option<&mut PointLight>)>,
) {
    let dt = time.delta_secs();
    let cam = dir.cam.translation;
    for (e, mut fx, mut tf, mut vis, _light) in &mut q {
        // si la cámara queda dentro de una bocanada, su contorno taparía la pantalla
        if let EffectKind::Smoke { .. } = fx.kind {
            *vis = if tf.translation.distance(cam) < tf.scale.x * 1.25 + 0.5 { Visibility::Hidden } else { Visibility::Inherited };
        }
        fx.t += dt;
        let k = (fx.t / fx.life).min(1.0);
        match fx.kind {
            EffectKind::Fireball { size } => {
                let s = size * (0.25 + 1.0 * k.powf(0.35)) * (1.0 - k.powi(3));
                tf.scale = Vec3::splat(s.max(0.01));
            }
            EffectKind::Smoke { size } => {
                let s = size * (0.4 + 1.4 * k.sqrt()) * (1.0 - k.powi(4));
                tf.scale = Vec3::splat(s.max(0.01));
                if game.zone == Zone::Atmosphere {
                    tf.translation.y += 6.0 * dt;
                }
            }
            EffectKind::Wreck => {}
            EffectKind::Debris { mut vel, spin } => {
                if game.zone == Zone::Atmosphere {
                    vel.y -= 9.8 * dt;
                }
                tf.translation += vel * dt;
                tf.rotate(Quat::from_scaled_axis(spin * dt));
                fx.kind = EffectKind::Debris { vel, spin };
                tf.scale *= 1.0 - 0.3 * dt;
            }
        }
        if fx.t >= fx.life {
            commands.entity(e).try_despawn();
        }
    }
}

// ---------------------------------------------------------------- entorno

pub fn environment(
    time: Res<Time>,
    game: Res<Game>,
    dir: Res<Director>,
    mut fogs: Query<&mut DistanceFog>,
    mut stars: Single<(&mut Transform, &mut Visibility), With<StarField>>,
    mut sky: Query<&mut Transform, (With<SkyDome>, Without<StarField>)>,
    mut sun: Query<&mut Transform, (With<SunDisc>, Without<StarField>, Without<SkyDome>)>,
    mut galaxy: Query<&mut Transform, (With<Galaxy>, Without<StarField>, Without<SkyDome>, Without<SunDisc>)>,
    mut clouds: Query<&mut Transform, (With<EarthClouds>, Without<StarField>, Without<SkyDome>, Without<SunDisc>, Without<Galaxy>)>,
    mut rocks: Query<(&mut Transform, &Rock), (Without<StarField>, Without<SkyDome>, Without<SunDisc>, Without<Galaxy>, Without<EarthClouds>)>,
    mut dust: Query<&mut Transform, (With<SpaceDust>, Without<StarField>, Without<SkyDome>, Without<SunDisc>, Without<Galaxy>, Without<EarthClouds>, Without<Rock>)>,
    player: Single<&Transform, (With<Player>, Without<StarField>, Without<SkyDome>, Without<SunDisc>, Without<Galaxy>, Without<EarthClouds>, Without<Rock>, Without<SpaceDust>)>,
) {
    let dt = time.delta_secs();
    let c = dir.cam.translation;
    let p = player.translation;
    stars.0.translation = c;
    for mut s in &mut sky {
        s.translation = c;
    }
    for mut g in &mut galaxy {
        g.translation = c + Vec3::new(0.35, -0.25, 0.9).normalize() * 52000.0;
        g.rotation = Quat::from_rotation_x(0.9) * Quat::from_rotation_z(0.4);
    }
    match game.zone {
        Zone::Atmosphere => {
            for mut s in &mut sun {
                s.translation = c + SUN_DIR.normalize() * 60000.0;
                s.scale = Vec3::splat(2600.0);
            }
            let t = ((p.y - 1500.0) / (CEILING - 1500.0)).clamp(0.0, 1.0);
            for mut fog in &mut fogs {
                fog.color = Color::srgb(0.95, 0.58, 0.52).mix(&Color::srgb(0.40, 0.22, 0.55), t);
                fog.falloff = FogFalloff::Linear { start: 2500.0 + t * 3000.0, end: 13000.0 + t * 15000.0 };
            }
            *stars.1 = Visibility::Hidden;
        }
        Zone::Space => {
            for mut s in &mut sun {
                s.translation = c + SPACE_SUN.normalize() * 60000.0;
                s.scale = Vec3::splat(1500.0);
            }
            for mut fog in &mut fogs {
                fog.color = Color::NONE;
                fog.falloff = FogFalloff::Linear { start: 1.0e7, end: 2.0e7 };
            }
            *stars.1 = Visibility::Visible;
        }
    }
    for mut cl in &mut clouds {
        cl.rotate_y(0.004 * dt);
    }
    for (mut tf, r) in &mut rocks {
        tf.rotate(Quat::from_scaled_axis(r.spin * dt));
    }
    for mut d in &mut dust {
        let rel = d.translation - p;
        let wrap = |v: f32| ((v + 300.0).rem_euclid(600.0)) - 300.0;
        d.translation = p + Vec3::new(wrap(rel.x), wrap(rel.y), wrap(rel.z));
    }
}

// ---------------------------------------------------------------- zonas / oleadas

pub fn zone_transition(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    sounds: Res<Sounds>,
    mut game: ResMut<Game>,
    mut player: Single<(&mut Transform, &mut Player)>,
    to_clear: Query<Entity, Or<(With<ZoneEntity>, With<Enemy>, With<Bullet>, With<Missile>, With<Effect>)>>,
) {
    let dt = time.delta_secs();
    let (ref mut tf, ref mut pl) = *player;
    if game.pending_zone.is_none() && !game.dead && game.started {
        match game.zone {
            Zone::Atmosphere if tf.translation.y > CEILING => {
                game.pending_zone = Some(Zone::Space);
                game.fade_dir = 1.0;
                game.message("SALIENDO DE LA ATMÓSFERA...", 3.0);
                play(&mut commands, &sounds.whoosh, 0.8);
            }
            Zone::Space if tf.translation.distance(EARTH_CENTER) < EARTH_RADIUS + 1700.0 => {
                game.pending_zone = Some(Zone::Atmosphere);
                game.fade_dir = 1.0;
                game.message("REENTRADA ATMOSFÉRICA...", 3.0);
                play(&mut commands, &sounds.whoosh, 0.8);
            }
            _ => {}
        }
    }
    if game.fade_dir > 0.0 {
        game.fade = (game.fade + dt * 1.4).min(1.0);
        if game.fade >= 1.0 {
            if let Some(z) = game.pending_zone.take() {
                for e in &to_clear {
                    commands.entity(e).try_despawn();
                }
                let fwd = tf.forward().as_vec3();
                let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
                if game.reset_on_switch {
                    let keep_help = game.show_help;
                    *game = Game { started: true, fade: 1.0, show_help: keep_help, ..default() };
                    tf.translation = Vec3::new(0.0, 700.0, 5200.0);
                    tf.rotation = Quat::IDENTITY;
                    pl.speed = 220.0;
                    pl.throttle = 0.55;
                    spawn_atmosphere(&mut commands, &a, true);
                    game.message("Nuevo despegue. ¡Buena caza, piloto!", 3.0);
                } else {
                    match z {
                        Zone::Space => {
                            tf.translation = Vec3::new(0.0, -300.0, 1800.0);
                            tf.rotation = Quat::IDENTITY;
                            spawn_space(&mut commands, &a, game.mothership_alive, game.carrier_hp > 0.0);
                            let m = if game.mothership_alive { "ÓRBITA TERRESTRE — destruye la nave nodriza" } else { "ÓRBITA TERRESTRE" };
                            game.message(m, 4.0);
                        }
                        Zone::Atmosphere => {
                            tf.translation = Vec3::new(0.0, CEILING - 800.0, 3000.0);
                            tf.rotation = Transform::IDENTITY.looking_to((flat - Vec3::Y * 0.3).normalize(), Vec3::Y).rotation;
                            spawn_atmosphere(&mut commands, &a, game.carrier_hp > 0.0);
                            game.message("ESPACIO AÉREO TERRESTRE", 3.0);
                        }
                    }
                    game.zone = z;
                }
                game.wave_timer = 4.0;
                game.ev_zone = true;
                game.lock = None;
                game.locked = false;
                game.fade_dir = -1.0;
            }
        }
    } else if game.fade_dir < 0.0 {
        game.fade = (game.fade - dt * 0.9).max(0.0);
        if game.fade <= 0.0 {
            game.fade_dir = 0.0;
        }
    }
}

pub fn waves(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    mut game: ResMut<Game>,
    player: Single<&Transform, With<Player>>,
    enemies: Query<(&Enemy, &Transform, Has<crate::spartan::AsaltoTerrestre>), Without<Player>>,
    carrier: Query<&CarrierShip>,
    diff: Res<crate::missions::Difficulty>,
) {
    let has_carrier = !carrier.is_empty();
    let assign = |commands: &mut Commands, e: Entity, chance: f64| {
        let mut rng = rand::thread_rng();
        if has_carrier && rng.gen_bool(chance) {
            let s = crate::models::CARRIER_SCALE;
            let aim = Vec3::new(rng.gen_range(-0.25..0.25) * s, rng.gen_range(0.0..0.08) * s, rng.gen_range(-0.75..0.75) * s);
            commands.entity(e).insert(AttackCarrier { aim });
        }
    };
    if game.pending_zone.is_some() || game.dead {
        return;
    }
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    let fighters = enemies.iter().filter(|(e, _, asalto)| e.kind == EnemyKind::Fighter && !asalto).count();
    let p = player.translation;
    if fighters == 0 && game.auto_waves {
        game.wave_timer -= dt;
        if game.wave_timer <= 0.0 {
            game.wave += 1;
            let n = diff.wave_size(game.wave);
            for _ in 0..n {
                let ang = rng.gen_range(0.0..std::f32::consts::TAU);
                let d = rng.gen_range(2200.0..3200.0);
                let mut pos = p + Vec3::new(ang.cos() * d, rng.gen_range(-300.0..500.0), ang.sin() * d);
                if game.zone == Zone::Atmosphere {
                    pos.x = pos.x.clamp(-ATMOS_LIMIT, ATMOS_LIMIT);
                    pos.z = pos.z.clamp(-ATMOS_LIMIT, ATMOS_LIMIT);
                    let g = ground_height(pos.x, pos.z);
                    pos.y = pos.y.clamp(g + 350.0, CEILING - 500.0);
                }
                let e = spawn_fighter(&mut commands, &a, pos, p, game.wave);
                assign(&mut commands, e, 0.4);
            }
            let w = game.wave;
            game.message(format!("OLEADA {w}: {n} cazas enemigos se aproximan"), 3.5);
            game.wave_timer = 5.0;
        }
    }
    // La nave nodriza lanza cazas periódicamente
    if game.zone == Zone::Space && game.mothership_alive {
        game.launch_timer -= dt;
        if game.launch_timer <= 0.0 && fighters < 12 {
            game.launch_timer = 14.0;
            if let Some((_, mtf, _)) = enemies.iter().find(|(e, _, _)| e.kind == EnemyKind::Mothership) {
                for s in [-1.0f32, 1.0] {
                    let pos = mtf.transform_point(Vec3::new(s * 20.0, -60.0, -420.0));
                    let e = spawn_fighter(&mut commands, &a, pos, p, game.wave);
                    assign(&mut commands, e, 0.6);
                }
                game.message("¡La nave nodriza lanza cazas!", 2.0);
            }
        }
    }
}

pub fn engine_audio(
    game: Res<Game>,
    player: Single<&Player>,
    sink: Query<&mut AudioSink, With<EngineSound>>,
    mut mix: ResMut<crate::sfx::MixTargets>,
) {
    use bevy::audio::Volume;
    let base = if game.zone == Zone::Space { 0.18 } else { 0.32 };
    let v = if !game.started || game.paused || game.dead { 0.0 } else { base + player.throttle * 0.15 + if player.boosting { 0.15 } else { 0.0 } };
    let speed = 0.7 + player.speed / 700.0;
    mix.engine = (v, speed);
    for mut s in sink {
        s.set_volume(Volume::Linear(v));
        s.set_speed(speed);
    }
}

/// Nave nodriza terrestre: defensa antiaérea, anillo de recarga y daños.
pub fn carrier_update(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    sounds: Res<Sounds>,
    mut game: ResMut<Game>,
    mut last_hp: Local<f32>,
    player: Single<&Transform, With<Player>>,
    mut carrier: Query<(Entity, &Transform, &mut CarrierShip), Without<Player>>,
    mut ring: Query<(Entity, &mut Transform, &mut MeshMaterial3d<StandardMaterial>), (With<RearmRing>, Without<Player>, Without<CarrierShip>)>,
    enemies: Query<(&Transform, &Enemy), (Without<Player>, Without<CarrierShip>, Without<RearmRing>, Without<crate::missions::Elite>)>,
) {
    let dt = time.delta_secs();
    game.rearm_cd -= dt;
    game.rearm_flash = (game.rearm_flash - dt).max(0.0);
    game.carrier_alert_cd -= dt;
    let mut rng = rand::thread_rng();
    let Some((ce, ctf, mut c)) = carrier.iter_mut().next() else {
        *last_hp = game.carrier_hp;
        return;
    };
    let ctf = *ctf;
    // defensa antiaérea: dispara al Invid más cercano
    c.gun_cd -= dt;
    if c.gun_cd <= 0.0 {
        let best = enemies
            .iter()
            .filter(|(_, e)| e.kind == EnemyKind::Fighter && e.hp > 0.0)
            .map(|(t, e)| (t.translation, e.vel, t.translation.distance(ctf.translation)))
            .filter(|x| x.2 < 1100.0)
            .min_by(|x, y| x.2.total_cmp(&y.2));
        if let Some((tp, tv, d)) = best {
            c.gun_cd = 0.35;
            let local = Vec3::new(rng.gen_range(-0.3..0.3), 0.12, rng.gen_range(-0.7..0.7)) * crate::models::CARRIER_SCALE;
            let muzzle = ctf.transform_point(local);
            let dir = (tp + tv * (d / 1300.0) - muzzle).normalize_or(Vec3::Y);
            let spread = Vec3::new(rng.gen_range(-0.03..0.03), rng.gen_range(-0.03..0.03), rng.gen_range(-0.03..0.03));
            spawn_bullet(&mut commands, &a, muzzle, (dir + spread).normalize() * 1300.0, 5.0, true, false);
        } else {
            c.gun_cd = 0.5;
        }
    }
    // aviso de ataque
    if game.carrier_hp < *last_hp - 0.5 && game.carrier_alert_cd <= 0.0 && !game.dead {
        game.carrier_alert_cd = 9.0;
        let pct = (game.carrier_hp / CARRIER_HP * 100.0).floor().max(0.0);
        game.message(format!("¡LA NAVE NODRIZA TERRESTRE ESTÁ BAJO ATAQUE! ({pct:.0}%)"), 3.0);
        play(&mut commands, &sounds.alarm, 0.35);
    }
    *last_hp = game.carrier_hp;
    // destruida: fin de la misión
    if game.carrier_hp <= 0.0 {
        game.carrier_hp = 0.0;
        for i in 0..16 {
            let local = Vec3::new(rng.gen_range(-0.35..0.35), rng.gen_range(-0.1..0.12), -0.85 + i as f32 * 0.11) * crate::models::CARRIER_SCALE;
            spawn_explosion(&mut commands, &a, ctf.transform_point(local), 60.0, Vec3::ZERO);
        }
        play(&mut commands, &sounds.big_explosion, 1.0);
        play(&mut commands, &sounds.kick, 1.0);
        play(&mut commands, &sounds.cymbal, 0.5);
        commands.entity(ce).try_despawn();
        for (re, _, _) in &ring {
            commands.entity(re).try_despawn();
        }
        game.dead = true;
        game.shake = 1.5;
        game.message("MISIÓN FALLIDA: la nave nodriza terrestre fue destruida — pulsa R", 999.0);
        return;
    }
    // anillo de recarga
    let ready = game.rearm_cd <= 0.0;
    let t = time.elapsed_secs();
    for (_, mut rt, mut mat) in &mut ring {
        rt.rotate_local_y(dt * 0.6);
        rt.scale = Vec3::splat(if ready { 1.0 + (t * 3.0).sin() * 0.05 } else { 0.85 });
        let want = if ready { a.m_ring.clone() } else { a.m_ring_off.clone() };
        if mat.0 != want {
            mat.0 = want;
        }
        let d = rt.translation.distance(player.translation);
        if ready && d < crate::models::REARM_RADIUS && !game.dead {
            game.missiles = 8;
            game.missile_reload = 0.0;
            game.cluster = 3;
            game.cluster_reload = 0.0;
            crate::sfx::bark(&mut commands, "#c_recarga");
            game.hull = (game.hull + 50.0).min(100.0);
            game.shield = 100.0;
            game.heat = 0.0;
            game.overheated = false;
            game.rearm_cd = 12.0;
            game.rearm_flash = 1.2;
            play(&mut commands, &sounds.lock, 0.7);
            play(&mut commands, &sounds.whoosh, 0.4);
            game.message("RECARGA COMPLETA: misiles 8/8 · racimo 3/3 · escudo 100% · casco reparado", 3.0);
        }
    }
}

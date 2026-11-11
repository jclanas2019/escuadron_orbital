//! Campaña: 5 misiones con radio, eventos de tensión y dos niveles de dificultad.

use std::collections::VecDeque;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rand::Rng;

use crate::audio::{play, Sounds};
use crate::components::*;
use crate::gameplay::{spawn_explosion, spawn_puff};
use crate::models::*;
use crate::world::*;

// ---------------------------------------------------------------- dificultad

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Basica,
    Media,
}

#[derive(Resource)]
pub struct Difficulty(pub Level);

impl Difficulty {
    pub fn name(&self) -> &'static str {
        match self.0 {
            Level::Basica => "BÁSICA",
            Level::Media => "MEDIA",
        }
    }
    pub fn media(&self) -> bool {
        self.0 == Level::Media
    }
    /// Multiplicador del daño que hacen los Invid.
    pub fn enemy_dmg(&self) -> f32 {
        if self.media() { 1.0 } else { 0.6 }
    }
    /// Multiplicador del tiempo entre disparos enemigos (más alto = disparan menos).
    pub fn fire_cd(&self) -> f32 {
        if self.media() { 1.0 } else { 1.5 }
    }
    pub fn enemy_hp(&self) -> f32 {
        if self.media() { 1.0 } else { 0.75 }
    }
    pub fn enemy_speed(&self) -> f32 {
        if self.media() { 1.0 } else { 0.85 }
    }
    pub fn carrier_dmg(&self) -> f32 {
        if self.media() { 1.0 } else { 0.5 }
    }
    pub fn shield_regen(&self) -> f32 {
        if self.media() { 12.0 } else { 20.0 }
    }
    pub fn lock_time(&self) -> f32 {
        if self.media() { 0.8 } else { 0.5 }
    }
    pub fn wave_size(&self, wave: u32) -> u32 {
        if self.media() { (2 + wave).min(9) } else { (1 + wave).min(5) }
    }
    /// Plasma guiado Invid (sólo en dificultad media, desde la misión 3).
    pub fn plasma(&self, mission: usize) -> bool {
        self.media() && mission >= 2
    }
}

/// Aplica la dificultad a cada enemigo nuevo.
pub fn scale_new_enemies(diff: Res<Difficulty>, mut q: Query<&mut Enemy, Added<Enemy>>) {
    for mut e in &mut q {
        if e.kind == EnemyKind::Mothership {
            continue;
        }
        e.hp *= diff.enemy_hp();
        e.max_hp *= diff.enemy_hp();
        e.speed *= diff.enemy_speed();
    }
}

// ---------------------------------------------------------------- radio

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Speaker {
    Control,
    Azul1,
    Azul3,
    Alerta,
}

impl Speaker {
    pub fn label(self) -> &'static str {
        match self {
            Speaker::Control => "CONTROL",
            Speaker::Azul1 => "AZUL 1",
            Speaker::Azul3 => "AZUL 3",
            Speaker::Alerta => "ALERTA",
        }
    }
    pub fn color(self) -> Color {
        match self {
            Speaker::Control => Color::srgb(1.0, 0.75, 0.3),
            Speaker::Azul1 | Speaker::Azul3 => Color::srgb(0.45, 0.8, 1.0),
            Speaker::Alerta => Color::srgb(1.0, 0.3, 0.25),
        }
    }
}

#[derive(Resource, Default)]
pub struct Radio {
    pub queue: VecDeque<(Speaker, String)>,
    pub current: Option<(Speaker, String, f32)>,
}

impl Radio {
    pub fn say(&mut self, who: Speaker, text: impl Into<String>) {
        self.queue.push_back((who, text.into()));
    }
    /// Mensaje urgente: interrumpe lo que se esté diciendo.
    pub fn urgent(&mut self, who: Speaker, text: impl Into<String>) {
        self.queue.clear();
        self.current = None;
        self.queue.push_back((who, text.into()));
    }
    pub fn clear(&mut self) {
        self.queue.clear();
        self.current = None;
    }
}

pub fn radio_update(
    time: Res<Time<Real>>,
    mut radio: ResMut<Radio>,
    mut commands: Commands,
    sounds: Res<Sounds>,
    game: Res<Game>,
    capas: Option<Res<crate::sfx::Capas>>,
    mut st: ResMut<crate::sfx::SfxState>,
) {
    let dt = time.delta_secs();
    if game.paused {
        return;
    }
    if let Some((_, _, t)) = radio.current.as_mut() {
        *t -= dt;
        if *t <= 0.0 {
            radio.current = None;
        }
    }
    if radio.current.is_none() {
        if let Some((who, text)) = radio.queue.pop_front() {
            let mut dur = 2.2 + text.chars().count() as f32 * 0.045;
            // voz de radio en japonés (subtítulos en español)
            if let Some(v) = crate::sfx::radio_voice(&mut commands, capas.as_deref(), &mut st, Some(&text)) {
                dur = dur.max(v + 0.6);
            }
            radio.current = Some((who, text, dur));
            play(&mut commands, &sounds.lock, 0.12);
        } else {
            crate::sfx::radio_close(&mut commands, capas.as_deref(), &mut st);
        }
    } else if radio.current.is_none() && st.voice.is_some() {
        crate::sfx::radio_close(&mut commands, capas.as_deref(), &mut st);
    }
}

// ---------------------------------------------------------------- misiones

pub const MISSIONS: [&str; 5] = ["PRIMER CONTACTO", "DEFENDER LA NODRIZA", "LA COLMENA", "EL COMANDANTE", "ASALTO ORBITAL"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Esperando a que empiece el juego.
    Idle,
    Briefing,
    Active,
    Complete,
    Failed,
    Victory,
    /// Tras la campaña: patrulla libre con oleadas infinitas.
    Free,
}

#[derive(Resource)]
pub struct Mission {
    pub idx: usize,
    pub phase: Phase,
    pub t: f32,
    pub step: u8,
    pub base_kills: u32,
    pub base_turrets: u32,
    pub need: u32,
    pub timer: f32,
    pub spawn_cd: f32,
    pub events: u32,
    pub elite: Option<Entity>,
    pub banner: f32,
    pub banner_title: String,
    pub banner_sub: String,
    pub objective: String,
    pub last_beep: i32,
    pub stats_time: f32,
}

impl Default for Mission {
    fn default() -> Self {
        Self {
            idx: 0,
            phase: Phase::Idle,
            t: 0.0,
            step: 0,
            base_kills: 0,
            base_turrets: 0,
            need: 0,
            timer: 0.0,
            spawn_cd: 0.0,
            events: 0,
            elite: None,
            banner: 0.0,
            banner_title: String::new(),
            banner_sub: String::new(),
            objective: String::new(),
            last_beep: -1,
            stats_time: 0.0,
        }
    }
}

impl Mission {
    fn once(&mut self, bit: u32) -> bool {
        let first = self.events & (1 << bit) == 0;
        self.events |= 1 << bit;
        first
    }
    fn banner(&mut self, title: impl Into<String>, sub: impl Into<String>, secs: f32) {
        self.banner_title = title.into();
        self.banner_sub = sub.into();
        self.banner = secs;
    }
}

/// Invid de élite (el "Comandante" de la misión 4).
#[derive(Component)]
pub struct Elite;

/// Proyectil de plasma guiado Invid.
#[derive(Component)]
pub struct EnemyPlasma {
    pub vel: Vec3,
    pub life: f32,
    pub trail: f32,
}

/// Recarga del plasma de cada caza Invid.
#[derive(Component)]
pub struct PlasmaCd(pub f32);

/// Burbuja de escudo de la nave nodriza Invid.
#[derive(Component)]
pub struct MotherShield;

#[derive(Resource)]
pub struct MissionAssets {
    pub shield: Handle<StandardMaterial>,
    pub plasma: Handle<StandardMaterial>,
    pub aura: Handle<StandardMaterial>,
}

pub fn setup_missions(mut commands: Commands, mut mats: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(MissionAssets {
        shield: mats.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.3, 0.8, 0.16),
            emissive: LinearRgba::rgb(0.8, 0.1, 0.6),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            fog_enabled: false,
            ..default()
        }),
        plasma: mats.add(StandardMaterial {
            base_color: Color::LinearRgba(LinearRgba::rgb(14.0, 2.0, 10.0)),
            unlit: true,
            fog_enabled: false,
            ..default()
        }),
        aura: mats.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.1, 0.1, 0.25),
            emissive: LinearRgba::rgb(3.0, 0.2, 0.1),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            ..default()
        }),
    });
}

/// Aparece un grupo de Invid a cierta distancia de un punto.
#[allow(clippy::too_many_arguments)]
fn spawn_group(
    commands: &mut Commands,
    a: &GameAssets,
    game: &Game,
    center: Vec3,
    look: Vec3,
    n: u32,
    dist: f32,
    dir: Option<Vec3>,
    carrier_chance: f64,
    has_carrier: bool,
) {
    let mut rng = rand::thread_rng();
    let base_dir = dir.unwrap_or_else(|| {
        let ang = rng.gen_range(0.0..std::f32::consts::TAU);
        Vec3::new(ang.cos(), 0.0, ang.sin())
    });
    for i in 0..n {
        let spread = Vec3::new(rng.gen_range(-1.0..1.0), rng.gen_range(-0.3..0.3), rng.gen_range(-1.0..1.0)) * 140.0;
        let mut pos = center + base_dir * dist + spread + Vec3::Y * (i as f32 * 25.0);
        if game.zone == Zone::Atmosphere {
            pos.x = pos.x.clamp(-ATMOS_LIMIT, ATMOS_LIMIT);
            pos.z = pos.z.clamp(-ATMOS_LIMIT, ATMOS_LIMIT);
            let g = ground_height(pos.x, pos.z);
            pos.y = pos.y.clamp(g + 300.0, CEILING - 450.0);
        }
        let e = spawn_fighter(commands, a, pos, look, game.wave.max(1));
        if has_carrier && rng.gen_bool(carrier_chance) {
            let s = CARRIER_SCALE;
            let aim = Vec3::new(rng.gen_range(-0.25..0.25) * s, rng.gen_range(0.0..0.08) * s, rng.gen_range(-0.75..0.75) * s);
            commands.entity(e).insert(AttackCarrier { aim });
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn mission_update(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    ma: Res<MissionAssets>,
    sounds: Res<Sounds>,
    diff: Res<Difficulty>,
    mut game: ResMut<Game>,
    mut m: ResMut<Mission>,
    mut radio: ResMut<Radio>,
    mut dir: ResMut<Director>,
    intro: Option<Res<Intro>>,
    player: Single<&Transform, With<Player>>,
    carrier: Query<&Transform, (With<CarrierShip>, Without<Player>)>,
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy, Has<Elite>), (Without<Player>, Without<CarrierShip>, Without<MotherShield>)>,
    mother_turrets: Query<(), With<MothershipTurret>>,
    shields: Query<Entity, With<MotherShield>>,
) {
    let dt = time.delta_secs();
    let in_intro = intro.is_some_and(|i| i.phase != IntroPhase::Off);
    if !game.started || in_intro {
        return;
    }
    m.banner = (m.banner - dt).max(0.0);
    game.jam = (game.jam - dt).max(0.0);
    let p = player.translation;
    let cpos = carrier.iter().next().map(|c| c.translation);
    let has_carrier = cpos.is_some();

    // ---- escudo de la nave nodriza Invid: activo mientras queden torretas
    let turrets_left = mother_turrets.iter().count();
    let mother = enemies.iter().find(|e| e.2.kind == EnemyKind::Mothership).map(|e| (e.0, e.2.half_extents.unwrap_or(Vec3::splat(300.0))));
    let shield_up = mother.is_some() && turrets_left > 0;
    if game.mother_shield && !shield_up && mother.is_some() {
        radio.urgent(Speaker::Control, "¡El escudo de la nave nodriza Invid ha caído! ¡Ataquen el casco!");
        play(&mut commands, &sounds.big_explosion, 0.6);
    }
    game.mother_shield = shield_up;
    match (shield_up, mother, shields.iter().next()) {
        (true, Some((me, half)), None) => {
            commands.entity(me).with_children(|c| {
                c.spawn((
                    Mesh3d(a.plain_sphere.clone()),
                    MeshMaterial3d(ma.shield.clone()),
                    Transform::from_scale(half * 1.25),
                    MotherShield,
                    NotShadowCaster,
                ));
            });
        }
        (false, _, Some(se)) => {
            commands.entity(se).try_despawn();
        }
        _ => {}
    }

    // ---- reinicio tras derribo o nave nodriza destruida (R)
    if game.fresh {
        game.fresh = false;
        if m.phase != Phase::Free {
            m.phase = Phase::Briefing;
            m.t = 0.0;
            m.events = 0;
        }
        radio.clear();
    }
    if game.dead {
        return;
    }
    m.t += dt;
    m.stats_time += dt;
    let kills = game.kills - m.base_kills;
    let turret_kills = game.turret_kills - m.base_turrets;

    match m.phase {
        Phase::Idle => {
            m.phase = Phase::Briefing;
            m.t = 0.0;
            m.events = 0;
        }
        Phase::Briefing => {
            if m.once(31) {
                // primer cuadro del briefing: preparar la misión
                m.step = 0;
                m.base_kills = game.kills;
                m.base_turrets = game.turret_kills;
                m.elite = None;
                m.last_beep = -1;
                game.auto_waves = false;
                let n = m.idx + 1;
                let title = MISSIONS[m.idx];
                m.banner(format!("MISIÓN {n}/5 · {title}"), format!("Dificultad {}", diff.name()), 4.5);
                play(&mut commands, &sounds.whoosh, 0.35);
                match m.idx {
                    0 => {
                        m.need = if diff.media() { 8 } else { 5 };
                        radio.say(Speaker::Control, "Escuadrón Orbital, aquí la nave nodriza. Firmas Invid entrando en el sector.");
                        radio.say(Speaker::Control, format!("Intercepten y derriben {} unidades. Repito: no dejen que se acerquen.", m.need));
                        radio.say(Speaker::Azul1, "Azul 1 en tu ala, líder. Vamos a por ellos.");
                        radio.say(Speaker::Control, "En tierra, los Destroid Spartan defienden cuatro sectores del desierto. Apóyalos si puedes.");
                    }
                    1 => {
                        m.timer = if diff.media() { 90.0 } else { 70.0 };
                        radio.say(Speaker::Control, "¡Atención! Un enjambre Invid viene directo hacia la nave nodriza.");
                        radio.say(Speaker::Control, format!("Necesitamos {:.0} segundos para cargar los cañones principales. ¡Aguanten!", m.timer));
                        radio.say(Speaker::Azul3, "Azul 3 y 4 cubriendo el casco. Líder, intercepta lo que se nos escape.");
                    }
                    2 => {
                        game.hive_alive = true;
                        radio.say(Speaker::Control, "Detectamos una colmena Invid en el desierto, al sureste. De ahí salen los exploradores.");
                        radio.say(Speaker::Control, "Destrúyanla. Está marcada en el HUD con un rombo naranja.");
                        radio.say(Speaker::Azul1, "Consejo, líder: baja a menos de 500 metros y pulsa G. En GERWALK o BATTROID puedes quedarte suspendido y apuntar mejor.");
                        if diff.plasma(m.idx) {
                            radio.say(Speaker::Alerta, "Inteligencia confirma plasma guiado Invid. Si suena la alarma: ¡turbo y viraje cerrado!");
                        }
                    }
                    3 => {
                        radio.say(Speaker::Control, "Contacto prioritario: un Comandante Invid se dirige a la nave nodriza.");
                        radio.say(Speaker::Control, "Si llega al casco, perderemos la nave. ¡Derríbenlo a toda costa!");
                        radio.say(Speaker::Azul1, "Tiene escolta. Nosotros nos encargamos de ellos: el grande es todo tuyo, líder.");
                    }
                    _ => {
                        radio.say(Speaker::Control, "Es la hora. La nave nodriza Invid está en órbita baja.");
                        radio.say(Speaker::Control, "Sube sobre los 3400 metros para salir a la órbita y destrúyela.");
                        radio.say(Speaker::Control, "Primero derriba sus torretas: mientras vivan, su escudo la protege.");
                    }
                }
            }
            if m.t > 4.0 {
                m.events = 0;
                m.phase = Phase::Active;
                m.t = 0.0;
                game.auto_waves = matches!(m.idx, 0 | 2 | 4);
                game.wave_timer = game.wave_timer.min(2.0);
            }
        }
        Phase::Active => {
            match m.idx {
                // ------------------------------------------- 1: primer contacto
                0 => {
                    m.objective = format!("Derriba Invid: {}/{}", kills.min(m.need), m.need);
                    if kills * 2 >= m.need && m.once(0) {
                        // emboscada por la cola
                        let back = -player.forward().as_vec3();
                        spawn_group(&mut commands, &a, &game, p, p, if diff.media() { 4 } else { 3 }, 650.0, Some(back), 0.0, has_carrier);
                        radio.urgent(Speaker::Azul1, "¡Contactos a las seis! ¡Rompe, rompe!");
                        play(&mut commands, &sounds.alarm, 0.5);
                        game.shake = game.shake.max(0.4);
                    }
                    if kills >= m.need {
                        complete(&mut m, &mut radio, &mut commands, &sounds, &mut dir, "Sector despejado. Buen trabajo, líder.");
                    }
                }
                // ------------------------------------------- 2: defender la nodriza
                1 => {
                    m.timer -= dt;
                    let pct = (game.carrier_hp / CARRIER_HP * 100.0).max(0.0);
                    m.objective = format!("Protege la nave nodriza: {:.0} s  ·  casco {:.0}%", m.timer.max(0.0), pct);
                    m.spawn_cd -= dt;
                    if m.spawn_cd <= 0.0 && m.timer > 8.0 {
                        m.spawn_cd = if diff.media() { 9.0 } else { 12.0 };
                        let c = cpos.unwrap_or(p);
                        spawn_group(&mut commands, &a, &game, c, c, if diff.media() { 4 } else { 3 }, 2600.0, None, 0.75, has_carrier);
                    }
                    let half = m.timer < 0.5 * if diff.media() { 90.0 } else { 70.0 };
                    if half && m.once(0) {
                        game.jam = 12.0;
                        radio.urgent(Speaker::Control, "¡Interferencia Invid! Perdemos el radar... ¡vuelen a la vista!");
                        play(&mut commands, &sounds.alarm, 0.4);
                    }
                    // a mitad de la defensa aparece el nuevo jefe: el Soldado de Choque
                    let tq = m.timer < 0.42 * if diff.media() { 90.0 } else { 70.0 };
                    if tq && m.once(2) {
                        if let Some(c) = cpos {
                            let ang = rand::thread_rng().gen_range(0.0..std::f32::consts::TAU);
                            let start = c + Vec3::new(ang.cos() * 2600.0, 350.0, ang.sin() * 2600.0);
                            let e = crate::mecha::spawn_shock(&mut commands, &a, start, c, diff.media());
                            let s = crate::models::CARRIER_SCALE;
                            commands.entity(e).insert(AttackCarrier { aim: Vec3::new(0.0, 0.04 * s, 0.1 * s) });
                            radio.say(Speaker::Alerta, "¡Un SOLDADO DE CHOQUE Invid va hacia la nodriza! ¡Su blindaje es enorme!");
                            play(&mut commands, &sounds.alarm, 0.5);
                        }
                    }
                    if pct < 40.0 && m.once(1) {
                        radio.urgent(Speaker::Control, "¡Casco de la nodriza al 40%! ¡No aguantaremos mucho más!");
                    }
                    countdown(&mut m, &mut commands, &sounds);
                    if m.timer <= 0.0 {
                        complete(&mut m, &mut radio, &mut commands, &sounds, &mut dir, "¡Cañones cargados! ¡Fuego!... El enjambre se retira. Lo lograron.");
                        // los cañones principales barren a los Invid restantes
                        let mut rng = rand::thread_rng();
                        for (e, tf, en, _) in &mut enemies {
                            if en.kind == EnemyKind::Fighter && rng.gen_bool(0.8) {
                                spawn_explosion(&mut commands, &a, tf.translation, 14.0, Vec3::ZERO);
                                commands.entity(e).try_despawn();
                            }
                        }
                        play(&mut commands, &sounds.big_explosion, 0.8);
                    }
                }
                // ------------------------------------------- 3: baterías
                2 => {
                    let _ = turret_kills;
                    let hive = enemies.iter().find(|e| e.2.kind == EnemyKind::Hive).map(|e| (e.1.translation, e.2.hp / e.2.max_hp));
                    if game.zone != Zone::Atmosphere {
                        m.objective = "Regresa a la atmósfera: la colmena está en tierra".into();
                    } else if let Some((hp_pos, frac)) = hive {
                        m.step = 1;
                        let d = hp_pos.distance(p);
                        m.objective = format!("Destruye la colmena Invid · {:.1} km · estructura {:.0}%", d / 1000.0, frac * 100.0);
                        if d < 2500.0 && m.once(0) {
                            radio.urgent(Speaker::Azul1, "¡Colmena a la vista! Está escupiendo exploradores, cuidado.");
                        }
                        if frac < 0.75 && m.once(1) {
                            let back = -player.forward().as_vec3();
                            spawn_group(&mut commands, &a, &game, p, p, if diff.media() { 4 } else { 2 }, 900.0, Some(back), 0.0, has_carrier);
                            radio.urgent(Speaker::Control, "¡La colmena pide refuerzos! ¡Interceptores tras de ti!");
                            play(&mut commands, &sounds.alarm, 0.5);
                        }
                        if frac < 0.5 && m.once(2) {
                            let e = crate::mecha::spawn_boss(&mut commands, &a, hp_pos + Vec3::Y * 260.0, p, diff.media());
                            let _ = e;
                            radio.urgent(Speaker::Alerta, "¡Un JEFE INVID emerge de la colmena! ¡Es mucho más fuerte que los exploradores!");
                            play(&mut commands, &sounds.big_explosion, 0.6);
                            game.shake = game.shake.max(0.8);
                        }
                    } else if m.step == 1 && !game.hive_alive {
                        complete(&mut m, &mut radio, &mut commands, &sounds, &mut dir, "¡Colmena destruida! Ya no saldrán más exploradores de ahí.");
                    } else {
                        m.objective = "Localizando la colmena...".into();
                    }
                }
                // ------------------------------------------- 4: el comandante
                3 => {
                    let c = cpos.unwrap_or(Vec3::new(0.0, 2650.0, 2400.0));
                    if m.step == 0 {
                        m.step = 1;
                        let start = c + Vec3::new(-2500.0, -400.0, -6400.0);
                        // el Comandante es un Jefe Invid
                        let e = crate::mecha::spawn_boss(&mut commands, &a, start, c, diff.media());
                        commands.entity(e).insert(Elite).with_children(|ch| {
                            ch.spawn((Mesh3d(a.plain_sphere.clone()), MeshMaterial3d(ma.aura.clone()), Transform::from_scale(Vec3::splat(20.0)), NotShadowCaster));
                        });
                        m.elite = Some(e);
                        spawn_group(&mut commands, &a, &game, start, p, if diff.media() { 5 } else { 3 }, 120.0, None, 0.0, has_carrier);
                    }
                    let elite = m.elite.and_then(|e| enemies.get_mut(e).ok());
                    match elite {
                        Some((_, mut tf, mut en, _)) => {
                            m.step = 2; // el Comandante ya existe en el mundo
                            // vuela directo a la nave nodriza con un leve zigzag
                            let speed = if diff.media() { 175.0 } else { 140.0 };
                            let to = c - tf.translation;
                            let d = to.length();
                            let weave = Vec3::new((m.t * 0.9).sin(), (m.t * 0.6).cos() * 0.3, 0.0) * 0.35;
                            let dirv = (to.normalize_or(Vec3::Z) + tf.rotation * weave).normalize();
                            let rot = Transform::IDENTITY.looking_to(dirv, Vec3::Y).rotation;
                            tf.rotation = tf.rotation.slerp(rot, (1.5 * dt).min(1.0));
                            let f = tf.forward().as_vec3();
                            en.vel = f * speed;
                            tf.translation += en.vel * dt;
                            m.objective = format!("Derriba al Comandante · distancia a la nodriza {:.1} km · blindaje {:.0}%", d / 1000.0, en.hp / en.max_hp * 100.0);
                            for (bit, km) in [(0u32, 3.0f32), (1, 2.0), (2, 1.0)] {
                                if d < km * 1000.0 && m.once(bit) {
                                    let txt = match bit {
                                        0 => "El Comandante a 3 kilómetros de la nave. ¡Intercéptenlo!",
                                        1 => "¡2 kilómetros! ¡Lo tenemos encima!",
                                        _ => "¡UN KILÓMETRO! ¡DERRÍBENLO YA!",
                                    };
                                    radio.urgent(Speaker::Control, txt);
                                    play(&mut commands, &sounds.alarm, 0.3 + bit as f32 * 0.15);
                                }
                            }
                            if d < 420.0 {
                                // impacto contra la nodriza
                                for _ in 0..6 {
                                    spawn_explosion(&mut commands, &a, tf.translation + Vec3::new(rand::random::<f32>() * 80.0 - 40.0, 0.0, rand::random::<f32>() * 80.0 - 40.0), 50.0, Vec3::ZERO);
                                }
                                play(&mut commands, &sounds.big_explosion, 1.0);
                                game.carrier_hp -= CARRIER_HP * 0.3 * diff.carrier_dmg().max(0.7);
                                game.shake = 1.5;
                                en.hp = 0.0;
                                fail(&mut m, &mut radio, "El Comandante impactó la nave nodriza. Misión fallida.");
                            }
                        }
                        None => {
                            if m.step == 2 {
                                complete(&mut m, &mut radio, &mut commands, &sounds, &mut dir, "¡Comandante abatido! Gran disparo, líder.");
                            }
                        }
                    }
                }
                // ------------------------------------------- 5: asalto orbital
                _ => {
                    if game.zone == Zone::Atmosphere {
                        m.objective = format!("Asciende a la órbita: {:.0} / 3400 m", p.y.max(0.0));
                    } else {
                        if m.once(0) {
                            radio.say(Speaker::Control, "Órbita alcanzada. Nave nodriza Invid al frente. Escudo activo.");
                            radio.say(Speaker::Azul1, "Te sigo, líder. Por la Tierra.");
                        }
                        if game.mother_shield {
                            m.objective = format!("Destruye las torretas del escudo: quedan {}", turrets_left);
                        } else if game.mothership_alive {
                            let hp = enemies.iter().find(|e| e.2.kind == EnemyKind::Mothership).map(|e| e.2.hp / e.2.max_hp * 100.0).unwrap_or(0.0);
                            m.objective = format!("Destruye la nave nodriza Invid · casco {:.0}%", hp);
                            if hp < 50.0 && m.once(1) {
                                radio.urgent(Speaker::Control, "¡Está perdiendo integridad! ¡Sigan disparando!");
                                let c = mother.map(|_| p).unwrap_or(p);
                                spawn_group(&mut commands, &a, &game, c, p, if diff.media() { 5 } else { 3 }, 800.0, None, 0.3, has_carrier);
                                play(&mut commands, &sounds.alarm, 0.5);
                            }
                        } else {
                            m.phase = Phase::Victory;
                            m.t = 0.0;
                            m.banner("¡VICTORIA!", "La flota Invid se retira de la órbita terrestre", 7.0);
                            radio.urgent(Speaker::Control, "¡Nave nodriza Invid destruida! ¡La Tierra está a salvo!");
                            radio.say(Speaker::Azul1, "Volvemos a casa, líder. Ha sido un honor.");
                            dir.slowmo = 2.0;
                            play(&mut commands, &sounds.whoosh, 0.6);
                        }
                    }
                }
            }
        }
        Phase::Complete => {
            if m.t > 6.0 {
                m.idx += 1;
                // recompensa: recarga completa
                game.missiles = 8;
                game.cluster = 3;
                game.shield = 100.0;
                game.hull = (game.hull + 30.0).min(100.0);
                m.phase = Phase::Briefing;
                m.t = 0.0;
                m.events = 0;
            }
        }
        Phase::Failed => {
            if m.t > 5.0 {
                // reintentar: limpiar el cielo y reparar algo la nodriza
                for (e, _, en, _) in &enemies {
                    if en.kind == EnemyKind::Fighter {
                        commands.entity(e).try_despawn();
                    }
                }
                game.carrier_hp = game.carrier_hp.max(CARRIER_HP * 0.6);
                m.phase = Phase::Briefing;
                m.t = 0.0;
                m.events = 0;
            }
        }
        Phase::Victory => {
            if m.t > 8.0 {
                m.phase = Phase::Free;
                game.auto_waves = true;
                m.objective = "Patrulla libre: oleadas infinitas".into();
                radio.say(Speaker::Control, "Quedan restos de la flota enemiga. Patrulla libre autorizada.");
            }
        }
        Phase::Free => {
            m.spawn_cd -= dt;
            if m.spawn_cd <= 0.0 {
                m.spawn_cd = if diff.media() { 75.0 } else { 110.0 };
                if m.t > 20.0 {
                    let back = -player.forward().as_vec3();
                    if rand::thread_rng().gen_bool(0.5) {
                        crate::mecha::spawn_shock(&mut commands, &a, p + back * 1500.0 + Vec3::Y * 200.0, p, diff.media());
                        radio.urgent(Speaker::Alerta, "¡Soldado de choque en el sector!");
                    } else {
                        crate::mecha::spawn_boss(&mut commands, &a, p + back * 1500.0 + Vec3::Y * 200.0, p, diff.media());
                        radio.urgent(Speaker::Alerta, "¡Jefe Invid en el sector!");
                    }
                    play(&mut commands, &sounds.alarm, 0.5);
                }
            }
        }
    }
}

fn complete(m: &mut Mission, radio: &mut Radio, commands: &mut Commands, sounds: &Sounds, dir: &mut Director, text: &str) {
    m.phase = Phase::Complete;
    m.t = 0.0;
    m.objective.clear();
    m.banner("MISIÓN CUMPLIDA", MISSIONS[m.idx], 4.5);
    radio.urgent(Speaker::Control, text);
    play(commands, &sounds.whoosh, 0.5);
    dir.slowmo = dir.slowmo.max(0.8);
}

fn fail(m: &mut Mission, radio: &mut Radio, text: &str) {
    m.phase = Phase::Failed;
    m.t = 0.0;
    m.objective.clear();
    m.banner("MISIÓN FALLIDA", "Reintentando...", 4.5);
    radio.urgent(Speaker::Alerta, text);
}

fn countdown(m: &mut Mission, commands: &mut Commands, sounds: &Sounds) {
    let s = m.timer.ceil() as i32;
    if (1..=10).contains(&s) && s != m.last_beep {
        m.last_beep = s;
        play(commands, &sounds.lock, 0.45);
    }
}

// ---------------------------------------------------------------- plasma guiado

pub fn plasma_fire(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    ma: Res<MissionAssets>,
    sounds: Res<Sounds>,
    diff: Res<Difficulty>,
    m: Res<Mission>,
    game: Res<Game>,
    player: Single<&Transform, With<Player>>,
    mut fighters: Query<(Entity, &Transform, &Enemy, Option<&mut PlasmaCd>), (Without<Player>, Without<Elite>, Without<AttackCarrier>)>,
    plasmas: Query<(), With<EnemyPlasma>>,
) {
    let free = m.phase == Phase::Free;
    if !diff.plasma(if free { 9 } else { m.idx }) || game.dead || !matches!(m.phase, Phase::Active | Phase::Free) {
        return;
    }
    let dt = time.delta_secs();
    let mut active = plasmas.iter().count();
    let mut rng = rand::thread_rng();
    for (e, tf, en, cd) in &mut fighters {
        if en.kind != EnemyKind::Fighter {
            continue;
        }
        let Some(mut cd) = cd else {
            commands.entity(e).insert(PlasmaCd(rng.gen_range(6.0..14.0)));
            continue;
        };
        cd.0 -= dt;
        let to = player.translation - tf.translation;
        let d = to.length();
        if cd.0 <= 0.0 && active < 2 && d > 450.0 && d < 1600.0 && tf.forward().angle_between(to) < 0.6 {
            cd.0 = rng.gen_range(16.0..26.0);
            active += 1;
            let pos = tf.translation + tf.forward() * 8.0;
            commands.spawn((
                Mesh3d(a.plain_sphere.clone()),
                MeshMaterial3d(ma.plasma.clone()),
                Transform::from_translation(pos).with_scale(Vec3::splat(2.6)),
                EnemyPlasma { vel: to.normalize() * 260.0, life: 9.0, trail: 0.0 },
                NotShadowCaster,
                ZoneEntity,
            ));
            play(&mut commands, &sounds.missile, 0.5);
        }
    }
}

pub fn plasma_update(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    sounds: Res<Sounds>,
    diff: Res<Difficulty>,
    mut game: ResMut<Game>,
    mut radio: ResMut<Radio>,
    mut beep: Local<f32>,
    mut warned: Local<bool>,
    player: Single<&Transform, With<Player>>,
    mut plasmas: Query<(Entity, &mut Transform, &mut EnemyPlasma), Without<Player>>,
    bullets: Query<(&Transform, &Bullet), (Without<Player>, Without<EnemyPlasma>)>,
) {
    let dt = time.delta_secs();
    let p = player.translation;
    let mut closest = f32::MAX;
    for (e, mut tf, mut pl) in &mut plasmas {
        pl.life -= dt;
        let to = p - tf.translation;
        let d = to.length();
        // giro limitado: un viraje cerrado o el turbo lo dejan atrás
        let speed = (pl.vel.length() + 25.0 * dt).min(370.0);
        let mut dirv = pl.vel.normalize_or(Vec3::Z);
        let want = to.normalize_or(dirv);
        let ang = dirv.angle_between(want);
        let max = 1.15 * dt;
        if ang > 1e-4 && !game.dead {
            dirv = dirv.slerp(want, (max / ang).min(1.0)).normalize();
        }
        pl.vel = dirv * speed;
        tf.translation += pl.vel * dt;
        let pulse = 2.6 + (time.elapsed_secs() * 18.0).sin() * 0.5;
        tf.scale = Vec3::splat(pulse);
        pl.trail -= dt;
        if pl.trail <= 0.0 {
            pl.trail = 0.05;
            spawn_puff(&mut commands, &a, tf.translation, 1.6, 0.7);
        }
        // se puede derribar con láseres
        let mut dead = pl.life <= 0.0;
        for (bt, b) in &bullets {
            if b.from_player && bt.translation.distance(tf.translation) < 6.0 {
                dead = true;
                game.message("¡Plasma interceptado!", 1.5);
            }
        }
        if !dead && d < 8.0 && !game.dead {
            game.damage(32.0 * diff.enemy_dmg());
            play(&mut commands, &sounds.explosion, 0.8);
            dead = true;
        }
        if game.zone == Zone::Atmosphere && tf.translation.y < ground_height(tf.translation.x, tf.translation.z) {
            dead = true;
        }
        if dead {
            spawn_explosion(&mut commands, &a, tf.translation, 8.0, Vec3::ZERO);
            commands.entity(e).try_despawn();
        } else if to.dot(pl.vel) > 0.0 {
            closest = closest.min(d);
        }
    }
    game.plasma_warn = if closest < 1600.0 { closest } else { 0.0 };
    // pitido de alerta que se acelera al acercarse
    if game.plasma_warn > 0.0 {
        if !*warned {
            *warned = true;
            radio.urgent(Speaker::Alerta, "¡PLASMA EN COLA! ¡Turbo y viraje cerrado!");
        }
        *beep -= dt;
        if *beep <= 0.0 {
            *beep = 0.08 + game.plasma_warn / 1600.0 * 0.55;
            play(&mut commands, &sounds.lock, 0.35);
        }
    } else {
        *warned = false;
    }
}

/// Marcadores de objetivos en el HUD: comandante, torretas, plasma.
pub fn objective_markers(
    game: Res<Game>,
    m: Res<Mission>,
    dir: Res<Director>,
    time: Res<Time<Real>>,
    player: Single<&Transform, With<Player>>,
    enemies: Query<(&Transform, &Enemy, Has<Elite>), Without<Player>>,
    plasmas: Query<&Transform, (With<EnemyPlasma>, Without<Player>)>,
    mut g: Gizmos,
) {
    if !game.started || game.dead {
        return;
    }
    let cam = dir.cam.translation;
    let cf = dir.cam.forward().as_vec3();
    let blink = (time.elapsed_secs() * 6.0).sin() > 0.0;
    let mut mark = |pos: Vec3, k: f32, col: Color, excl: bool| {
        if (pos - cam).dot(cf) <= 0.0 {
            return;
        }
        let view_dir = (cam - pos).normalize_or(Vec3::Z);
        let rot = Quat::from_rotation_arc(Vec3::Z, view_dir);
        let (rr, ru) = (rot * Vec3::X, rot * Vec3::Y);
        let top = pos + ru * k * 2.2;
        g.linestrip([top + ru * k, top + rr * k, top - ru * k, top - rr * k, top + ru * k], col);
        if excl {
            g.line(top + ru * k * 0.6, top - ru * k * 0.1, col);
            g.line(top - ru * k * 0.35, top - ru * k * 0.5, col);
        }
    };
    let red = Color::LinearRgba(LinearRgba::new(4.0, 0.4, 0.3, 1.0));
    let orange = Color::LinearRgba(LinearRgba::new(3.0, 1.6, 0.2, 1.0));
    for (tf, en, elite) in &enemies {
        let d = tf.translation.distance(cam);
        let k = (d * 0.022).max(4.0);
        if elite && blink {
            mark(tf.translation, k * 1.3, red, true);
        } else if m.idx == 2 && m.phase == Phase::Active && en.kind == EnemyKind::Hive {
            mark(tf.translation + Vec3::Y * 200.0, k * 1.5, orange, false);
        } else if m.idx == 4 && game.mother_shield && en.kind == EnemyKind::Turret && game.zone == Zone::Space {
            mark(tf.translation, k, orange, false);
        }
    }
    for tf in &plasmas {
        let d = tf.translation.distance(cam);
        if blink {
            mark(tf.translation, (d * 0.02).max(3.0), red, true);
        }
    }
    let _ = player;
}

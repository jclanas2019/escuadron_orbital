//! Audio cinematográfico por capas (se suma a los sonidos de siempre de `audio.rs`):
//! * explosiones cercanas / lejanas / enormes con subgraves, crepitar y reverberación,
//!   posicionadas en estéreo según la cámara y con retraso del sonido en la atmósfera;
//! * lanzamiento de misiles, salvas en racimo, pasadas de misiles, impactos metálicos,
//!   servos de la transformación;
//! * ambientes en bucle (cabina, viento, estática de radio);
//! * diálogos de radio y gritos de combate en japonés (con subtítulos en español).

use std::collections::HashMap;

use bevy::audio::{PlaybackMode, SpatialListener, Volume};
use bevy::prelude::*;
use rand::Rng;

use crate::audio::AssetsDir;
use crate::components::*;

/// Archivos de `assets/audio/capas/` (si falta alguno simplemente no se usa).
const CAPAS: &[&str] = &[
    "explosion_cerca_1", "explosion_cerca_2", "explosion_cerca_3", "explosion_lejos_1", "explosion_lejos_2", "explosion_enorme",
    "micro_explosion_1", "micro_explosion_2", "micro_explosion_3", "misil_lanzamiento_1", "misil_lanzamiento_2", "racimo_lanzamiento",
    "paso_misil", "radio_abre", "radio_cierra", "radio_estatica", "cabina_ambiente", "viento", "laser_capa", "impacto_metal_1",
    "impacto_metal_2", "transformacion", "clip_explosion_1", "clip_explosion_2", "clip_explosion_3", "clip_impacto", "clip_enjambre",
    "clip_lanzamiento",
];

#[derive(Resource, Default)]
pub struct Capas {
    /// grupo ("explosion_cerca") → variantes
    pub grupos: HashMap<String, Vec<Handle<AudioSource>>>,
    /// prefijo del subtítulo / id de grito → (audio, duración)
    pub voces: Vec<(&'static str, Handle<AudioSource>, f32)>,
}

impl Capas {
    pub fn pick(&self, g: &str) -> Option<Handle<AudioSource>> {
        let v = self.grupos.get(g)?;
        if v.is_empty() {
            return None;
        }
        Some(v[rand::thread_rng().gen_range(0..v.len())].clone())
    }
    pub fn voice_for(&self, text: &str) -> Option<(Handle<AudioSource>, f32)> {
        self.voces.iter().find(|(k, _, _)| text.starts_with(k)).map(|(_, h, d)| (h.clone(), *d))
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum SfxKind {
    Explosion { size: f32 },
    MissileLaunch,
    ClusterLaunch,
    Flyby,
    LaserLayer,
    Hit,
    Transform,
    /// Grito de combate en japonés (id de `voces.rs`, p. ej. "#p_misil").
    Bark(&'static str),
}

/// Petición de sonido (se procesa en `sfx_dispatch`).
#[derive(Component)]
pub struct Sfx {
    pub kind: SfxKind,
    pub pos: Option<Vec3>,
    pub vol: f32,
    pub delay: f32,
}

pub fn sfx(commands: &mut Commands, kind: SfxKind, pos: Option<Vec3>, vol: f32) {
    commands.spawn(Sfx { kind, pos, vol, delay: 0.0 });
}

pub fn bark(commands: &mut Commands, id: &'static str) {
    sfx(commands, SfxKind::Bark(id), None, 1.0);
}

/// Emisor espacial: se mantiene pegado a la cámara en la dirección de la fuente,
/// así el volumen lo controlamos nosotros y el motor de audio sólo hace el paneo.
#[derive(Component)]
pub struct SpatialPin(pub Vec3);

#[derive(Component)]
pub struct AmbLoop(pub u8);

/// Volúmenes que el juego quiere para los bucles (música, motor, ambientes). Sirve para
/// reconstruir la mezcla al grabar la demo sin tarjeta de sonido.
#[derive(Resource, Default)]
pub struct MixTargets {
    /// (pista 0 título / 1 combate / 2 clímax, nivel)
    pub music: (u8, f32),
    pub engine: (f32, f32),
    pub amb: [f32; 3],
    pub wind_speed: f32,
    /// capas de la música adaptativa y cuánto se baja la orquestal
    pub adapt: [f32; 4],
    pub duck: f32,
}

#[derive(Resource, Default)]
pub struct SfxState {
    pub bark_cd: f32,
    pub huge_cd: f32,
    pub expl_this_frame: u32,
    pub voice: Option<Entity>,
    pub speaking: bool,
}

pub fn setup_capas(mut commands: Commands, server: Res<AssetServer>, dir: Res<AssetsDir>, cams: Query<Entity, With<WorldCam>>) {
    let mut c = Capas::default();
    for name in CAPAS {
        let rel = format!("audio/capas/{name}.ogg");
        if !dir.0.join(&rel).exists() {
            continue;
        }
        let g = name.trim_end_matches(|ch: char| ch.is_ascii_digit()).trim_end_matches('_').to_string();
        c.grupos.entry(g).or_default().push(server.load(rel));
    }
    for (k, file, d, _) in crate::voces::VOCES {
        let rel = format!("audio/voz/{file}.ogg");
        if dir.0.join(&rel).exists() {
            c.voces.push((k, server.load(rel), *d));
        }
    }
    // bucles de ambiente (0 = cabina, 1 = viento, 2 = estática de radio)
    for (i, g) in ["cabina_ambiente", "viento", "radio_estatica"].iter().enumerate() {
        if let Some(h) = c.pick(g) {
            commands.spawn((AudioPlayer::new(h), PlaybackSettings { volume: Volume::Linear(0.0), ..PlaybackSettings::LOOP }, AmbLoop(i as u8)));
        }
    }
    for cam in &cams {
        commands.entity(cam).insert(SpatialListener::new(2.0));
    }
    commands.insert_resource(c);
    commands.insert_resource(SfxState::default());
}

fn emit(commands: &mut Commands, h: Handle<AudioSource>, vol: f32, speed: f32, world: Option<Vec3>, cam: &Transform) {
    let mut s = PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(vol), speed, ..default() };
    match world {
        Some(wp) => {
            s.spatial = true;
            let p = cam.translation + (wp - cam.translation).normalize_or(cam.forward().as_vec3()) * 0.5;
            commands.spawn((AudioPlayer::new(h), s, Transform::from_translation(p), SpatialPin(wp)));
        }
        None => {
            commands.spawn((AudioPlayer::new(h), s));
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn sfx_dispatch(
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Sfx)>,
    capas: Option<Res<Capas>>,
    mut st: ResMut<SfxState>,
    dir: Res<Director>,
    mut game: ResMut<Game>,
    radio: Res<crate::missions::Radio>,
    pins: Query<(), With<SpatialPin>>,
) {
    let dt = time.delta_secs();
    st.bark_cd -= dt;
    st.huge_cd -= dt;
    st.expl_this_frame = 0;
    let Some(capas) = capas else {
        for (e, _) in &q {
            commands.entity(e).despawn();
        }
        return;
    };
    let mut rng = rand::thread_rng();
    let cam = dir.cam;
    let atmo = game.zone == Zone::Atmosphere;
    let mut active = pins.iter().count();
    for (e, mut r) in &mut q {
        if r.delay > 0.0 {
            r.delay -= dt;
            continue;
        }
        commands.entity(e).despawn();
        let d = r.pos.map(|p| p.distance(cam.translation)).unwrap_or(0.0);
        match r.kind {
            SfxKind::Explosion { size } => {
                // retraso del sonido en la atmósfera: primero el destello, luego el trueno
                if atmo && d > 450.0 && r.delay == 0.0 {
                    commands.spawn(Sfx { kind: r.kind, pos: r.pos, vol: r.vol, delay: (d / 340.0).min(2.5) - 0.0001 });
                    continue;
                }
                if st.expl_this_frame >= 3 || active > 28 {
                    continue;
                }
                st.expl_this_frame += 1;
                active += 1;
                let speed = (1.12 - size / 120.0).clamp(0.8, 1.15) * rng.gen_range(0.93..1.07);
                let att = 1.0 / (1.0 + d / 350.0);
                if size < 6.0 {
                    if let Some(h) = capas.pick("micro_explosion") {
                        emit(&mut commands, h, 0.55 * att * r.vol, rng.gen_range(0.9..1.15), r.pos, &cam);
                    }
                } else if d < 1300.0 {
                    if let Some(h) = capas.pick("explosion_cerca") {
                        emit(&mut commands, h, (0.95 * att * r.vol).min(1.0), speed, r.pos, &cam);
                    }
                    if d < 700.0 && rng.gen_bool(0.6) {
                        if let Some(h) = capas.pick("clip_explosion") {
                            emit(&mut commands, h, 0.45 * att * r.vol, speed, r.pos, &cam);
                        }
                    }
                    if d < size * 14.0 {
                        game.shake = (game.shake + 0.35 * (1.0 - d / (size * 14.0))).min(1.5);
                    }
                }
                if d > 700.0 {
                    if let Some(h) = capas.pick("explosion_lejos") {
                        let v = 0.9 / (1.0 + (d - 700.0) / 1600.0);
                        emit(&mut commands, h, v * r.vol, speed, r.pos, &cam);
                    }
                }
                if size >= 40.0 && st.huge_cd <= 0.0 {
                    st.huge_cd = 2.0;
                    if let Some(h) = capas.pick("explosion_enorme") {
                        emit(&mut commands, h, 1.0, 1.0, None, &cam);
                    }
                }
            }
            SfxKind::MissileLaunch => {
                if let Some(h) = capas.pick("misil_lanzamiento") {
                    emit(&mut commands, h, 0.55 * r.vol, rng.gen_range(0.95..1.08), r.pos, &cam);
                }
                if let Some(h) = capas.pick("clip_lanzamiento") {
                    emit(&mut commands, h, 0.25 * r.vol, 1.0, r.pos, &cam);
                }
            }
            SfxKind::ClusterLaunch => {
                let att = 1.0 / (1.0 + d / 400.0);
                if let Some(h) = capas.pick("racimo_lanzamiento") {
                    emit(&mut commands, h, 0.8 * att * r.vol, 1.0, if d < 30.0 { None } else { r.pos }, &cam);
                }
                if let Some(h) = capas.pick("clip_enjambre") {
                    emit(&mut commands, h, 0.35 * att * r.vol, 1.0, if d < 30.0 { None } else { r.pos }, &cam);
                }
            }
            SfxKind::Flyby => {
                if active < 28 {
                    active += 1;
                    if let Some(h) = capas.pick("paso_misil") {
                        emit(&mut commands, h, 0.45 * r.vol, rng.gen_range(0.9..1.2), r.pos, &cam);
                    }
                }
            }
            SfxKind::LaserLayer => {
                if let Some(h) = capas.pick("laser_capa") {
                    emit(&mut commands, h, 0.22 * r.vol, rng.gen_range(0.95..1.05), None, &cam);
                }
            }
            SfxKind::Hit => {
                if let Some(h) = capas.pick("impacto_metal") {
                    emit(&mut commands, h, 0.6 * r.vol, rng.gen_range(0.9..1.1), None, &cam);
                }
                if rng.gen_bool(0.5) {
                    if let Some(h) = capas.pick("clip_impacto") {
                        emit(&mut commands, h, 0.3 * r.vol, 1.0, None, &cam);
                    }
                }
            }
            SfxKind::Transform => {
                if let Some(h) = capas.pick("transformacion") {
                    emit(&mut commands, h, 0.7 * r.vol, 1.0, None, &cam);
                }
            }
            SfxKind::Bark(id) => {
                // no pisar los mensajes de radio ni repetir gritos seguidos
                if st.bark_cd > 0.0 || radio.current.is_some() || !radio.queue.is_empty() {
                    continue;
                }
                if let Some((h, dur)) = capas.voice_for(id) {
                    st.bark_cd = dur + 2.5;
                    let helmet = id.starts_with("#p_");
                    if !helmet {
                        if let Some(c) = capas.pick("radio_abre") {
                            emit(&mut commands, c, 0.35, 1.0, None, &cam);
                        }
                    }
                    emit(&mut commands, h, if helmet { 0.85 } else { 0.8 }, 1.0, None, &cam);
                }
            }
        }
    }
}

/// Mantiene los emisores espaciales en la dirección de su fuente respecto a la cámara.
pub fn spatial_pins(dir: Res<Director>, mut q: Query<(&SpatialPin, &mut Transform)>) {
    let cam = dir.cam;
    for (pin, mut tf) in &mut q {
        tf.translation = cam.translation + (pin.0 - cam.translation).normalize_or(cam.forward().as_vec3()) * 0.5;
    }
}

/// Voz de radio en japonés al empezar cada mensaje (se llama desde `radio_update`).
pub fn radio_voice(commands: &mut Commands, capas: Option<&Capas>, st: &mut SfxState, text: Option<&str>) -> Option<f32> {
    let capas = capas?;
    if let Some(v) = st.voice.take() {
        commands.entity(v).try_despawn();
    }
    let text = text?;
    let (h, dur) = capas.voice_for(text)?;
    if let Some(c) = capas.pick("radio_abre") {
        commands.spawn((AudioPlayer::new(c), PlaybackSettings { volume: Volume::Linear(0.4), ..PlaybackSettings::DESPAWN }));
    }
    let id = commands.spawn((AudioPlayer::new(h), PlaybackSettings { volume: Volume::Linear(0.95), ..PlaybackSettings::DESPAWN })).id();
    st.voice = Some(id);
    st.speaking = true;
    Some(dur)
}

pub fn radio_close(commands: &mut Commands, capas: Option<&Capas>, st: &mut SfxState) {
    if !st.speaking {
        return;
    }
    st.speaking = false;
    if let Some(v) = st.voice.take() {
        commands.entity(v).try_despawn();
    }
    if let Some(c) = capas.and_then(|c| c.pick("radio_cierra")) {
        commands.spawn((AudioPlayer::new(c), PlaybackSettings { volume: Volume::Linear(0.35), ..PlaybackSettings::DESPAWN }));
    }
}

/// Ambientes: zumbido de cabina en las vistas interiores, viento en la atmósfera según la
/// velocidad, y estática bajo las voces de radio.
pub fn ambience(
    time: Res<Time<Real>>,
    game: Res<Game>,
    dir: Res<Director>,
    st: Res<SfxState>,
    player: Single<&Player>,
    mut q: Query<(&AmbLoop, &mut AudioSink)>,
    mut mix: ResMut<MixTargets>,
) {
    let k = 1.0 - (-3.0 * time.delta_secs()).exp();
    let inside = matches!(dir.cut.map(|c| c.0).unwrap_or(dir.view), CamView::Pov | CamView::Shoulder | CamView::FacePilot | CamView::StickClose);
    let on = game.started && !game.paused;
    let target_of = |l: u8| -> f32 {
        if !on {
            0.0
        } else {
            match l {
                0 => {
                    if inside {
                        0.32
                    } else {
                        0.06
                    }
                }
                1 => {
                    if game.zone == Zone::Atmosphere && !game.dead {
                        (player.speed / 600.0).clamp(0.05, 1.0) * if inside { 0.18 } else { 0.42 }
                    } else {
                        0.0
                    }
                }
                _ => {
                    if st.speaking {
                        0.05
                    } else {
                        0.0
                    }
                }
            }
        }
    };
    let wind_speed = 0.8 + (player.speed / 900.0).min(0.6);
    mix.amb = [target_of(0), target_of(1), target_of(2)];
    mix.wind_speed = wind_speed;
    for (l, mut sink) in &mut q {
        let target = target_of(l.0);
        let cur = sink.volume().to_linear();
        sink.set_volume(Volume::Linear(cur + (target - cur) * k));
        if l.0 == 1 {
            sink.set_speed(wind_speed);
        }
    }
}

/// Charla de combate: cada 15-30 s, si hay Invid cerca, algún piloto del Escuadrón Azul
/// (o tú) grita algo por radio.
pub fn combat_chatter(
    time: Res<Time>,
    mut commands: Commands,
    game: Res<Game>,
    mut t: Local<f32>,
    player: Single<&Transform, With<Player>>,
    enemies: Query<&Transform, With<Enemy>>,
    allies: Query<(), With<crate::allies::Ally>>,
) {
    if !game.started || game.paused || game.dead {
        return;
    }
    *t -= time.delta_secs();
    if *t > 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    *t = rng.gen_range(15.0..30.0);
    let p = player.translation;
    let near = enemies.iter().filter(|e| e.translation.distance(p) < 1500.0).count();
    if near == 0 {
        return;
    }
    let lines: &[&'static str] = if allies.is_empty() {
        &["#p_escapa", "#p_caer", "#p_aguanta"]
    } else if near >= 4 {
        &["#a2_rapido", "#a3_cubre", "#a1_izq", "#a4_engage", "#p_aguanta"]
    } else {
        &["#a1_cubre", "#a2_engage", "#a4_engage", "#p_escapa", "#p_caer", "#a3_cubre"]
    };
    bark(&mut commands, lines[rng.gen_range(0..lines.len())]);
}

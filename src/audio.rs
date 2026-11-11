//! Sonidos generados proceduralmente (sin archivos externos): láseres, explosiones, motor...

use bevy::audio::Volume;
use bevy::prelude::*;
use std::f32::consts::TAU;
use std::sync::Arc;

const RATE: u32 = 22050;

#[derive(Resource, Clone)]
pub struct Sounds {
    pub laser: Handle<AudioSource>,
    pub enemy_laser: Handle<AudioSource>,
    pub flak: Handle<AudioSource>,
    pub explosion: Handle<AudioSource>,
    pub big_explosion: Handle<AudioSource>,
    pub hit: Handle<AudioSource>,
    pub missile: Handle<AudioSource>,
    pub lock: Handle<AudioSource>,
    pub alarm: Handle<AudioSource>,
    pub engine: Handle<AudioSource>,
    pub whoosh: Handle<AudioSource>,
    /// Golpe de bombo extraído de la canción (capa grave de las explosiones).
    pub kick: Handle<AudioSource>,
    /// Platillo extraído de la canción (chasquido de los impactos).
    pub cymbal: Handle<AudioSource>,
}

/// Carpeta de assets encontrada al arrancar (para saber qué .ogg existen).
#[derive(Resource, Clone)]
pub struct AssetsDir(pub std::path::PathBuf);

/// Música: pista del título y pista de combate (se mezclan según el estado).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Track {
    Titulo,
    Combate,
    Climax,
}

#[derive(Component)]
pub struct MusicTrack(pub Track);

#[derive(Resource)]
pub struct MusicOn(pub bool);

#[derive(Component)]
pub struct EngineSound;

/// Generador pseudoaleatorio simple y determinista para el ruido.
struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    }
}

fn to_wav(samples: &[f32]) -> AudioSource {
    let data_len = (samples.len() * 2) as u32;
    let mut b: Vec<u8> = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32000.0) as i16;
        b.extend_from_slice(&v.to_le_bytes());
    }
    AudioSource {
        bytes: Arc::from(b.into_boxed_slice()),
    }
}

fn n(secs: f32) -> usize {
    (secs * RATE as f32) as usize
}

/// Barrido de frecuencia con onda cuadrada suavizada (láser).
fn zap(f0: f32, f1: f32, dur: f32, vol: f32) -> Vec<f32> {
    let len = n(dur);
    let mut phase = 0.0f32;
    (0..len)
        .map(|i| {
            let t = i as f32 / len as f32;
            let f = f0 * (f1 / f0).powf(t);
            phase += f / RATE as f32;
            let s = (phase * TAU).sin();
            let sq = (s * 3.0).tanh();
            sq * (1.0 - t).powf(1.5) * vol * (i as f32 / 60.0).min(1.0)
        })
        .collect()
}

/// Ruido filtrado con caída: explosiones e impactos.
fn boom(dur: f32, cutoff: f32, low_hz: f32, vol: f32, seed: u32) -> Vec<f32> {
    let len = n(dur);
    let mut rng = Lcg(seed);
    let mut lp = 0.0f32;
    let mut lp2 = 0.0f32;
    let a = (cutoff / RATE as f32).min(1.0);
    (0..len)
        .map(|i| {
            let t = i as f32 / len as f32;
            let tt = i as f32 / RATE as f32;
            let a_now = a * (1.0 - t * 0.85);
            lp += (rng.next() - lp) * a_now;
            lp2 += (lp - lp2) * a_now;
            let rumble = (tt * low_hz * TAU * (1.0 - 0.4 * t)).sin() * 0.6;
            let env = (1.0 - t).powf(2.2) * (i as f32 / 200.0).min(1.0);
            ((lp2 * 4.0 + rumble) * env * vol).tanh()
        })
        .collect()
}

fn beep(f: f32, dur: f32, vol: f32) -> Vec<f32> {
    let len = n(dur);
    (0..len)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let env = (1.0 - i as f32 / len as f32).min((i as f32 / 100.0).min(1.0));
            ((t * f * TAU).sin() * 2.0).tanh() * env * vol
        })
        .collect()
}

fn whoosh(dur: f32, vol: f32, seed: u32) -> Vec<f32> {
    let len = n(dur);
    let mut rng = Lcg(seed);
    let mut lp = 0.0f32;
    (0..len)
        .map(|i| {
            let t = i as f32 / len as f32;
            let a = 0.02 + 0.25 * t;
            lp += (rng.next() - lp) * a;
            let env = (t * 8.0).min(1.0) * (1.0 - t).powf(1.2);
            lp * env * vol * 3.0
        })
        .collect()
}

/// Bucle de motor: componentes periódicos que encajan exactamente en el bucle.
fn engine_loop() -> Vec<f32> {
    let dur = 2.0;
    let len = n(dur);
    let mut rng = Lcg(77);
    let mut lp = 0.0f32;
    let mut noise: Vec<f32> = (0..len)
        .map(|_| {
            lp += (rng.next() - lp) * 0.03;
            lp
        })
        .collect();
    // Fundido cruzado del ruido para que el bucle no tenga clics.
    let fade = n(0.25);
    for i in 0..fade {
        let w = i as f32 / fade as f32;
        let j = len - fade + i;
        noise[i] = noise[i] * w + noise[j] * (1.0 - w);
    }
    noise.truncate(len - fade);
    let len = noise.len();
    let d = len as f32 / RATE as f32;
    let f1 = (55.0 * d).round() / d;
    let f2 = (82.0 * d).round() / d;
    let f3 = (110.0 * d).round() / d;
    (0..len)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let s = (t * f1 * TAU).sin() * 0.35
                + (t * f2 * TAU).sin() * 0.18
                + ((t * f3 * TAU).sin() * 3.0).tanh() * 0.08
                + noise[i] * 3.5;
            s * 0.5
        })
        .collect()
}

/// Sonidos sintetizados por código: se usan si falta el .ogg correspondiente
/// y se pueden exportar con `--exportar-sonidos carpeta`.
pub fn procedural_bank() -> Vec<(&'static str, Vec<f32>)> {
    let mut alarm = beep(880.0, 0.12, 0.5);
    alarm.extend(vec![0.0; n(0.06)]);
    alarm.extend(beep(880.0, 0.12, 0.5));
    let mut big = boom(2.6, 2500.0, 38.0, 1.3, 9);
    let tail = boom(2.6, 900.0, 30.0, 0.9, 13);
    for (a, b) in big.iter_mut().zip(tail.iter()) {
        *a = (*a + *b * 0.7).clamp(-1.0, 1.0);
    }
    let mut hit = boom(0.18, 9000.0, 140.0, 0.9, 5);
    for (i, s) in hit.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        *s += (t * 1900.0 * TAU).sin() * (1.0 - t / 0.18).max(0.0) * 0.25;
    }
    vec![
        ("laser", zap(2200.0, 380.0, 0.16, 0.45)),
        ("laser_enemigo", zap(1100.0, 180.0, 0.22, 0.35)),
        ("antiaereo", boom(0.35, 5000.0, 90.0, 0.7, 3)),
        ("explosion", boom(1.3, 4000.0, 55.0, 1.1, 7)),
        ("explosion_grande", big),
        ("impacto", hit),
        ("misil", whoosh(1.0, 0.8, 11)),
        ("fijado", beep(1320.0, 0.07, 0.35)),
        ("alarma", alarm),
        ("motor", engine_loop()),
        ("transicion", whoosh(2.2, 0.9, 21)),
        ("golpe_bombo", boom(0.4, 600.0, 50.0, 1.0, 17)),
        ("platillo", boom(0.6, 12000.0, 400.0, 0.6, 19)),
    ]
}

/// Escribe los sonidos sintetizados como .wav (para convertirlos a .ogg).
pub fn export_procedural(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, samples) in procedural_bank() {
        std::fs::write(dir.join(format!("{name}.wav")), &*to_wav(&samples).bytes)?;
    }
    Ok(())
}

pub fn setup_sounds(
    mut commands: Commands,
    mut audio: ResMut<Assets<AudioSource>>,
    server: Res<AssetServer>,
    dir: Res<AssetsDir>,
) {
    let bank = procedural_bank();
    let mut get = |name: &str| -> Handle<AudioSource> {
        let rel = format!("audio/sfx/{name}.ogg");
        if dir.0.join(&rel).exists() {
            server.load(rel)
        } else {
            let s = &bank.iter().find(|b| b.0 == name).expect("sonido").1;
            audio.add(to_wav(s))
        }
    };
    let sounds = Sounds {
        laser: get("laser"),
        enemy_laser: get("laser_enemigo"),
        flak: get("antiaereo"),
        explosion: get("explosion"),
        big_explosion: get("explosion_grande"),
        hit: get("impacto"),
        missile: get("misil"),
        lock: get("fijado"),
        alarm: get("alarma"),
        engine: get("motor"),
        whoosh: get("transicion"),
        kick: get("golpe_bombo"),
        cymbal: get("platillo"),
    };
    commands.spawn((
        AudioPlayer::new(sounds.engine.clone()),
        PlaybackSettings {
            volume: Volume::Linear(0.0),
            ..PlaybackSettings::LOOP
        },
        EngineSound,
    ));
    // música (si existen los archivos)
    for (file, track) in [
        ("audio/musica/musica_titulo.ogg", Track::Titulo),
        ("audio/musica/musica_completa.ogg", Track::Combate),
        ("audio/musica/musica_climax.ogg", Track::Climax),
    ] {
        if dir.0.join(file).exists() {
            commands.spawn((
                AudioPlayer::new(server.load(file)),
                PlaybackSettings {
                    volume: Volume::Linear(0.0),
                    ..PlaybackSettings::LOOP
                },
                MusicTrack(track),
            ));
        }
    }
    commands.insert_resource(sounds);
}

/// Mezcla la música: tema tranquilo en el título, la pista completa en combate
/// y el clímax orquestal en los momentos decisivos (Comandante y asalto orbital).
/// Tecla M: silenciar / activar la música.
pub fn music_mix(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut on: ResMut<MusicOn>,
    mut game: ResMut<crate::components::Game>,
    mission: Res<crate::missions::Mission>,
    intro: Option<Res<crate::models::Intro>>,
    tracks: Query<(&MusicTrack, &mut AudioSink)>,
    sfx_state: Option<Res<crate::sfx::SfxState>>,
    mut mix: ResMut<crate::sfx::MixTargets>,
    adapt: Option<Res<Adaptativa>>,
) {
    use crate::missions::Phase;
    if keys.just_pressed(KeyCode::KeyM) {
        on.0 = !on.0;
        game.message(if on.0 { "Música: ACTIVADA" } else { "Música: SILENCIADA" }, 1.5);
    }
    let in_intro = intro.is_some_and(|i| i.phase != crate::models::IntroPhase::Off) || !game.started;
    let climax = !in_intro
        && ((mission.idx == 3 && mission.phase == Phase::Active)
            || (mission.idx == 4 && matches!(mission.phase, Phase::Active | Phase::Victory) && game.zone == crate::components::Zone::Space));
    let wanted = if in_intro { Track::Titulo } else if climax { Track::Climax } else { Track::Combate };
    let level = if !on.0 {
        0.0
    } else if in_intro {
        0.55
    } else if game.paused {
        0.12
    } else if game.dead {
        0.18
    } else {
        0.42
    };
    // la música baja un poco mientras habla la radio
    let level = if sfx_state.is_some_and(|s| s.speaking) && !in_intro { level * 0.62 } else { level };
    let level = level * adapt.map(|a| a.duck).unwrap_or(1.0);
    mix.music = (match wanted { Track::Titulo => 0, Track::Combate => 1, Track::Climax => 2 }, level);
    let k = 1.0 - (-1.8 * time.delta_secs()).exp();
    for (t, mut sink) in tracks {
        let target = if t.0 == wanted { level } else { 0.0 };
        let cur = sink.volume().to_linear();
        sink.set_volume(Volume::Linear(cur + (target - cur) * k));
    }
}

pub fn play(commands: &mut Commands, h: &Handle<AudioSource>, vol: f32) {
    commands.spawn((
        AudioPlayer::new(h.clone()),
        PlaybackSettings {
            volume: Volume::Linear(vol),
            ..PlaybackSettings::DESPAWN
        },
    ));
}

// ---------------------------------------------------------------- música adaptativa

/// Capas de la música adaptativa (Battle Stations): 0 suspenso, 1 batalla (armonía),
/// 2 batalla (percusión), 3 crescendo.
#[derive(Component)]
pub struct AdaptiveLayer(pub u8);

#[derive(Component)]
pub struct AdaptiveStinger;

/// Estado de la mezcla adaptativa.
#[derive(Resource, Default)]
pub struct Adaptativa {
    /// 0..1: cuánto peligro hay ahora mismo
    pub peligro: f32,
    /// 0..1: contactos acercándose (antes de la batalla)
    pub tension: f32,
    /// cuánto se baja la música orquestal para dejar sitio a las capas
    pub duck: f32,
    pub niveles: [f32; 4],
    pub stinger_cd: f32,
    pub alto: bool,
    /// la batalla "dura" unos segundos después del último Invid cercano (evita vaivenes)
    pub batalla_t: f32,
}

const CAPAS_MUSICA: [&str; 4] = ["bs_suspenso", "bs_batalla_armonia", "bs_batalla_percusion", "bs_crescendo"];

pub fn setup_adaptativa(mut commands: Commands, server: Res<AssetServer>, dir: Res<AssetsDir>) {
    for (i, n) in CAPAS_MUSICA.iter().enumerate() {
        let rel = format!("audio/adaptativa/{n}.ogg");
        if dir.0.join(&rel).exists() {
            commands.spawn((AudioPlayer::new(server.load(rel)), PlaybackSettings { volume: Volume::Linear(0.0), ..PlaybackSettings::LOOP }, AdaptiveLayer(i as u8)));
        }
    }
    commands.insert_resource(Adaptativa::default());
}

/// Mide el peligro y la tensión y mezcla las capas: suspenso cuando hay contactos
/// acercándose, batalla cuando los Invid están encima, y la percusión y el crescendo
/// suben con el peligro (jefes, casco bajo, plasma, nodriza en apuros, alguien a la cola).
#[allow(clippy::too_many_arguments)]
pub fn musica_adaptativa(
    time: Res<Time<Real>>,
    mut commands: Commands,
    server: Res<AssetServer>,
    dir: Res<AssetsDir>,
    on: Res<MusicOn>,
    game: Res<crate::components::Game>,
    mission: Res<crate::missions::Mission>,
    intro: Option<Res<crate::models::Intro>>,
    player: Single<&Transform, With<crate::components::Player>>,
    enemies: Query<(&Transform, &crate::components::Enemy, Has<crate::mecha::Boss>), Without<crate::components::Player>>,
    mut ad: ResMut<Adaptativa>,
    mut mix: ResMut<crate::sfx::MixTargets>,
    layers: Query<(&AdaptiveLayer, &mut AudioSink)>,
) {
    use crate::components::{EnemyKind, CARRIER_HP};
    let dt = time.delta_secs();
    ad.stinger_cd -= dt;
    let p = player.translation;
    let pf = player.forward().as_vec3();
    let mut near = 0.0f32;
    let mut contacts = 0.0f32;
    let mut boss = 0.0f32;
    let mut tailed = 0.0f32;
    for (tf, en, is_boss) in &enemies {
        if en.hp <= 0.0 || en.kind != EnemyKind::Fighter {
            continue;
        }
        let d = tf.translation.distance(p);
        if d < 1300.0 {
            near += 1.0;
        } else if d < 4500.0 {
            contacts += 1.0;
        }
        if is_boss && d < 2800.0 {
            boss = 1.0;
        }
        if d < 900.0 && crate::maniobras::en_la_cola(p, pf, tf.translation, tf.forward().as_vec3(), 900.0) {
            tailed = 1.0;
        }
    }
    let in_intro = intro.is_some_and(|i| i.phase != crate::models::IntroPhase::Off) || !game.started;
    let hull_low = ((45.0 - game.hull) / 30.0).clamp(0.0, 1.0);
    let carrier_low = if game.zone == crate::components::Zone::Atmosphere { ((0.45 - game.carrier_hp / CARRIER_HP) / 0.25).clamp(0.0, 1.0) } else { 0.0 };
    let plasma = if game.plasma_warn > 0.0 { 1.0 } else { 0.0 };
    let peligro = (0.13 * near.min(5.0) + 0.38 * boss + 0.32 * hull_low + 0.28 * plasma + 0.25 * carrier_low + 0.18 * tailed).clamp(0.0, 1.0);
    if near > 0.0 {
        ad.batalla_t = 7.0;
    } else {
        ad.batalla_t -= dt;
    }
    let en_batalla = ad.batalla_t > 0.0;
    let tension = if !en_batalla { (contacts / 3.0).min(1.0).max(if mission.phase == crate::missions::Phase::Briefing { 0.6 } else { 0.0 }) } else { 0.0 };
    // seguimiento suave (sube más rápido de lo que baja)
    let k_up = 1.0 - (-dt / 1.2).exp();
    let k_dn = 1.0 - (-dt / 4.0).exp();
    let f = |cur: f32, t: f32| cur + (t - cur) * if t > cur { k_up } else { k_dn };
    ad.peligro = f(ad.peligro, peligro);
    ad.tension = f(ad.tension, tension);
    let batalla = if en_batalla { 1.0 } else { 0.0 };
    let master = if !on.0 || in_intro || game.dead { 0.0 } else if game.paused { 0.15 } else { 1.0 };
    let climax = mission.phase == crate::missions::Phase::Active && mission.idx >= 3;
    let capa_k = if climax { 0.6 } else { 1.0 };
    let pd = ad.peligro;
    let targets = [
        0.42 * ad.tension,
        batalla * (0.30 + 0.12 * pd),
        batalla * (0.10 + 0.50 * pd),
        ((pd - 0.55) / 0.45).clamp(0.0, 1.0) * 0.55,
    ];
    let k = 1.0 - (-dt / 1.5).exp();
    for i in 0..4 {
        let t = targets[i] * master * capa_k;
        ad.niveles[i] += (t - ad.niveles[i]) * k;
    }
    // la música orquestal se aparta cuando entran las capas
    let presencia = (ad.niveles[0] / 0.42).max(ad.niveles[1] / 0.30).clamp(0.0, 1.0);
    ad.duck = 1.0 - 0.7 * presencia;
    for (l, mut sink) in layers {
        sink.set_volume(Volume::Linear(ad.niveles[l.0 as usize]));
    }
    mix.adapt = ad.niveles;
    if (time.elapsed_secs() * 0.5).fract() < dt * 0.5 {
        debug!("música adaptativa: peligro {:.2} tensión {:.2} capas {:?} duck {:.2} (cerca {near}, contactos {contacts})", ad.peligro, ad.tension, ad.niveles.map(|x| (x * 100.0).round() / 100.0), ad.duck);
    }
    mix.duck = ad.duck;
    // golpe orquestal cuando el peligro se dispara
    if pd > 0.72 && !ad.alto && ad.stinger_cd <= 0.0 && master > 0.5 {
        ad.alto = true;
        ad.stinger_cd = 30.0;
        let rel = "audio/adaptativa/bs_alerta.ogg";
        if dir.0.join(rel).exists() {
            commands.spawn((AudioPlayer::new(server.load(rel)), PlaybackSettings { volume: Volume::Linear(0.5), ..PlaybackSettings::DESPAWN }, AdaptiveStinger));
        }
    }
    if pd < 0.45 {
        ad.alto = false;
    }
}

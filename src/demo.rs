//! Grabación de una demo cinemática: `NAVE_DEMO=carpeta nave_combate`
//!
//! * El tiempo avanza en pasos fijos de 1/24 s (aunque cada fotograma tarde más en dibujarse),
//!   y cada fotograma se guarda como PNG en `carpeta/frames/`.
//! * Un guion mueve la cámara y dispara eventos: escena de título, despegue, combate con salvas
//!   en racimo, aparición de un Jefe Invid, subida a la órbita y gran explosión final.
//! * Todo lo que el juego hace sonar se anota en `carpeta/audio.log` (momento, archivo, volumen,
//!   velocidad, paneo y los niveles de música, motor y ambientes). `herramientas/mezcla_demo.py`
//!   reconstruye con eso la banda sonora exacta y `herramientas/montar_demo.sh` arma el video.

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use bevy::audio::PlaybackMode;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::time::TimeUpdateStrategy;

use crate::audio::{EngineSound, MusicTrack, Track};
use crate::components::*;
use crate::gameplay::AutoPilot;
use crate::sfx::{AmbLoop, MixTargets, SpatialPin};

pub const FPS: f32 = 24.0;
/// Segundo de juego en que empieza la grabación (deja cargar los modelos).
const CAP0: f32 = 1.0;
/// Segundos de título antes de "pulsar ENTER".
const TITLE_SECS: f32 = 4.0;
/// Duración del vuelo grabado (desde que termina el despegue).
const FLIGHT_SECS: f32 = 62.0;

#[derive(Clone, Copy)]
enum Paso {
    Vista(CamView),
    Corte(CamView, f32),
    Racimo,
    Jefe,
    Orbita,
    GranExplosion,
}

/// Guion del vuelo (segundos desde el despegue).
const GUION: &[(f32, Paso)] = &[
    (0.0, Paso::Corte(CamView::Flyby, 4.0)),
    (4.0, Paso::Vista(CamView::Pov)),
    (9.0, Paso::Racimo),
    (12.5, Paso::Vista(CamView::Shoulder)),
    (16.5, Paso::Vista(CamView::Wing)),
    (20.0, Paso::Jefe),
    (23.0, Paso::Vista(CamView::Chase)),
    (24.5, Paso::Racimo),
    (30.0, Paso::Vista(CamView::Pov)),
    (34.0, Paso::Corte(CamView::FacePilot, 2.0)),
    (36.5, Paso::Racimo),
    (40.0, Paso::Orbita),
    (45.5, Paso::Vista(CamView::Chase)),
    (48.0, Paso::Racimo),
    (52.0, Paso::Vista(CamView::Wing)),
    (55.0, Paso::GranExplosion),
    (57.0, Paso::Vista(CamView::Chase)),
];

#[derive(Resource)]
pub struct Demo {
    pub dir: PathBuf,
    t: f32,
    frame: u32,
    t_fly: Option<f32>,
    next: usize,
    press: u8,
    log: std::io::BufWriter<std::fs::File>,
}

impl Demo {
    fn cap(&self) -> f32 {
        self.t - CAP0
    }
}

pub fn demo_setup(mut commands: Commands, mut game: ResMut<Game>) {
    let Ok(d) = std::env::var("NAVE_DEMO") else { return };
    let dir = PathBuf::from(d);
    std::fs::create_dir_all(dir.join("frames")).expect("carpeta de la demo");
    let log = std::fs::File::create(dir.join("audio.log")).expect("audio.log");
    commands.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / FPS as f64)));
    commands.insert_resource(Demo { dir, t: 0.0, frame: 0, t_fly: None, next: 0, press: 0, log: std::io::BufWriter::new(log) });
    game.show_help = false;
}

/// "Pulsa" ENTER en la pantalla de título (en PreUpdate, como un teclado).
pub fn demo_keys(demo: Option<ResMut<Demo>>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    let Some(mut demo) = demo else { return };
    match demo.press {
        2 => {
            keys.press(KeyCode::Enter);
            demo.press = 1;
        }
        1 => {
            keys.release(KeyCode::Enter);
            demo.press = 0;
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
pub fn demo_run(
    time: Res<Time<Real>>,
    demo: Option<ResMut<Demo>>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    mut dir: ResMut<Director>,
    mut auto: ResMut<AutoPilot>,
    intro: Option<Res<crate::models::Intro>>,
    player: Single<(Entity, &Transform), With<Player>>,
    enemies: Query<(Entity, &Transform, &Enemy), Without<Player>>,
    a: Res<crate::world::GameAssets>,
    mut cluster: ResMut<crate::fx::ClusterQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut demo) = demo else { return };
    demo.t += time.delta_secs();
    let cap = demo.cap();
    if cap >= 0.0 {
        let path = demo.dir.join("frames").join(format!("f{:05}.png", demo.frame));
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        demo.frame += 1;
        if demo.frame % 48 == 0 {
            info!("DEMO: fotograma {} · t={:.1}s", demo.frame, cap);
        }
    }
    let in_title = intro.as_ref().is_some_and(|i| i.phase == crate::models::IntroPhase::Title);
    if in_title && cap >= TITLE_SECS && demo.t_fly.is_none() && demo.press == 0 {
        demo.press = 2;
    }
    let flying = game.started && intro.as_ref().is_none_or(|i| i.phase == crate::models::IntroPhase::Off);
    if flying && demo.t_fly.is_none() {
        demo.t_fly = Some(cap);
        auto.0 = true;
        dir.auto = true;
    }
    let Some(t_fly) = demo.t_fly else { return };
    let ft = cap - t_fly;
    // invulnerable durante la demo
    game.hull = 100.0;
    game.shield = 100.0;
    game.cluster = 3;
    let (pe, ptf) = *player;
    while demo.next < GUION.len() && GUION[demo.next].0 <= ft {
        let paso = GUION[demo.next].1;
        demo.next += 1;
        match paso {
            Paso::Vista(v) => {
                dir.view = v;
                dir.cut = None;
            }
            Paso::Corte(v, d) => {
                dir.flyby_set = false;
                dir.cut = Some((v, d));
            }
            Paso::Racimo => {
                let fwd = ptf.forward().as_vec3();
                let mut c: Vec<(Entity, f32)> = enemies
                    .iter()
                    .filter(|(_, _, en)| en.hp > 0.0)
                    .filter_map(|(e, t, _)| {
                        let to = t.translation - ptf.translation;
                        let ang = fwd.angle_between(to);
                        (to.length() < 3500.0 && ang < 1.2).then_some((e, ang))
                    })
                    .collect();
                c.sort_by(|x, y| x.1.total_cmp(&y.1));
                let targets: Vec<Entity> = c.iter().take(4).map(|x| x.0).collect();
                cluster.salvo(pe, &targets, 12, 0.045);
                crate::sfx::sfx(&mut commands, crate::sfx::SfxKind::ClusterLaunch, None, 1.0);
                crate::sfx::bark(&mut commands, "#p_racimo");
                game.ev_cluster = true;
            }
            Paso::Jefe => {
                let p = ptf.translation + ptf.forward().as_vec3() * 650.0 + Vec3::Y * 30.0;
                let e = crate::mecha::spawn_boss(&mut commands, &a, p, ptf.translation, false);
                dir.close_target = Some(e);
                dir.cut = Some((CamView::EnemyClose, 2.8));
            }
            Paso::Orbita => {
                if game.zone == Zone::Atmosphere {
                    game.pending_zone = Some(Zone::Space);
                    game.fade_dir = 1.0;
                }
            }
            Paso::GranExplosion => {
                let base = ptf.translation + ptf.forward().as_vec3() * 520.0;
                for (k, off) in [Vec3::ZERO, Vec3::new(40.0, 25.0, -30.0), Vec3::new(-50.0, -20.0, 20.0)].iter().enumerate() {
                    commands.spawn(crate::fx::Boom { pos: base + *off, size: 45.0 - k as f32 * 8.0, vel: Vec3::ZERO });
                }
            }
        }
    }
    let flight_secs = std::env::var("NAVE_DEMO_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(FLIGHT_SECS);
    if ft > flight_secs {
        let _ = writeln!(demo.log, "END {cap:.4}");
        let _ = demo.log.flush();
        info!("DEMO terminada: {} fotogramas", demo.frame);
        exit.write(AppExit::Success);
    }
}

/// Anota cada sonido que empieza o se corta, y los niveles de los bucles.
#[allow(clippy::too_many_arguments)]
pub fn demo_audio_log(
    demo: Option<ResMut<Demo>>,
    server: Res<AssetServer>,
    dir: Res<Director>,
    mix: Res<MixTargets>,
    added: Query<(Entity, &AudioPlayer, &PlaybackSettings, Option<&SpatialPin>, Option<&MusicTrack>, Has<EngineSound>, Option<&AmbLoop>, Option<&crate::audio::AdaptiveLayer>), Added<AudioPlayer>>,
    mut removed: RemovedComponents<AudioPlayer>,
) {
    let Some(mut demo) = demo else { return };
    let t = demo.cap();
    let cam = dir.cam;
    for (e, ap, ps, pin, music, engine, amb, adapt) in &added {
        let Some(path) = server.get_path(ap.0.id()) else { continue };
        let path = path.path().to_string_lossy().to_string();
        if matches!(ps.mode, PlaybackMode::Loop) {
            let kind = if let Some(m) = music {
                format!("music{}", match m.0 { Track::Titulo => 0, Track::Combate => 1, Track::Climax => 2 })
            } else if engine {
                "engine".into()
            } else if let Some(l) = amb {
                format!("amb{}", l.0)
            } else if let Some(l) = adapt {
                format!("adapt{}", l.0)
            } else {
                "other".into()
            };
            let _ = writeln!(demo.log, "L {t:.4} {kind} {path}");
        } else {
            let pan = pin.map(|p| (p.0 - cam.translation).normalize_or(Vec3::ZERO).dot(cam.right().as_vec3())).unwrap_or(0.0);
            let _ = writeln!(demo.log, "S {t:.4} {} {path} {:.4} {:.4} {pan:.3} {}", e.to_bits(), ps.volume.to_linear(), ps.speed, pin.is_some() as u8);
        }
    }
    for e in removed.read() {
        let _ = writeln!(demo.log, "X {t:.4} {}", e.to_bits());
    }
    let _ = writeln!(
        demo.log,
        "M {t:.4} {} {:.4} {:.4} {:.4} {:.4} {:.4} {:.4} {:.4} {:.4} {:.4} {:.4} {:.4} {:.4}",
        mix.music.0, mix.music.1, mix.engine.0, mix.engine.1, mix.amb[0], mix.amb[1], mix.amb[2], mix.wind_speed,
        mix.adapt[0], mix.adapt[1], mix.adapt[2], mix.adapt[3], mix.duck
    );
}

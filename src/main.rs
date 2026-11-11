//! ESCUADRÓN ORBITAL — combate de cazas en la Tierra y en órbita, vista de piloto.
//! Hecho con Bevy. Todo (modelos, terreno, sonido) se genera por código.

mod allies;
mod audio;
mod cockpit;
mod components;
mod demo;
mod fx;
mod gameplay;
mod hud;
mod hudfx;
mod mando;
mod maniobras;
mod mecha;
mod missions;
mod models;
mod sfx;
mod spartan;
mod sdlpad;
mod toon;
mod voces;
mod world;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

use components::*;
use gameplay::*;
use hud::*;

fn main() {
    // `nave_combate --exportar-sonidos carpeta` → escribe los sonidos sintetizados en .wav
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--exportar-sonidos") {
        let dir = args.get(i + 1).cloned().unwrap_or_else(|| "sonidos_wav".into());
        audio::export_procedural(std::path::Path::new(&dir)).expect("no se pudieron escribir los sonidos");
        println!("Sonidos exportados en {dir}");
        return;
    }
    let assets_dir = find_assets_dir();
    let plugins = DefaultPlugins.set(bevy::asset::AssetPlugin {
            file_path: assets_dir.clone(),
            ..default()
        }).set(WindowPlugin {
            primary_window: Some(Window {
                title: "Escuadrón Orbital — Bevy".into(),
                resolution: WindowResolution::new(1600, 900),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        });
    // en macOS los mandos van por SDL2 (si se compiló con --features sdl)
    let plugins = if sdlpad::activo() { plugins.disable::<bevy::gilrs::GilrsPlugin>() } else { plugins };
    App::new()
        .insert_resource(audio::AssetsDir(assets_dir.clone().into()))
        .insert_resource(audio::MusicOn(true))
        .insert_resource(ClearColor(Color::srgb(0.48, 0.66, 0.92)))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.75, 0.85, 1.0),
            brightness: 900.0,
            ..default()
        })
        .insert_resource(Game::default())
        .insert_resource(spartan::Frentes::default())
        .insert_resource(sfx::MixTargets::default())
        .insert_resource(InvertY::default())
        .insert_resource(AutoPilot::default())
        .insert_resource(Director::default())
        .insert_resource(missions::Difficulty(missions::Level::Basica))
        .insert_resource(missions::Mission::default())
        .insert_resource(missions::Radio::default())
        .insert_resource(mecha::Mecha::default())
        .insert_resource(cockpit::TgtInfo::default())
        .add_plugins(plugins)
        .add_plugins(sdlpad::plugin)
        .add_systems(PreStartup, (load_font, mando::setup_mando))
        .add_systems(PreUpdate, (mando::mando_input, demo::demo_keys).chain().after(bevy::input::InputSystems))
        .add_systems(Update, demo::demo_run.after(cockpit::director).before(hudfx::draw_hud))
        .add_systems(Last, demo::demo_audio_log)
        .add_systems(
            Startup,
            (
                audio::setup_sounds,
                audio::setup_adaptativa,
                world::setup_assets,
                fx::setup_fx,
                models::load_models,
                setup_player,
                sfx::setup_capas,
                models::setup_lighting,
                setup_hud,
                hudfx::setup_gizmos,
                missions::setup_missions,
                autotest_setup,
                demo::demo_setup,
                models::setup_intro,
            )
                .chain(),
        )
        .add_observer(models::pose_seated_pilot)
        .add_observer(models::start_intro_anim)
        .add_observer(models::wreck_ready)
        .add_observer(models::tame_invid_materials)
        .add_observer(spartan::spartan_lod)
        .add_systems(Update, (audio::musica_adaptativa, audio::music_mix).chain())
        .add_systems(Update, (missions::radio_update, missions::scale_new_enemies))
        .add_systems(
            Update,
            (missions::mission_update, missions::plasma_fire, mecha::boss_attacks, mecha::shock_attacks, mecha::hive_spawner, spartan::frentes, spartan::spartan_ai, missions::plasma_update, ensure_hive, add_maniobras)
                .chain()
                .run_if(playing)
                .after(waves)
                .before(effects),
        )
        .add_systems(Update, missions::objective_markers.after(cockpit::director))
        .add_systems(Update, (mecha::form_models, form_test))
        .add_systems(Update, (input_misc, autotest, rearm_test, models::intro_update, models::zone_lighting))
        .add_systems(
            Update,
            (
                mecha::mecha_input,
                player_flight,
                player_weapons,
                enemy_ai,
                allies::ally_ai,
                missiles,
                bullets,
                physical_collisions,
                carrier_update,
                allies::ally_status,
                enemy_deaths,
                player_status,
                waves,
            )
                .chain()
                .run_if(playing)
                .after(input_misc),
        )
        .add_systems(
            Update,
            (effects, zone_transition, cockpit::director, environment, cockpit::cockpit_anim, cockpit::target_camera, cockpit::update_screens, models::update_wrecks, engine_audio, hudfx::draw_hud)
                .chain()
                .after(waves),
        )
        .add_systems(Update, test_cam.after(cockpit::director).before(hudfx::draw_hud))
        .add_systems(Update, (boom_test, fixed_dt_test))
        .add_systems(Update, fx::cluster_launch.after(player_weapons).before(missiles).run_if(playing))
        .add_systems(Update, (fx::attach_trails, fx::trails).chain().after(missiles).after(cockpit::director))
        .add_systems(Update, (fx::booms, fx::fx_parts).chain().after(waves).before(effects))
        .add_systems(Update, (sfx::combat_chatter, sfx::sfx_dispatch, sfx::spatial_pins, sfx::ambience).chain().after(cockpit::director))
        .add_systems(Update, (hud_text, hud_overlays).after(cockpit::director).after(models::intro_update))
        .run();
}

/// Busca la carpeta `assets` (modelos 3D) junto al ejecutable o en el proyecto,
/// para que funcione tanto con `cargo run` como con el binario precompilado.
fn find_assets_dir() -> String {
    let mut cands: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("NAVE_ASSETS") {
        cands.push(p.into());
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut d = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..4 {
            if let Some(dir) = d {
                cands.push(dir.join("assets"));
                d = dir.parent().map(|p| p.to_path_buf());
            } else {
                break;
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        cands.push(cwd.join("assets"));
    }
    for c in &cands {
        if c.join("models").join("valkyrie.glb").exists() {
            return c.to_string_lossy().into_owned();
        }
    }
    eprintln!("AVISO: no se encontró la carpeta assets/models con los modelos 3D");
    "assets".into()
}

/// Reemplaza la fuente por defecto (solo ASCII) por DejaVu Sans Mono, que
/// incluye tildes, eñes y símbolos del HUD. Va incrustada en el ejecutable.
fn load_font(mut fonts: ResMut<Assets<Font>>) {
    let data = include_bytes!("../fonts/DejaVuSansMono-Bold.ttf").to_vec();
    let _ = fonts.insert(AssetId::default(), Font::from_bytes(data));
}

// ---------------------------------------------------------------- modo de prueba automática
// NAVE_SCREENSHOT=archivo.png  → arranca solo, toma una captura y sale.
// NAVE_ZONE=space  NAVE_VIEW=chase  NAVE_SHOT_AT=segundos

#[derive(Resource)]
struct AutoTest {
    path: String,
    at: f32,
    done: bool,
}

fn autotest_setup(
    mut commands: Commands,
    mut game: ResMut<Game>,
    mut auto: ResMut<AutoPilot>,
    mut dir: ResMut<Director>,
    mut mission: ResMut<missions::Mission>,
    mut diff: ResMut<missions::Difficulty>,
) {
    if let Ok(i) = std::env::var("NAVE_MISSION").map(|v| v.parse::<usize>().unwrap_or(1)) {
        mission.idx = i.clamp(1, 5) - 1;
    }
    if std::env::var("NAVE_DIFF").as_deref() == Ok("media") {
        diff.0 = missions::Level::Media;
    }
    if let Ok(path) = std::env::var("NAVE_SCREENSHOT") {
        auto.0 = std::env::var("NAVE_AUTOPILOT").is_ok();
        game.started = std::env::var("NAVE_INTRO").is_err();
        game.show_help = std::env::var("NAVE_HELP").is_ok();
        dir.auto = std::env::var("NAVE_CUTS").is_ok();
        if let Ok(v) = std::env::var("NAVE_VIEW") {
            dir.view = match v.as_str() {
                "shoulder" => CamView::Shoulder,
                "face" => CamView::FacePilot,
                "chase" => CamView::Chase,
                "wing" => CamView::Wing,
                "flyby" => CamView::Flyby,
                "stick" => CamView::StickClose,
                "invid" => CamView::EnemyClose,
                "kill" => CamView::Chase,
                _ => CamView::Pov,
            };
        }
        if std::env::var("NAVE_ZONE").as_deref() == Ok("space") {
            game.pending_zone = Some(Zone::Space);
            game.fade_dir = 1.0;
        }
        let at = std::env::var("NAVE_SHOT_AT").ok().and_then(|s| s.parse().ok()).unwrap_or(8.0);
        commands.insert_resource(AutoTest { path, at, done: false });
    }
}

fn autotest(
    time: Res<Time>,
    mut commands: Commands,
    t: Option<ResMut<AutoTest>>,
    mut exit: MessageWriter<AppExit>,
    game: Res<Game>,
    dir: Res<Director>,
    mission: Res<missions::Mission>,
) {
    let Some(mut t) = t else { return };
    let e = time.elapsed_secs();

    if !t.done && e > t.at {
        t.done = true;
        use bevy::render::view::screenshot::{save_to_disk, Screenshot};
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(t.path.clone()));
    }
    if t.done && e > t.at + 2.0 {
        info!("PRUEBA: puntos={} derribos={} oleada={} casco={:.0} escudo={:.0} zona={:?} vista={:?} mision={} fase={:?} objetivo='{}' nodriza={:.0}", game.score, game.kills, game.wave, game.hull, game.shield, game.zone, dir.current(), mission.idx + 1, mission.phase, mission.objective, game.carrier_hp);
        exit.write(AppExit::Success);
    }
}

/// Sólo pruebas: NAVE_LOOKAT=carrier|mother|ring coloca la cámara mirando esa nave.
fn test_cam(
    time: Res<Time<Real>>,
    mut dir: ResMut<Director>,
    mut cam: Single<&mut Transform, With<WorldCam>>,
    carrier: Query<&Transform, (With<CarrierShip>, Without<WorldCam>)>,
    ring: Query<&Transform, (With<RearmRing>, Without<WorldCam>, Without<CarrierShip>)>,
    enemies: Query<(&Transform, &Enemy), (Without<WorldCam>, Without<CarrierShip>, Without<RearmRing>)>,
    allies: Query<&Transform, (With<allies::Ally>, Without<WorldCam>, Without<CarrierShip>, Without<RearmRing>, Without<Enemy>)>,
    spartans: Query<&Transform, (With<spartan::Spartan>, Without<WorldCam>, Without<CarrierShip>, Without<RearmRing>, Without<Enemy>, Without<allies::Ally>)>,
) {
    let Ok(what) = std::env::var("NAVE_LOOKAT") else { return };
    let t = time.elapsed_secs() * 0.05;
    let (target, dist) = match what.as_str() {
        "carrier" => match carrier.iter().next() {
            Some(c) => (c.translation, 620.0),
            None => return,
        },
        "squad" => match allies.iter().next() {
            Some(a) => {
                let p = a.translation + a.rotation * Vec3::new(30.0, 12.0, -60.0);
                **cam = Transform::from_translation(p).looking_at(a.translation + a.rotation * Vec3::new(0.0, 0.0, 30.0), Vec3::Y);
                dir.cam = **cam;
                return;
            }
            None => return,
        },
        "spartan" => match spartans.iter().next() {
            Some(s) => {
                let p = s.translation + s.rotation * Vec3::new(22.0, 9.0, -42.0);
                **cam = Transform::from_translation(p).looking_at(s.translation + Vec3::Y * 9.0 + s.rotation * Vec3::new(-10.0, 4.0, 0.0), Vec3::Y);
                dir.cam = **cam;
                return;
            }
            None => return,
        },
        "ring" => match ring.iter().next() {
            Some(r) => (r.translation - Vec3::Y * 40.0, 330.0),
            None => return,
        },
        _ => match enemies.iter().find(|e| e.1.kind == EnemyKind::Mothership) {
            Some((m, _)) => (m.translation, 1500.0),
            None => return,
        },
    };
    let pos = target + Vec3::new((1.1 + t).sin() * dist, dist * 0.35, (1.1 + t).cos() * dist);
    **cam = Transform::from_translation(pos).looking_at(target, Vec3::Y);
    dir.cam = **cam;
}

/// Sólo pruebas: NAVE_REARMTEST=1 vacía los misiles y lleva la nave a través del anillo.
fn rearm_test(
    time: Res<Time<Real>>,
    mut game: ResMut<Game>,
    mut stage: Local<u8>,
    mut player: Single<&mut Transform, With<Player>>,
    ring: Query<&Transform, (With<RearmRing>, Without<Player>)>,
) {
    if std::env::var("NAVE_REARMTEST").is_err() {
        return;
    }
    let t = time.elapsed_secs();
    let Some(r) = ring.iter().next() else { return };
    if *stage == 0 && t > 8.0 {
        game.missiles = 0;
        game.hull = 40.0;
        game.shield = 10.0;
        game.heat = 0.9;
        player.translation = r.translation + Vec3::new(0.0, 0.0, 150.0);
        player.rotation = Quat::IDENTITY;
        info!("REARM-TEST antes: misiles={} casco={} escudo={}", game.missiles, game.hull, game.shield);
        *stage = 1;
    }
    if *stage == 1 && t > 11.0 {
        info!("REARM-TEST después: misiles={} casco={} escudo={} calor={:.2} cd={:.1}", game.missiles, game.hull, game.shield, game.heat, game.rearm_cd);
        *stage = 2;
    }
}

/// La colmena Invid existe en la atmósfera mientras no haya sido destruida.
fn ensure_hive(
    mut commands: Commands,
    a: Res<world::GameAssets>,
    game: Res<Game>,
    diff: Res<missions::Difficulty>,
    hives: Query<(), With<mecha::Hive>>,
) {
    if game.zone == Zone::Atmosphere && game.hive_alive && game.pending_zone.is_none() && hives.is_empty() {
        mecha::spawn_hive(&mut commands, &a, diff.media());
    }
}

/// Sólo pruebas: NAVE_FORM=gerwalk|battroid coloca la nave cerca del suelo frente a la colmena
/// en esa forma. NAVE_BOSS=1 hace aparecer un Jefe delante.
fn form_test(
    time: Res<Time<Real>>,
    mut commands: Commands,
    a: Res<world::GameAssets>,
    mut stage: Local<u8>,
    mut mecha: ResMut<mecha::Mecha>,
    mut player: Single<(&mut Transform, &mut Player)>,
) {
    let form = std::env::var("NAVE_FORM").ok();
    let boss = std::env::var("NAVE_BOSS").is_ok();
    if form.is_none() && !boss {
        return;
    }
    let t = time.elapsed_secs();
    if *stage == 0 && t > 6.0 {
        *stage = 1;
        let h = mecha::hive_pos();
        let pos = Vec3::new(h.x + 900.0, 0.0, h.z + 900.0);
        let g = world::ground_height(pos.x, pos.z);
        let (ref mut tf, ref mut pl) = *player;
        tf.translation = Vec3::new(pos.x, g + 120.0, pos.z);
        tf.look_at(Vec3::new(h.x, g + 160.0, h.z), Vec3::Y);
        pl.speed = 20.0;
        pl.throttle = 0.1;
        match form.as_deref() {
            Some("gerwalk") => mecha.mode = mecha::Mode::Gerwalk,
            Some("battroid") => mecha.mode = mecha::Mode::Battroid,
            _ => {}
        }
        if boss {
            let p = tf.translation + tf.forward() * 120.0 + Vec3::Y * 10.0;
            if std::env::var("NAVE_BOSS").as_deref() == Ok("shock") {
                mecha::spawn_shock(&mut commands, &a, p, tf.translation, true);
            } else {
                mecha::spawn_boss(&mut commands, &a, p, tf.translation, true);
            }
        }
    }
}

/// Sólo pruebas: NAVE_BOOM=segundos crea una explosión grande delante de la cámara.
fn boom_test(time: Res<Time>, mut commands: Commands, dir: Res<Director>, mut done: Local<bool>) {
    let Some(t0) = std::env::var("NAVE_BOOM").ok().and_then(|v| v.parse::<f32>().ok()) else { return };
    if !*done && time.elapsed_secs() >= t0 {
        *done = true;
        let size = std::env::var("NAVE_BOOM_SIZE").ok().and_then(|v| v.parse().ok()).unwrap_or(22.0);
        let p = dir.cam.translation + dir.cam.forward().as_vec3() * (size * 12.0 + 160.0);
        commands.spawn(fx::Boom { pos: p, size, vel: Vec3::ZERO });
    }
}

/// Sólo pruebas: NAVE_FIXEDDT=segundos → desde ese instante cada fotograma avanza 1/30 s
/// (el renderizado por software es muy lento y si no las trayectorias salen a saltos).
fn fixed_dt_test(time: Res<Time>, mut commands: Commands, mut done: Local<bool>) {
    let Some(t0) = std::env::var("NAVE_FIXEDDT").ok().and_then(|v| v.parse::<f32>().ok()) else { return };
    if !*done && time.elapsed_secs() >= t0 {
        *done = true;
        commands.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 30.0)));
    }
}

/// Los cazas Invid normales aprenden maniobras evasivas (los jefes no).
fn add_maniobras(
    mut commands: Commands,
    q: Query<(Entity, &Enemy), (Added<Enemy>, Without<mecha::Boss>, Without<missions::Elite>)>,
) {
    for (e, en) in &q {
        if en.kind == EnemyKind::Fighter {
            commands.entity(e).insert(maniobras::Maniobra::default());
        }
    }
}

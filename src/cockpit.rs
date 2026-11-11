//! Cabina estilo anime de los 90: consola hexagonal con pantallas dibujadas,
//! franjas de peligro, piloto con casco y guantes en los mandos,
//! y un "director" de cámaras con siete vistas cinematográficas.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use rand::Rng;

use crate::components::*;
use crate::toon::*;
use crate::world::*;
use crate::models::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};

#[derive(Resource)]
pub struct Screens {
    pub main: Handle<Image>,
    pub dial: Handle<Image>,
    pub status: Handle<Image>,
    pub tgt_overlay: Handle<Image>,
    pub tgt_cam: Handle<Image>,
}

#[derive(Component)]
pub struct StickPivot;

#[derive(Component)]
pub struct ThrottleHandle;

/// Cámara de puntería: video en vivo del blanco en la pantalla izquierda.
#[derive(Component)]
pub struct TargetCam;

/// Aguja de un instrumento analógico.
#[derive(Component)]
pub struct Needle(pub u8);

/// Botón grande de "PRECAUCIÓN MAESTRA".
#[derive(Component)]
pub struct MasterCaution;

/// Lo que ve la cámara de puntería (para dibujar su capa de datos).
#[derive(Resource, Default)]
pub struct TgtInfo {
    pub has: bool,
    pub dist: f32,
    pub zoom: f32,
    pub kind: Option<EnemyKind>,
    pub locked: bool,
    pub hp: f32,
}

const MAIN_W: usize = 768;
const MAIN_H: usize = 348;
const DIAL: usize = 256;
const SIDE_W: usize = 384;
const SIDE_H: usize = 288;

/// Prisma hexagonal (contorno de la consola), con sombreado cel.
fn hex_prism(pts: &[Vec2], depth: f32) -> Mesh {
    let n = pts.len();
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    // cara frontal (+z) y trasera (-z)
    for (z, flip) in [(depth * 0.5, false), (-depth * 0.5, true)] {
        let base = pos.len() as u32;
        for p in pts {
            pos.push([p.x, p.y, z]);
        }
        for i in 1..n as u32 - 1 {
            if flip {
                idx.extend_from_slice(&[base, base + i + 1, base + i]);
            } else {
                idx.extend_from_slice(&[base, base + i, base + i + 1]);
            }
        }
    }
    // laterales
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let base = pos.len() as u32;
        pos.push([a.x, a.y, depth * 0.5]);
        pos.push([b.x, b.y, depth * 0.5]);
        pos.push([b.x, b.y, -depth * 0.5]);
        pos.push([a.x, a.y, -depth * 0.5]);
        idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_indices(Indices::U32(idx));
    m.duplicate_vertices();
    m.compute_flat_normals();
    bake_toon(m, Vec3::new(0.2, 0.7, 0.7))
}

pub fn build_cockpit(
    p: &mut ChildSpawnerCommands,
    t: &ToonMeshes,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    pilot_scene: &Handle<WorldAsset>,
) -> Screens {
    let frame = flat(mats, Color::srgb(0.13, 0.14, 0.22));
    let body = flat(mats, Color::srgb(0.10, 0.11, 0.17));
    let bezel = flat(mats, Color::srgb(0.30, 0.37, 0.42));
    let glove = flat(mats, Color::srgb(0.66, 0.61, 0.52));
    let sleeve = flat(mats, Color::srgb(0.30, 0.29, 0.52));
    let cuff = flat(mats, Color::srgb(0.96, 0.96, 0.96));
    let seat = flat(mats, Color::srgb(0.26, 0.22, 0.26));
    let stick = flat(mats, Color::srgb(0.17, 0.25, 0.22));
    let green_cap = glow(mats, 0.6, 2.2, 0.8);
    let red = glow(mats, 6.0, 0.4, 0.3);
    let amber = glow(mats, 4.0, 2.0, 0.3);
    let hazard = mats.add(StandardMaterial {
        base_color_texture: Some(images.add(hazard_image())),
        unlit: true,
        ..default()
    });

    let screen_lit = |mats: &mut Assets<StandardMaterial>, img: &Handle<Image>, k: f32, tint: Vec3| {
        mats.add(StandardMaterial {
            base_color: Color::LinearRgba(LinearRgba::rgb(k * tint.x, k * tint.y, k * tint.z)),
            base_color_texture: Some(img.clone()),
            unlit: true,
            fog_enabled: false,
            ..default()
        })
    };
    let blank = |w: usize, h: usize| {
        let mut c = Canvas::new(w, h);
        c.clear([8, 10, 18]);
        c.to_image()
    };
    let mut overlay = Canvas::new(SIDE_W, SIDE_H);
    overlay.clear_transparent();
    let tgt_cam = images.add(Image::new_target_texture(
        512,
        384,
        bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        Some(bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb),
    ));
    let screens = Screens {
        main: images.add(blank(MAIN_W, MAIN_H)),
        dial: images.add(blank(DIAL, DIAL)),
        status: images.add(blank(SIDE_W, SIDE_H)),
        tgt_overlay: images.add(overlay.to_image()),
        tgt_cam,
    };
    let screen_mat = screen_lit(mats, &screens.main, 1.25, Vec3::ONE);
    let dial_mat = screen_lit(mats, &screens.dial, 1.25, Vec3::ONE);
    let status_mat = screen_lit(mats, &screens.status, 1.25, Vec3::ONE);
    // el video se ve como un monitor monocromo verde-azulado
    let tgt_mat = screen_lit(mats, &screens.tgt_cam, 1.35, Vec3::new(0.80, 1.0, 0.88));
    let overlay_mat = mats.add(StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::rgb(1.4, 1.4, 1.4)),
        base_color_texture: Some(screens.tgt_overlay.clone()),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        fog_enabled: false,
        ..default()
    });
    let gauge_mat = screen_lit(mats, &images.add(gauge_face_image()), 1.0, Vec3::ONE);
    let needle_mat = glow(mats, 3.0, 1.2, 0.2);
    let caution_off = flat(mats, Color::srgb(0.35, 0.22, 0.05));
    let caution_on = glow(mats, 6.0, 3.2, 0.3);
    let osb = flat(mats, Color::srgb(0.20, 0.22, 0.28));
    let osb_tip = glow(mats, 0.4, 1.4, 0.6);
    let switch_metal = flat(mats, Color::srgb(0.75, 0.76, 0.80));
    let screw = flat(mats, Color::srgb(0.45, 0.48, 0.52));
    let label_plate = flat(mats, Color::srgb(0.85, 0.82, 0.70));

    // Botones de borde (OSB) alrededor de una pantalla: fila de botoncitos con punta iluminada.
    let osb_row = |c: &mut ChildSpawnerCommands, from: Vec2, to: Vec2, n: usize, z: f32| {
        for i in 0..n {
            let k = if n == 1 { 0.5 } else { i as f32 / (n - 1) as f32 };
            let p = from.lerp(to, k);
            t.part(c, Shape::Cube, &osb, Transform::from_xyz(p.x, p.y, z).with_scale(Vec3::new(0.032, 0.022, 0.014)), 0.002);
            c.spawn((Mesh3d(t.cube.clone()), MeshMaterial3d(osb_tip.clone()), Transform::from_xyz(p.x, p.y, z + 0.008).with_scale(Vec3::new(0.016, 0.005, 0.002))));
        }
    };

    // ------------------------------------------------ consola hexagonal central
    let hex = [
        Vec2::new(-0.50, 0.06),
        Vec2::new(-0.40, 0.19),
        Vec2::new(0.40, 0.19),
        Vec2::new(0.50, 0.06),
        Vec2::new(0.41, -0.19),
        Vec2::new(-0.41, -0.19),
    ];
    let hex_mesh = meshes.add(hex_prism(&hex, 0.07));
    let inner: Vec<Vec2> = hex.iter().map(|v| *v * Vec2::new(0.93, 0.86)).collect();
    let inner_mesh = meshes.add(hex_prism(&inner, 0.02));
    let disc = meshes.add(Circle::new(1.0));
    let console_tf = Transform::from_xyz(0.0, -0.37, -0.82).with_rotation(Quat::from_rotation_x(-0.32));
    p.spawn((console_tf, Visibility::default())).with_children(|c| {
        c.spawn((Mesh3d(hex_mesh.clone()), MeshMaterial3d(bezel.clone())));
        c.spawn((Mesh3d(hex_mesh.clone()), MeshMaterial3d(t.ink.clone()), Transform::from_scale(Vec3::new(1.03, 1.05, 1.2))));
        c.spawn((Mesh3d(inner_mesh), MeshMaterial3d(body.clone()), Transform::from_xyz(0.0, 0.0, 0.03)));
        c.spawn((Mesh3d(meshes.add(Rectangle::new(0.70, 0.317))), MeshMaterial3d(screen_mat), Transform::from_xyz(0.0, 0.005, 0.045)));
        // botones bajo la pantalla y tornillos en las esquinas
        osb_row(c, Vec2::new(-0.27, -0.172), Vec2::new(0.27, -0.172), 7, 0.04);
        for (x, y) in [(-0.44, 0.06), (0.44, 0.06), (-0.37, -0.16), (0.37, -0.16), (-0.35, 0.165), (0.35, 0.165)] {
            t.part(c, Shape::Cyl, &screw, Transform::from_xyz(x, y, 0.04).with_rotation(Quat::from_rotation_x(1.5708)).with_scale(Vec3::new(0.008, 0.006, 0.008)), 0.0);
        }
        // visera sobre la pantalla
        t.part(c, Shape::Cube, &body, Transform::from_xyz(0.0, 0.205, 0.05).with_rotation(Quat::from_rotation_x(0.35)).with_scale(Vec3::new(0.80, 0.018, 0.11)), 0.004);
        // instrumentos analógicos en las torres superiores
        for (x, kind) in [(-0.30f32, 0u8), (0.30, 1u8)] {
            t.part(c, Shape::Cyl, &body, Transform::from_xyz(x, 0.25, -0.005).with_rotation(Quat::from_rotation_x(1.5708)).with_scale(Vec3::new(0.06, 0.05, 0.06)), 0.004);
            c.spawn((Mesh3d(disc.clone()), MeshMaterial3d(gauge_mat.clone()), Transform::from_xyz(x, 0.25, 0.022).with_scale(Vec3::splat(0.052))));
            c.spawn((Transform::from_xyz(x, 0.25, 0.026), Visibility::default(), Needle(kind))).with_children(|n| {
                n.spawn((Mesh3d(t.cube.clone()), MeshMaterial3d(needle_mat.clone()), Transform::from_xyz(0.0, 0.019, 0.0).with_scale(Vec3::new(0.005, 0.04, 0.002))));
            });
            c.spawn((Mesh3d(t.cube.clone()), MeshMaterial3d(label_plate.clone()), Transform::from_xyz(x, 0.19, 0.012).with_scale(Vec3::new(0.05, 0.012, 0.004))));
        }
        // precaución maestra (centro superior)
        t.part(c, Shape::Cube, &body, Transform::from_xyz(0.0, 0.245, 0.0).with_scale(Vec3::new(0.10, 0.06, 0.05)), 0.004);
        c.spawn((Mesh3d(t.cube.clone()), MeshMaterial3d(caution_off.clone()), Transform::from_xyz(0.0, 0.245, 0.024).with_scale(Vec3::new(0.08, 0.042, 0.004))));
        c.spawn((Mesh3d(t.cube.clone()), MeshMaterial3d(caution_on.clone()), Transform::from_xyz(0.0, 0.245, 0.027).with_scale(Vec3::new(0.08, 0.042, 0.004)), MasterCaution, Visibility::Hidden));
        t.part(c, Shape::Cube, &hazard, Transform::from_xyz(-0.11, 0.245, 0.0).with_scale(Vec3::new(0.035, 0.05, 0.045)), 0.003);
        t.part(c, Shape::Cube, &hazard, Transform::from_xyz(0.11, 0.245, 0.0).with_scale(Vec3::new(0.035, 0.05, 0.045)), 0.003);
    });

    // ------------------------------------------------ pantallas laterales (cámara de blanco / estado)
    for (side, scr, is_tgt) in [(-1.0f32, status_mat.clone(), true), (1.0, status_mat.clone(), false)] {
        let tf = Transform::from_xyz(side * 0.585, -0.36, -0.75)
            .with_rotation(Quat::from_rotation_y(-side * 0.66) * Quat::from_rotation_x(-0.30));
        let tgt_mat = tgt_mat.clone();
        let overlay_mat = overlay_mat.clone();
        let rect = meshes.add(Rectangle::new(0.30, 0.225));
        p.spawn((tf, Visibility::default())).with_children(|c| {
            t.part(c, Shape::Cube, &bezel, Transform::from_xyz(0.0, 0.0, -0.01).with_scale(Vec3::new(0.40, 0.30, 0.04)), 0.005);
            t.part(c, Shape::Cube, &body, Transform::from_xyz(0.0, 0.0, 0.006).with_scale(Vec3::new(0.32, 0.245, 0.01)), 0.0);
            if is_tgt {
                c.spawn((Mesh3d(rect.clone()), MeshMaterial3d(tgt_mat), Transform::from_xyz(0.0, 0.0, 0.012)));
                c.spawn((Mesh3d(rect.clone()), MeshMaterial3d(overlay_mat), Transform::from_xyz(0.0, 0.0, 0.014)));
            } else {
                c.spawn((Mesh3d(rect.clone()), MeshMaterial3d(scr), Transform::from_xyz(0.0, 0.0, 0.012)));
            }
            osb_row(c, Vec2::new(-0.178, -0.09), Vec2::new(-0.178, 0.09), 5, 0.012);
            osb_row(c, Vec2::new(0.178, -0.09), Vec2::new(0.178, 0.09), 5, 0.012);
            osb_row(c, Vec2::new(-0.10, 0.133), Vec2::new(0.10, 0.133), 4, 0.012);
            t.part(c, Shape::Cube, &hazard, Transform::from_xyz(0.0, -0.165, 0.0).with_scale(Vec3::new(0.40, 0.03, 0.045)), 0.004);
            t.part(c, Shape::Cube, &label_plate, Transform::from_xyz(0.0, -0.137, 0.012).with_scale(Vec3::new(0.12, 0.016, 0.004)), 0.0);
        });
    }

    // ------------------------------------------------ panel inferior: indicador circular e interruptores
    p.spawn((Transform::from_xyz(0.0, -0.60, -0.66).with_rotation(Quat::from_rotation_x(-0.75)), Visibility::default()))
        .with_children(|c| {
            t.part(c, Shape::Cube, &body, Transform::from_xyz(0.0, 0.0, -0.02).with_scale(Vec3::new(0.66, 0.26, 0.03)), 0.005);
            c.spawn((Mesh3d(meshes.add(Rectangle::new(0.23, 0.23))), MeshMaterial3d(dial_mat), Transform::from_xyz(0.0, 0.0, 0.0)));
            for i in 0..4 {
                for side in [-1.0f32, 1.0] {
                    let x = side * (0.16 + i as f32 * 0.045);
                    t.part(c, Shape::Cyl, &screw, Transform::from_xyz(x, 0.04, -0.003).with_rotation(Quat::from_rotation_x(1.5708)).with_scale(Vec3::new(0.012, 0.01, 0.012)), 0.002);
                    // palanquita del interruptor, inclinada arriba o abajo
                    let up = (i + if side > 0.0 { 1 } else { 0 }) % 2 == 0;
                    t.part(
                        c,
                        Shape::Cyl,
                        &switch_metal,
                        Transform::from_xyz(x, 0.04 + if up { 0.012 } else { -0.012 }, 0.012)
                            .with_rotation(Quat::from_rotation_x(if up { 0.9 } else { 2.2 }))
                            .with_scale(Vec3::new(0.004, 0.03, 0.004)),
                        0.002,
                    );
                    c.spawn((Mesh3d(t.cube.clone()), MeshMaterial3d(if up { osb_tip.clone() } else { red.clone() }), Transform::from_xyz(x, -0.04, 0.0).with_scale(Vec3::new(0.012, 0.008, 0.004)), BlinkLight(i as f32 * 0.9 + side)));
                }
            }
        });
    let _ = amber;

    // ------------------------------------------------ marco de la cabina
    let th = 0.05;
    for s in [-1.0f32, 1.0] {
        t.beam(p, Shape::Cube, &frame, Vec3::new(s * 0.95, -0.42, -0.95), Vec3::new(s * 0.34, 0.62, -1.18), th, 0.006);
        t.beam(p, Shape::Cube, &frame, Vec3::new(s * 0.34, 0.62, -1.18), Vec3::new(s * 0.42, 0.78, 0.4), th, 0.006);
        // alféizar lateral
        t.part(p, Shape::Cube, &body, Transform::from_xyz(s * 0.78, -0.55, -0.25).with_scale(Vec3::new(0.32, 0.22, 1.5)), 0.008);
        // arco trasero
        t.beam(p, Shape::Cube, &frame, Vec3::new(s * 0.70, -0.40, 0.48), Vec3::new(s * 0.40, 0.62, 0.42), th, 0.006);
    }
    t.beam(p, Shape::Cube, &frame, Vec3::new(-0.34, 0.62, -1.18), Vec3::new(0.34, 0.62, -1.18), th, 0.006);
    t.beam(p, Shape::Cube, &frame, Vec3::new(-0.40, 0.62, 0.42), Vec3::new(0.40, 0.62, 0.42), th, 0.006);

    // ------------------------------------------------ mandos
    // palanca (derecha)
    p.spawn((Transform::from_xyz(0.25, -0.80, -0.50), Visibility::default(), StickPivot)).with_children(|c| {
        t.part(c, Shape::Cyl, &stick, Transform::from_xyz(0.0, 0.15, 0.0).with_scale(Vec3::new(0.022, 0.30, 0.022)), 0.004);
        t.part(c, Shape::Capsule, &stick, Transform::from_xyz(0.0, 0.34, 0.0).with_rotation(Quat::from_rotation_x(-0.25)).with_scale(Vec3::new(0.075, 0.11, 0.085)), 0.005);
        c.spawn((Mesh3d(t.cube.clone()), MeshMaterial3d(green_cap.clone()), Transform::from_xyz(0.0, 0.43, 0.0).with_rotation(Quat::from_rotation_x(-0.25)).with_scale(Vec3::new(0.06, 0.012, 0.07))));
        c.spawn((Mesh3d(t.sphere.clone()), MeshMaterial3d(red.clone()), Transform::from_xyz(-0.01, 0.40, -0.045).with_scale(Vec3::splat(0.014))));
        // guante y antebrazo (sólo en la vista POV; en las demás está el piloto 3D)
        c.spawn((Transform::default(), Visibility::default(), PovOnly)).with_children(|c| {
        t.part(c, Shape::Sphere, &glove, Transform::from_xyz(0.025, 0.33, 0.03).with_scale(Vec3::new(0.062, 0.058, 0.07)), 0.004);
        t.part(c, Shape::Capsule, &glove, Transform::from_xyz(-0.01, 0.39, 0.0).with_rotation(Quat::from_rotation_z(0.9)).with_scale(Vec3::new(0.035, 0.05, 0.035)), 0.003);
        // antebrazo derecho
        t.beam(c, Shape::Capsule, &sleeve, Vec3::new(0.06, 0.30, 0.08), Vec3::new(0.10, 0.02, 0.42), 0.09, 0.005);
        t.part(c, Shape::Cyl, &cuff, Transform::from_xyz(0.065, 0.27, 0.12).with_rotation(Quat::from_rotation_x(1.0)).with_scale(Vec3::new(0.05, 0.03, 0.05)), 0.003);
        });
    });
    // acelerador (izquierda)
    p.spawn((Transform::from_xyz(-0.34, -0.46, -0.46), Visibility::default(), ThrottleHandle)).with_children(|c| {
        t.part(c, Shape::Cube, &hazard, Transform::from_scale(Vec3::new(0.05, 0.07, 0.14)), 0.004);
        c.spawn((Transform::default(), Visibility::default(), PovOnly)).with_children(|c| {
            t.part(c, Shape::Sphere, &glove, Transform::from_xyz(0.012, 0.02, 0.03).with_scale(Vec3::new(0.06, 0.06, 0.075)), 0.004);
            t.beam(c, Shape::Capsule, &sleeve, Vec3::new(-0.02, 0.0, 0.09), Vec3::new(-0.06, -0.26, 0.42), 0.09, 0.005);
            t.part(c, Shape::Cyl, &cuff, Transform::from_xyz(-0.025, -0.02, 0.12).with_rotation(Quat::from_rotation_x(1.0)).with_scale(Vec3::new(0.05, 0.03, 0.05)), 0.003);
        });
    });
    t.part(p, Shape::Cube, &body, Transform::from_xyz(-0.36, -0.52, -0.40).with_scale(Vec3::new(0.08, 0.05, 0.36)), 0.005);

    // ------------------------------------------------ piloto (oculto en vista POV)
    p.spawn((Transform::default(), Visibility::default(), PilotBody)).with_children(|c| {
        // piloto 3D del usuario (glTF con esqueleto), sentado con pose calculada al cargar
        c.spawn((WorldAssetRoot(pilot_scene.clone()), Transform::from_xyz(0.0, -1.6, 0.1), SeatedPilot));
        // asiento
        t.part(c, Shape::Cube, &seat, Transform::from_xyz(0.0, 0.0, 0.30).with_scale(Vec3::new(0.30, 0.26, 0.10)), 0.006);
        t.part(c, Shape::Cube, &seat, Transform::from_xyz(0.0, -0.40, 0.36).with_scale(Vec3::new(0.52, 0.62, 0.12)), 0.006);
    });
    screens
}

// ---------------------------------------------------------------- pantallas

pub fn update_screens(
    time: Res<Time<Real>>,
    mut acc: Local<f32>,
    game: Res<Game>,
    tgt: Res<TgtInfo>,
    screens: Res<Screens>,
    mut images: ResMut<Assets<Image>>,
    player: Single<(&Transform, &Player)>,
    enemies: Query<(Entity, &Transform, &Enemy), Without<Player>>,
    carriers: Query<&Transform, (With<CarrierShip>, Without<Player>)>,
    rings: Query<&Transform, (With<RearmRing>, Without<Player>)>,
    allies: Query<&Transform, (With<crate::allies::Ally>, Without<Player>)>,
    mecha: Res<crate::mecha::Mecha>,
) {
    *acc += time.delta_secs();
    if *acc < 0.07 {
        return;
    }
    *acc = 0.0;
    let (tf, pl) = *player;
    let t = time.elapsed_secs();
    let pi = std::f32::consts::PI;
    let blink = (t * 5.0).sin() > 0.0;
    let dim = |col: [u8; 3]| [col[0] / 4, col[1] / 4, col[2] / 4];
    let fwd = tf.forward();
    let pitch = fwd.y.clamp(-1.0, 1.0).asin();
    let r = tf.right();
    let u = tf.up();
    let roll = (-r.y).atan2(u.y);
    let hdg = (fwd.x.atan2(-fwd.z).to_degrees() + 360.0) % 360.0;
    let agl = if game.zone == Zone::Atmosphere { tf.translation.y - ground_height(tf.translation.x, tf.translation.z) } else { 9999.0 };
    let fighters = enemies.iter().filter(|e| e.2.kind == EnemyKind::Fighter && e.2.hp > 0.0).count();

    // ================= pantalla principal
    let mut c = Canvas::new(MAIN_W, MAIN_H);
    c.clear([10, 12, 22]);
    // anunciadores superiores
    let ann: [(&str, bool, [u8; 3]); 8] = [
        ("MASTER", game.hull < 50.0 && blink, [255, 170, 40]),
        ("LOCK", game.locked, [255, 70, 70]),
        ("HEAT", game.overheated || game.heat > 0.8, [255, 90, 60]),
        ("SHLD", game.shield > 99.0, [90, 230, 120]),
        ("MSL", game.missiles > 0, [130, 170, 255]),
        ("ALT", agl < 250.0 && blink, [255, 70, 70]),
        ("BOOST", pl.boosting, [255, 200, 60]),
        ("REARM", game.rearm_cd <= 0.0, [90, 220, 230]),
    ];
    for (i, (lbl, on, col)) in ann.iter().enumerate() {
        let x = 8 + i as i32 * 95;
        c.rect(x, 6, 88, 24, if *on { *col } else { [28, 32, 44] });
        let w = Canvas::text_width(lbl, 2);
        c.text(x + 44 - w / 2, 11, 2, lbl, if *on { [12, 12, 18] } else { dim(*col) });
    }
    // columna izquierda: acelerador y velocidad
    c.text(14, 44, 2, "THR", [150, 160, 190]);
    let bands: [[u8; 3]; 7] = [[90, 220, 110], [150, 230, 90], [210, 230, 80], [240, 200, 60], [245, 150, 50], [240, 100, 60], [230, 60, 70]];
    let lit = (pl.throttle * 7.0).ceil() as usize;
    for (i, col) in bands.iter().enumerate() {
        let k = 6 - i;
        let w_top = 150.0 - k as f32 * 20.0;
        let on = (6 - k) < lit || pl.boosting;
        c.trapezoid(100.0, 62 + k as i32 * 22, 19, w_top, w_top - 20.0, if on { *col } else { dim(*col) });
    }
    c.digits(22, 230, 34, &format!("{:03}", pl.speed.min(999.0) as i32), [250, 225, 120]);
    c.text(140, 254, 2, "M/S", [150, 160, 190]);
    c.text(14, 290, 2, &format!("ALT {:>5}", (tf.translation.y.max(0.0)) as i32), [150, 230, 170]);
    c.text(14, 312, 2, &format!("G   {:>4.1}", 1.0 + pl.ang_vel.x.abs() * pl.speed / 90.0), [150, 230, 170]);

    // centro: horizonte artificial
    let (ax, ay, ar) = (380.0f32, 186.0f32, 112.0f32);
    let ppd = 3.2; // píxeles por grado
    let off = pitch.to_degrees() * ppd;
    let (sr, cr) = roll.sin_cos();
    for y in (ay - ar) as i32..=(ay + ar) as i32 {
        for x in (ax - ar) as i32..=(ax + ar) as i32 {
            let dx = x as f32 - ax;
            let dy = y as f32 - ay;
            if dx * dx + dy * dy > ar * ar {
                continue;
            }
            // coordenadas en el marco del horizonte (rotado por el alabeo)
            let hx = dx * cr - dy * sr;
            let hy = dx * sr + dy * cr;
            let rel = hy - off;
            let mut col = if rel < 0.0 { [70, 110, 230] } else { [165, 100, 60] };
            if rel.abs() < 1.6 {
                col = [250, 250, 250];
            } else {
                for k in [-30.0f32, -20.0, -10.0, 10.0, 20.0, 30.0] {
                    let w = if (k as i32) % 20 == 0 { 34.0 } else { 20.0 };
                    if (rel + k * ppd).abs() < 1.1 && hx.abs() < w {
                        col = [240, 240, 240];
                    }
                }
            }
            c.put(x, y, col);
        }
    }
    c.ring(ax, ay, ar, ar + 5.0, 0.0, 2.0 * pi, [200, 205, 215]);
    for k in -3..=3 {
        let a = -pi / 2.0 + k as f32 * pi / 12.0;
        c.line(ax + a.cos() * (ar + 6.0), ay + a.sin() * (ar + 6.0), ax + a.cos() * (ar + 16.0), ay + a.sin() * (ar + 16.0), 2.5, [200, 205, 215]);
    }
    let ra = -pi / 2.0 - roll;
    c.disc(ax + ra.cos() * (ar - 8.0), ay + ra.sin() * (ar - 8.0), 5.0, [255, 200, 60]);
    // símbolo del avión
    c.rect((ax - 52.0) as i32, ay as i32 - 2, 34, 5, [255, 170, 40]);
    c.rect((ax + 18.0) as i32, ay as i32 - 2, 34, 5, [255, 170, 40]);
    c.disc(ax, ay, 5.0, [255, 170, 40]);
    let hdg_s = format!("HDG {:03}", hdg as i32);
    c.text(ax as i32 - Canvas::text_width(&hdg_s, 2) / 2, 310, 2, &hdg_s, [150, 230, 170]);

    // derecha: radar
    let (rx, ry, rr) = (642.0f32, 176.0f32, 106.0f32);
    c.disc(rx, ry, rr + 5.0, [200, 210, 220]);
    c.disc(rx, ry, rr, [18, 70, 46]);
    for k in [0.33f32, 0.66] {
        c.ring(rx, ry, rr * k - 1.0, rr * k + 1.0, 0.0, 2.0 * pi, [60, 150, 100]);
    }
    c.line(rx - rr, ry, rx + rr, ry, 1.5, [60, 150, 100]);
    c.line(rx, ry - rr, rx, ry + rr, 1.5, [60, 150, 100]);
    let sweep = t * 2.4;
    for k in 0..14 {
        let a = sweep - k as f32 * 0.05;
        let f = 1.0 - k as f32 / 14.0;
        let col = [(60.0 + 100.0 * f) as u8, (150.0 + 105.0 * f) as u8, (100.0 + 90.0 * f) as u8];
        c.line(rx, ry, rx + a.cos() * rr, ry + a.sin() * rr, 2.0, col);
    }
    let range = if game.zone == Zone::Atmosphere { 4500.0 } else { 6500.0 };
    let inv = tf.rotation.inverse();
    for (e, etf, en) in &enemies {
        if en.hp <= 0.0 {
            continue;
        }
        let rel = inv * (etf.translation - tf.translation);
        let mut v = Vec2::new(rel.x, rel.z) / range;
        if v.length() > 1.0 {
            v = v.normalize();
        }
        let (col, sz) = match en.kind {
            EnemyKind::Fighter => ([255, 80, 120], 5.0),
            EnemyKind::Turret => ([255, 190, 60], 4.0),
            EnemyKind::Mothership => ([255, 90, 255], 9.0),
            EnemyKind::Hive => ([255, 140, 40], 10.0),
        };
        let (px, py) = (rx + v.x * (rr - 6.0), ry + v.y * (rr - 6.0));
        c.disc(px, py, sz, col);
        if game.lock == Some(e) {
            c.frame(px as i32 - 9, py as i32 - 9, 18, 18, 2, if game.locked { [255, 60, 60] } else { [255, 255, 120] });
        }
    }
    for (ct, col, sz) in carriers
        .iter()
        .map(|t| (t.translation, [110u8, 170u8, 255u8], 8i32))
        .chain(rings.iter().map(|t| (t.translation, [90u8, 255u8, 170u8], 4i32)))
        .chain(allies.iter().map(|t| (t.translation, [80u8, 200u8, 255u8], 3i32)))
    {
        let rel = inv * (ct - tf.translation);
        let mut v = Vec2::new(rel.x, rel.z) / range;
        if v.length() > 1.0 {
            v = v.normalize();
        }
        let (px, py) = ((rx + v.x * (rr - 6.0)) as i32, (ry + v.y * (rr - 6.0)) as i32);
        c.rect(px - sz, py - sz / 2, sz * 2, sz, col);
    }
    c.poly(&[(rx, ry - 8.0), (rx + 6.0, ry + 6.0), (rx - 6.0, ry + 6.0)], [255, 255, 255]);
    if game.jam > 0.0 {
        // interferencia: estática sobre el radar
        let mut rng = rand::thread_rng();
        for y in (ry - rr) as i32..(ry + rr) as i32 {
            for x in (rx - rr) as i32..(rx + rr) as i32 {
                let (dx, dy) = (x as f32 - rx, y as f32 - ry);
                if dx * dx + dy * dy < rr * rr {
                    let v: u8 = rng.gen_range(0..180);
                    c.put(x, y, [v / 2, v, v / 2]);
                }
            }
        }
        let w = Canvas::text_width("NO SIGNAL", 3);
        c.text(rx as i32 - w / 2, ry as i32 - 10, 3, "NO SIGNAL", [255, 80, 80]);
    }
    c.text(540, 300, 2, &format!("RDR {:.1}KM", range / 1000.0), [150, 230, 170]);
    c.text(540, 322, 2, &format!("TGT {}", fighters), [255, 120, 150]);
    if let Some(mut img) = images.get_mut(&screens.main) {
        img.data = Some(c.data);
    }

    // ================= indicador circular inferior
    let mut d = Canvas::new(DIAL, DIAL);
    d.clear([10, 12, 22]);
    let (cx, cy) = (128.0, 128.0);
    d.ring(cx, cy, 100.0, 106.0, 0.0, 2.0 * pi, [235, 235, 240]);
    for i in 0..24 {
        let a = i as f32 / 24.0 * 2.0 * pi;
        let r0 = if i % 2 == 0 { 84.0 } else { 92.0 };
        d.line(cx + a.cos() * r0, cy + a.sin() * r0, cx + a.cos() * 100.0, cy + a.sin() * 100.0, 3.0, [235, 235, 240]);
    }
    d.line(cx - 16.0, cy, cx + 16.0, cy, 4.0, [240, 60, 60]);
    d.line(cx, cy - 16.0, cx, cy + 16.0, 4.0, [240, 60, 60]);
    let a = pi * 0.75 + (pl.speed / 600.0).min(1.0) * pi * 1.5;
    for k in 0..14 {
        let w = 14.0 - k as f32;
        d.disc(cx + a.cos() * k as f32 * 6.0, cy + a.sin() * k as f32 * 6.0, w * 0.5, [250, 170, 40]);
    }
    d.rect(30, 128 - (fwd.y * 90.0) as i32 - 2, 50, 5, [240, 70, 70]);
    d.rect(26, 30, 4, 196, [90, 230, 110]);
    d.text(150, 214, 2, "SPD", [180, 180, 200]);
    if let Some(mut img) = images.get_mut(&screens.dial) {
        img.data = Some(d.data);
    }

    // ================= pantalla de estado (derecha)
    let mut st = Canvas::new(SIDE_W, SIDE_H);
    st.clear([10, 12, 22]);
    st.text(10, 8, 2, "VF-STATUS", [150, 230, 170]);
    st.rect(10, 26, 364, 2, [60, 90, 80]);
    let (sx, sy, sc) = (110.0f32, 160.0f32, 95.0f32);
    let hull_col: [u8; 3] = if game.hull > 60.0 { [80, 220, 120] } else if game.hull > 30.0 { [250, 190, 50] } else if blink { [255, 60, 60] } else { [120, 30, 30] };
    let tp = |pts: &[(f32, f32)]| -> Vec<(f32, f32)> { pts.iter().map(|(x, y)| (sx + x * sc, sy + y * sc)).collect() };
    // escudo: elipse alrededor de la nave
    let sh = (game.shield / 100.0).clamp(0.0, 1.0);
    if sh > 0.01 {
        for k in 0..3 {
            let rr2 = 112.0 + k as f32 * 4.0;
            st.ring(sx, sy, rr2, rr2 + 2.0, -pi / 2.0, -pi / 2.0 + 2.0 * pi * sh, [80 + 30 * k as u8, 140, 255]);
        }
    }
    let fus = tp(&[(0.0, -1.0), (0.09, -0.62), (0.13, -0.15), (0.14, 0.88), (-0.14, 0.88), (-0.13, -0.15), (-0.09, -0.62)]);
    for s2 in [-1.0f32, 1.0] {
        let wing = tp(&[(0.12 * s2, -0.05), (0.78 * s2, 0.42), (0.78 * s2, 0.56), (0.12 * s2, 0.48)]);
        st.poly(&wing, hull_col);
        st.poly_outline(&wing, 2.0, [230, 240, 235]);
        let tail = tp(&[(0.12 * s2, 0.58), (0.36 * s2, 0.86), (0.36 * s2, 0.98), (0.12 * s2, 0.92)]);
        st.poly(&tail, hull_col);
        st.poly_outline(&tail, 2.0, [230, 240, 235]);
    }
    st.poly(&fus, hull_col);
    st.poly_outline(&fus, 2.0, [230, 240, 235]);
    st.disc(sx, sy - 0.45 * sc, 7.0, [40, 60, 120]);
    // datos
    let rows: [(&str, String, [u8; 3]); 6] = [
        ("HULL", format!("{:>3}%", game.hull.max(0.0) as i32), hull_col),
        ("SHLD", format!("{:>3}%", game.shield as i32), [120, 170, 255]),
        ("MSL", format!("{}/8 R{}", game.missiles, game.cluster), [200, 210, 255]),
        ("LSR", if game.overheated { "HOT".into() } else { format!("{:>3}%", (100.0 - game.heat * 100.0).max(0.0) as i32) }, [255, 170, 80]),
        ("MODE", match mecha.mode { crate::mecha::Mode::Fighter => "FTR".into(), crate::mecha::Mode::Gerwalk => "GRW".into(), crate::mecha::Mode::Battroid => "BTR".into() }, [150, 230, 170]),
        ("BASE", format!("{:>3}%", (game.carrier_hp / CARRIER_HP * 100.0).max(0.0) as i32), if game.carrier_hp < CARRIER_HP * 0.35 { [255, 90, 70] } else { [120, 200, 255] }),
    ];
    for (i, (k, v, col)) in rows.iter().enumerate() {
        let y = 44 + i as i32 * 38;
        st.text(232, y, 2, k, [140, 150, 175]);
        st.text(290, y, 2, v, *col);
        st.rect(232, y + 20, 140, 2, [30, 40, 50]);
    }
    if let Some(mut img) = images.get_mut(&screens.status) {
        img.data = Some(st.data);
    }

    // ================= capa de datos de la cámara de blanco (transparente)
    let mut o = Canvas::new(SIDE_W, SIDE_H);
    o.clear_transparent();
    let g: [u8; 3] = [140, 255, 180];
    for y in (0..SIDE_H as i32).step_by(4) {
        for x in 0..SIDE_W as i32 {
            o.put_a(x, y, [0, 0, 0], 50);
        }
    }
    let (w, h) = (SIDE_W as i32, SIDE_H as i32);
    for (x, y, dx, dy) in [(8, 8, 1, 1), (w - 9, 8, -1, 1), (8, h - 9, 1, -1), (w - 9, h - 9, -1, -1)] {
        o.rect(x.min(x + dx * 28), y, 28, 3, g);
        o.rect(x, y.min(y + dy * 28), 3, 28, g);
    }
    o.text(16, 16, 2, "TGT CAM", g);
    if game.jam > 0.0 {
        let mut rng = rand::thread_rng();
        for _ in 0..4000 {
            let (x, y) = (rng.gen_range(0..w), rng.gen_range(0..h));
            o.put_a(x, y, [200, 255, 220], rng.gen_range(60..220));
        }
        o.text(w / 2 - Canvas::text_width("NO SIGNAL", 3) / 2, h / 2 - 10, 3, "NO SIGNAL", [255, 80, 80]);
    } else if tgt.has {
        let col = if tgt.locked { [255, 80, 80] } else { g };
        let bx = 54 + ((1.0 - tgt.zoom.min(1.0)) * 20.0) as i32;
        o.frame(w / 2 - bx, h / 2 - bx * 3 / 4, bx * 2, bx * 3 / 2, 2, col);
        let name = match tgt.kind {
            Some(EnemyKind::Fighter) => "INVID SCOUT",
            Some(EnemyKind::Turret) => "AA TURRET",
            Some(EnemyKind::Mothership) => "MOTHERSHIP",
            Some(EnemyKind::Hive) => "INVID HIVE",
            None => "",
        };
        o.text(16, h - 60, 2, name, g);
        o.text(16, h - 36, 2, &format!("RNG {:.2}KM", tgt.dist / 1000.0), g);
        o.text(w - 120, 16, 2, &format!("Z{:>4}X", (50.0 / tgt.zoom.to_degrees().max(0.5)) as i32), g);
        if tgt.locked && blink {
            o.text(w - 92, h - 36, 2, "LOCK", [255, 80, 80]);
        }
        let hw = ((tgt.hp.clamp(0.0, 1.0)) * 100.0) as i32;
        o.rect(w - 120, h - 60, 104, 12, [40, 60, 50]);
        o.rect(w - 118, h - 58, hw, 8, if tgt.hp > 0.4 { g } else { [255, 120, 80] });
    } else {
        o.text(w / 2 - Canvas::text_width("NO TGT", 3) / 2, h / 2 - 10, 3, "NO TGT", if blink { g } else { [60, 120, 80] });
    }
    o.line(w as f32 / 2.0 - 14.0, h as f32 / 2.0, w as f32 / 2.0 + 14.0, h as f32 / 2.0, 1.0, g);
    o.line(w as f32 / 2.0, h as f32 / 2.0 - 14.0, w as f32 / 2.0, h as f32 / 2.0 + 14.0, 1.0, g);
    if let Some(mut img) = images.get_mut(&screens.tgt_overlay) {
        img.data = Some(o.data);
    }
}

/// Apunta la cámara de blanco al blanco fijado (o al caza más cercano al frente).
pub fn target_camera(
    game: Res<Game>,
    dir: Res<Director>,
    mut info: ResMut<TgtInfo>,
    mut frame: Local<u32>,
    player: Single<&Transform, (With<Player>, Without<TargetCam>)>,
    enemies: Query<(Entity, &Transform, &Enemy), (Without<Player>, Without<TargetCam>)>,
    mut cam: Single<(&mut Transform, &mut Projection, &mut Camera), With<TargetCam>>,
) {
    *frame += 1;
    let (ref mut ctf, ref mut proj, ref mut camera) = *cam;
    // sólo se renderiza cuando la cabina está a la vista, y a medio ritmo
    camera.is_active = game.started && dir.current().interior() && *frame % 2 == 0;
    let p = player.translation;
    let f = player.forward().as_vec3();
    let mut best: Option<(Entity, Vec3, f32, &Enemy)> = None;
    if let Some(l) = game.lock {
        if let Ok((e, t, en)) = enemies.get(l) {
            best = Some((e, t.translation, t.translation.distance(p), en));
        }
    }
    if best.is_none() {
        for (e, t, en) in &enemies {
            if en.hp <= 0.0 || en.kind == EnemyKind::Turret {
                continue;
            }
            let to = t.translation - p;
            let d = to.length();
            if d < 5000.0 && to.dot(f) > 0.3 * d && best.as_ref().is_none_or(|b| d < b.2) {
                best = Some((e, t.translation, d, en));
            }
        }
    }
    let origin = p + f * 14.0;
    match best {
        Some((e, pos, d, en)) => {
            let size = match en.kind {
                EnemyKind::Mothership => 520.0,
                EnemyKind::Hive => 420.0,
                EnemyKind::Turret => 14.0,
                EnemyKind::Fighter => 16.0,
            };
            let fov = (2.0 * (size * 0.5 / d.max(1.0)).atan() * 1.6).clamp(0.4f32.to_radians(), 55f32.to_radians());
            **ctf = Transform::from_translation(origin).looking_at(pos, player.up().as_vec3());
            if let Projection::Perspective(pp) = proj.as_mut() {
                pp.fov = fov;
            }
            *info = TgtInfo { has: true, dist: d, zoom: fov, kind: Some(en.kind), locked: game.locked && game.lock == Some(e), hp: en.hp / en.max_hp };
        }
        None => {
            **ctf = Transform::from_translation(origin).looking_to(f, player.up().as_vec3());
            if let Projection::Perspective(pp) = proj.as_mut() {
                pp.fov = 40f32.to_radians();
            }
            *info = TgtInfo::default();
        }
    }
}

pub fn cockpit_anim(
    time: Res<Time>,
    game: Res<Game>,
    mut needles: Query<(&mut Transform, &Needle), (Without<StickPivot>, Without<ThrottleHandle>, Without<EngineGlow>)>,
    mut caution: Query<&mut Visibility, (With<MasterCaution>, Without<BlinkLight>)>,
    player: Single<&Player>,
    mut stick: Single<&mut Transform, (With<StickPivot>, Without<ThrottleHandle>)>,
    mut lever: Single<&mut Transform, (With<ThrottleHandle>, Without<StickPivot>)>,
    mut blinks: Query<(&mut Visibility, &BlinkLight)>,
    mut glows: Query<&mut Transform, (With<EngineGlow>, Without<StickPivot>, Without<ThrottleHandle>)>,
) {
    let i = player.input;
    let target = Quat::from_euler(EulerRot::XYZ, -i.x * 0.30, 0.0, i.z * 0.30 + i.y * 0.12);
    stick.rotation = stick.rotation.slerp(target, 0.25);
    lever.translation.z = -0.40 - player.throttle * 0.12 - if player.boosting { 0.04 } else { 0.0 };
    let t = time.elapsed_secs();
    for (mut v, b) in &mut blinks {
        *v = if ((t * 1.5 + b.0) * 2.0).sin() > -0.3 { Visibility::Inherited } else { Visibility::Hidden };
    }
    let pi = std::f32::consts::PI;
    for (mut tf, n) in &mut needles {
        let v = match n.0 {
            0 => (player.speed / 600.0).clamp(0.0, 1.0),
            _ => game.heat.clamp(0.0, 1.0),
        };
        // 0 → abajo-izquierda, 1 → abajo-derecha (arco de 270°)
        let target = Quat::from_rotation_z(pi * 0.75 - v * pi * 1.5);
        tf.rotation = tf.rotation.slerp(target, 0.2);
    }
    let warn = game.hull < 50.0 || game.hit_flash > 0.15 || game.overheated;
    let on = warn && (t * 6.0).sin() > 0.0;
    for mut v in &mut caution {
        *v = if on { Visibility::Inherited } else { Visibility::Hidden };
    }
    let flame = 0.25 + player.throttle * 0.35 + if player.boosting { 0.5 } else { 0.0 } + (t * 40.0).sin() * 0.03;
    for mut g in &mut glows {
        g.scale.z = flame;
    }
}

// ---------------------------------------------------------------- director de cámaras

pub fn director(
    real: Res<Time<Real>>,
    mut vtime: ResMut<Time<Virtual>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut game: ResMut<Game>,
    mut dir: ResMut<Director>,
    player: Single<(&Transform, &Player)>,
    mut cam: Single<(&mut Transform, &mut Projection), (With<WorldCam>, Without<Player>)>,
    mut cockpit: Single<&mut Visibility, (With<CockpitRoot>, Without<PlayerModel>, Without<PilotBody>)>,
    mut model: Single<&mut Visibility, (With<PlayerModel>, Without<CockpitRoot>, Without<PilotBody>)>,
    mut pilot: Single<&mut Visibility, (With<PilotBody>, Without<CockpitRoot>, Without<PlayerModel>)>,
    mut pov_only: Query<&mut Visibility, (With<PovOnly>, Without<PilotBody>, Without<CockpitRoot>, Without<PlayerModel>)>,
    intro: Option<Res<Intro>>,
    enemies: Query<&Transform, (With<Enemy>, Without<Player>, Without<WorldCam>)>,
    fighters: Query<(Entity, &Transform, &Enemy), (Without<Player>, Without<WorldCam>)>,
) {
    let mut nearest_fighter: Vec<Entity> = Vec::new();
    if let Some((e, _, _)) = fighters
        .iter()
        .filter(|f| f.2.kind == EnemyKind::Fighter && f.2.hp > 0.0)
        .min_by(|a, b| a.1.translation.distance_squared(dir.cam.translation).total_cmp(&b.1.translation.distance_squared(dir.cam.translation)))
    {
        nearest_fighter.push(e);
    }
    if intro.is_some_and(|i| i.phase != IntroPhase::Off) {
        return;
    }
    let dt = real.delta_secs().min(0.1);
    let (ptf, pl) = *player;
    let mut rng = rand::thread_rng();

    if keys.just_pressed(KeyCode::KeyC) {
        let i = CamView::CYCLE.iter().position(|v| *v == dir.view).unwrap_or(0);
        dir.view = CamView::CYCLE[(i + 1) % CamView::CYCLE.len()];
        dir.cut = None;
        dir.flyby_set = false;
    }
    for (k, v) in [
        (KeyCode::Digit1, CamView::Pov),
        (KeyCode::Digit2, CamView::Shoulder),
        (KeyCode::Digit3, CamView::FacePilot),
        (KeyCode::Digit4, CamView::Chase),
        (KeyCode::Digit5, CamView::Wing),
        (KeyCode::Digit6, CamView::Flyby),
    ] {
        if keys.just_pressed(k) {
            dir.view = v;
            dir.cut = None;
            dir.flyby_set = false;
        }
    }
    if keys.just_pressed(KeyCode::KeyX) {
        dir.auto = !dir.auto;
        let m = if dir.auto { "Cortes cinematográficos: ACTIVADOS" } else { "Cortes cinematográficos: DESACTIVADOS" };
        game.message(m, 2.0);
    }

    // Cortes por eventos
    if dir.auto && game.started && !game.dead {
        if game.ev_zone {
            dir.cut = Some((CamView::Flyby, 3.2));
            dir.flyby_set = false;
        } else if game.ev_cluster {
            // salva en racimo: cámara de persecución en cámara lenta para ver el abanico
            dir.cut = Some((CamView::Chase, 2.6));
            dir.slowmo = 0.9;
        } else if game.ev_missile && dir.cut.is_none() {
            dir.cut = Some((CamView::StickClose, 0.75));
        } else if game.ev_bighit && dir.cut.is_none() && rng.gen_bool(0.5) {
            dir.cut = Some((CamView::FacePilot, 0.9));
        }
        if game.ev_lock && dir.cut.is_none() {
            dir.close_target = game.lock;
            dir.cut = Some((CamView::EnemyClose, 1.4));
        }
        if let Some((kp, kv)) = game.ev_kill_pos {
            let force = std::env::var("NAVE_VIEW").as_deref() == Ok("kill");
            if force || (rng.gen_bool(0.45) && dir.cut.is_none()) {
                dir.kill_pos = kp;
                let side = Vec3::new(rng.gen_range(-1.0..1.0), rng.gen_range(0.1..0.6), rng.gen_range(-1.0..1.0)).normalize();
                dir.kill_anchor = kp + side * 60.0;
                dir.kill_vel = kv;
                dir.kill_t = 0.0;
                dir.cut = Some((CamView::KillCam, 1.3));
                dir.slowmo = 1.1;
            } else if game.ev_kill && rng.gen_bool(0.5) {
                dir.slowmo = 0.7;
            }
        }
    }
    game.ev_zone = false;
    game.ev_missile = false;
    game.ev_cluster = false;
    game.ev_bighit = false;
    game.ev_kill = false;
    game.ev_lock = false;
    game.ev_kill_pos = None;
    if let Some((v, tleft)) = dir.cut {
        let tl = tleft - dt;
        dir.cut = if tl <= 0.0 { None } else { Some((v, tl)) };
        if dir.cut.is_none() && v == CamView::Flyby {
            dir.flyby_set = false;
        }
    }
    dir.slowmo -= dt;
    let speed = if dir.slowmo > 0.0 && !game.paused { 0.3 } else { 1.0 };
    if (vtime.relative_speed() - speed).abs() > 0.01 {
        vtime.set_relative_speed(speed);
    }

    let mut view = dir.current();
    if game.dead {
        view = CamView::Chase;
    }
    let p = ptf.translation;
    let rot = ptf.rotation;
    let fwd = ptf.forward().as_vec3();
    let up = ptf.up().as_vec3();
    let right = ptf.right().as_vec3();
    let av = pl.ang_vel;
    let tt = real.elapsed_secs();
    let shake = game.shake * game.shake + if game.zone == Zone::Atmosphere { (pl.speed / 600.0).powi(2) * 0.12 } else { 0.0 };
    let jit = Vec3::new((tt * 47.0).sin() + (tt * 23.0).sin() * 0.5, (tt * 53.0).sin() + (tt * 31.0).cos() * 0.5, 0.0) * shake;

    let local_look = |from: Vec3, at: Vec3, up_l: Vec3| -> Transform {
        let world_from = ptf.transform_point(from);
        let world_at = ptf.transform_point(at);
        Transform::from_translation(world_from).looking_at(world_at, rot * up_l)
    };

    let (cam_tf, fov) = match view {
        CamView::Pov => {
            // la cabeza se inclina con las fuerzas G
            let head = Vec3::new(-av.y * 0.025, -av.x * 0.02 + 0.02, 0.0) + jit * 0.012;
            let look = Quat::from_euler(EulerRot::YXZ, av.y * 0.05, -0.13 + av.x * 0.03, -av.z * 0.025 + jit.x * 0.002);
            (Transform::from_translation(ptf.transform_point(head)).with_rotation(rot * look), 74.0)
        }
        CamView::Shoulder => {
            let mut t = local_look(Vec3::new(0.27, 0.30, 0.44) + jit * 0.01, Vec3::new(-0.08, -0.20, -1.3), Vec3::Y);
            t.rotate_local_z(-av.z * 0.03);
            (t, 72.0)
        }
        CamView::FacePilot => (local_look(Vec3::new(0.06, -0.12, -0.60) + jit * 0.01, Vec3::new(0.0, 0.0, 0.08), Vec3::Y), 62.0),
        CamView::StickClose => (local_look(Vec3::new(0.58, -0.24, -0.66) + jit * 0.005, Vec3::new(0.22, -0.48, -0.44), Vec3::Y), 50.0),
        CamView::EnemyClose => {
            // primer plano del Invid fijado, con nuestra nave al fondo
            if dir.close_target.and_then(|e| enemies.get(e).ok()).is_none() {
                dir.close_target = game.lock.or_else(|| nearest_fighter.first().copied());
            }
            match dir.close_target.and_then(|e| enemies.get(e).ok()) {
                Some(et) => {
                    let ep = et.translation;
                    let to_p = (p - ep).normalize_or(Vec3::Z);
                    let side = to_p.cross(Vec3::Y).normalize_or(Vec3::X);
                    let k = dir.close_target.and_then(|e| fighters.get(e).ok()).map(|f| (f.2.radius / 8.0).max(1.0)).unwrap_or(1.0);
                    let cpos = ep + to_p * 42.0 * k + side * 16.0 * k + Vec3::Y * 9.0 * k;
                    (Transform::from_translation(cpos).looking_at(ep + Vec3::Y * 2.0, Vec3::Y), 30.0)
                }
                None => {
                    dir.cut = None;
                    (local_look(Vec3::new(0.27, 0.30, 0.44), Vec3::new(-0.08, -0.20, -1.3), Vec3::Y), 72.0)
                }
            }
        }
        CamView::KillCam => {
            // la cámara acompaña a los restos mientras se separan
            dir.kill_t += dt * if dir.slowmo > 0.0 { 0.3 } else { 1.0 };
            let off = dir.kill_vel * dir.kill_t;
            (Transform::from_translation(dir.kill_anchor + off * 0.85).looking_at(dir.kill_pos + off, Vec3::Y), 42.0)
        }
        CamView::Wing => (local_look(Vec3::new(-17.0, 3.0, -3.0) + jit * 0.05, Vec3::new(0.0, 0.0, 0.5), Vec3::Y), 50.0),
        CamView::Chase => {
            let back = if game.dead { 70.0 } else { 26.0 };
            if dir.chase_pos == Vec3::ZERO {
                dir.chase_rot = rot;
                dir.chase_pos = Vec3::ONE;
            }
            // inercia sólo en la rotación: la cámara "se queda atrás" en los giros
            dir.chase_rot = dir.chase_rot.slerp(rot, 1.0 - (-3.0 * dt).exp());
            dir.chase_pos = p + dir.chase_rot * Vec3::new(0.0, 6.5, back);
            let look_at = p + fwd * 30.0;
            let t = Transform::from_translation(dir.chase_pos + jit * 0.4).looking_at(look_at, dir.chase_rot * Vec3::Y);
            (t, 66.0)
        }
        CamView::Flyby => {
            let behind = (dir.flyby_anchor - p).dot(fwd) < -60.0;
            if !dir.flyby_set || behind || dir.flyby_anchor.distance(p) > 2500.0 {
                let ahead = (pl.speed * 2.3).clamp(220.0, 900.0);
                let side = if rng.gen_bool(0.5) { 1.0 } else { -1.0 };
                let mut a = p + fwd * ahead + right * side * rng.gen_range(25.0..60.0) + up * rng.gen_range(-12.0..22.0);
                if game.zone == Zone::Atmosphere {
                    a.y = a.y.max(ground_height(a.x, a.z) + 15.0);
                }
                dir.flyby_anchor = a;
                dir.flyby_set = true;
            }
            let d = dir.flyby_anchor.distance(p);
            let fov = (2400.0 / d.max(30.0)).clamp(18.0, 60.0);
            (Transform::from_translation(dir.flyby_anchor).looking_at(p, Vec3::Y), fov)
        }
    };
    let (ref mut ctf, ref mut proj) = *cam;
    **ctf = cam_tf;
    dir.cam = cam_tf;
    if let Projection::Perspective(pp) = proj.as_mut() {
        let target = fov.to_radians();
        pp.fov += (target - pp.fov) * (1.0 - (-8.0 * dt).exp());
        if view != dir.view || (pp.fov - target).abs() > 0.5 {
            // los cortes son instantáneos
            if dir.cut.is_some() {
                pp.fov = target;
            }
        }
    }
    let interior = view.interior() && !game.dead;
    **cockpit = if interior { Visibility::Inherited } else { Visibility::Hidden };
    **model = if interior || game.dead { Visibility::Hidden } else { Visibility::Inherited };
    **pilot = if view == CamView::Pov { Visibility::Hidden } else { Visibility::Inherited };
    for mut v in &mut pov_only {
        *v = if view == CamView::Pov { Visibility::Inherited } else { Visibility::Hidden };
    }
}

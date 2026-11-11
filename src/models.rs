//! Modelos 3D del usuario (glTF): el caza Valkyrie y el piloto con esqueleto Mixamo.
//! - El Valkyrie es la nave del jugador en las vistas externas y aparece aparcado en la pista.
//! - El piloto se sienta en la cabina con una pose calculada por código (IK de dos huesos
//!   para que las manos lleguen a la palanca y al acelerador).
//! - Escena de título: el piloto camina junto a su nave al atardecer y corre a ella al despegar.

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI};

use bevy::image::Image;
use bevy::light::GeneratedEnvironmentMapLight;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension};
use bevy::asset::RenderAssetUsages;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady};
use rand::Rng;

use crate::components::*;
use crate::toon::*;
use crate::world::*;

pub const VALK_PATH: &str = "models/valkyrie.glb";
pub const PILOT_PATH: &str = "models/pilot.glb";
pub const INVID_PATH: &str = "models/invid.glb";
pub const VALK_FLIGHT_PATH: &str = "models/valkyrie_flight.glb";
pub const CARRIER_PATH: &str = "models/carrier.glb";
pub const MOTHER_PATH: &str = "models/invid_mothership.glb";

// ---- nave nodriza terrestre (modelo con el morro hacia -X)
pub const CARRIER_SCALE: f32 = 330.0;
pub const CARRIER_HALF: Vec3 = Vec3::new(0.40, 0.15, 0.90);
pub const CARRIER_POS_ATMO: Vec3 = Vec3::new(0.0, 2650.0, 2400.0);
pub const CARRIER_POS_SPACE: Vec3 = Vec3::new(-300.0, 300.0, 2600.0);
/// Anillo de recarga: sobre la popa, a la altura de la cubierta.
pub const REARM_OFFSET: Vec3 = Vec3::new(0.0, 95.0, 420.0);
pub const REARM_RADIUS: f32 = 30.0;
pub fn carrier_model_transform() -> Transform {
    Transform::from_rotation(Quat::from_rotation_y(-FRAC_PI_2)).with_scale(Vec3::splat(CARRIER_SCALE))
}

// ---- nave nodriza Invid (orientada en diagonal en el archivo: la alineamos con su eje mayor)
pub const MOTHER_SCALE: f32 = 420.0;
const MOTHER_CENTER: Vec3 = Vec3::new(0.0525, 0.0402, 0.1727);
const MOTHER_YAW: f32 = 0.6121; // 35.07°
const MOTHER_ZC: f32 = -0.13;
pub const MOTHER_HALF: Vec3 = Vec3::new(0.55, 0.23, 0.81);
/// Puntos de la superficie superior (espacio de la caja), donde van las torretas.
pub const MOTHER_TURRETS: [Vec3; 6] = [
    Vec3::new(-0.12, 0.22, -0.47),
    Vec3::new(0.12, 0.197, -0.47),
    Vec3::new(-0.12, 0.167, -0.17),
    Vec3::new(0.12, 0.176, -0.17),
    Vec3::new(-0.12, 0.188, 0.43),
    Vec3::new(0.12, 0.217, 0.43),
];
pub fn mothership_model_transform() -> Transform {
    let r = Quat::from_rotation_y(MOTHER_YAW);
    Transform {
        translation: -(r * MOTHER_CENTER + Vec3::new(0.0, 0.0, MOTHER_ZC)) * MOTHER_SCALE,
        rotation: r,
        scale: Vec3::splat(MOTHER_SCALE),
    }
}

// ---- Valkyrie de vuelo (cabina cerrada, morro hacia -X, ya nivelado)
const VALK_FLIGHT_EYE: Vec3 = Vec3::new(-0.36, 0.03, 0.0);
pub fn valk_flight_transform() -> Transform {
    let r = Quat::from_rotation_y(-FRAC_PI_2);
    Transform { translation: -(r * (VALK_FLIGHT_EYE * VALK_SCALE)), rotation: r, scale: Vec3::splat(VALK_SCALE) }
}
pub const INVID_PARTS_PATH: &str = "models/invid_parts.glb";
/// Altura del Invid en el juego ≈ 1.9 × 5.5 ≈ 10.5 m.
pub const INVID_SCALE: f32 = 5.5;
/// El modelo partido es el mismo Invid a 1/15.84 de escala y apoyado en y = 0.
const INVID_SPLIT_K: f32 = 15.84;
pub const VALK_SCALE: f32 = 8.4;
/// Posición de los ojos del piloto dentro del modelo original del Valkyrie.
const VALK_EYE: Vec3 = Vec3::new(-0.33, 0.15, 0.0);

/// El modelo apunta su morro hacia -X y viene algo alabeado: lo nivelamos y giramos a -Z.
pub fn valk_rot() -> Quat {
    Quat::from_rotation_y(-FRAC_PI_2) * Quat::from_rotation_x(-0.214)
}
/// El Invid mira a +Z: lo giramos para que avance hacia -Z, inclinado como en vuelo.
pub fn invid_transform() -> Transform {
    Transform {
        translation: Vec3::ZERO,
        rotation: Quat::from_rotation_x(-0.45) * Quat::from_rotation_y(PI),
        scale: Vec3::splat(INVID_SCALE),
    }
}
/// Transform del modelo partido para que coincida exactamente con el modelo completo.
pub fn invid_split_transform() -> Transform {
    invid_transform().mul_transform(Transform {
        translation: Vec3::new(0.0, -0.952, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(INVID_SPLIT_K),
    })
}

/// Restos de un Invid destruido: sus 6 piezas salen despedidas.
#[derive(Component)]
pub struct Wreck {
    pub vel: Vec3,
}

/// Una pieza del Invid en vuelo.
#[derive(Component)]
pub struct Chunk {
    pub vel: Vec3,
    pub spin: Vec3,
    pub smoke: f32,
    pub age: f32,
}

/// Transform del Valkyrie relativo a los ojos del piloto (origen del jugador).
pub fn valk_cockpit_transform() -> Transform {
    Transform {
        translation: -(valk_rot() * (VALK_EYE * VALK_SCALE)),
        rotation: valk_rot(),
        scale: Vec3::splat(VALK_SCALE),
    }
}

#[derive(Resource)]
pub struct Models {
    pub valk: Handle<WorldAsset>,
    pub pilot: Handle<WorldAsset>,
    pub graph: Handle<AnimationGraph>,
    pub run: AnimationNodeIndex,
    pub walk: AnimationNodeIndex,
    pub env_dusk: Handle<Image>,
    pub env_space: Handle<Image>,
}

/// Piloto sentado en la cabina (pose calculada al cargar).
#[derive(Component)]
pub struct SeatedPilot;

/// Piloto animado de la escena de título.
#[derive(Component)]
pub struct IntroPilot;

#[derive(Component)]
pub struct IntroEntity;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntroPhase {
    Off,
    Title,
    Scramble,
}

#[derive(Resource)]
pub struct Intro {
    pub phase: IntroPhase,
    pub t: f32,
    pub pad: Vec3,
    pub pilot_pos: Vec3,
    pub pilot_dir: Vec3,
    pub angle: f32,
    pub anim_set: Option<bool>, // Some(true) = corriendo
}

/// Cubemap de cielo generado (para los reflejos del metal del Valkyrie).
fn sky_cubemap(space: bool) -> Image {
    let n = 64usize;
    let mut data = Vec::with_capacity(n * n * 6 * 4);
    for face in 0..6 {
        for y in 0..n {
            for x in 0..n {
                let u = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
                let d = match face {
                    0 => Vec3::new(1.0, -v, -u),
                    1 => Vec3::new(-1.0, -v, u),
                    2 => Vec3::new(u, 1.0, v),
                    3 => Vec3::new(u, -1.0, -v),
                    4 => Vec3::new(u, -v, 1.0),
                    _ => Vec3::new(-u, -v, -1.0),
                }
                .normalize();
                let mut c = sky_color(d, space);
                if !space && d.y < 0.0 {
                    // suelo del desierto reflejado
                    c = Vec3::new(0.62, 0.40, 0.42).lerp(c, (1.0 + d.y * 6.0).clamp(0.0, 1.0));
                }
                let c = c.min(Vec3::ONE);
                let to8 = |f: f32| (f.max(0.0).powf(1.0 / 2.2) * 255.0) as u8;
                data.extend_from_slice(&[to8(c.x), to8(c.y), to8(c.z), 255]);
            }
        }
    }
    let mut img = Image::new(
        Extent3d { width: n as u32, height: (n * 6) as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.reinterpret_stacked_2d_as_array(6).expect("cubemap");
    img.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    img
}

pub fn load_models(
    mut commands: Commands,
    mut a: ResMut<GameAssets>,
    server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (graph, idx) = AnimationGraph::from_clips([
        server.load(GltfAssetLabel::Animation(0).from_asset(PILOT_PATH)),
        server.load(GltfAssetLabel::Animation(1).from_asset(PILOT_PATH)),
    ]);
    a.invid = server.load(GltfAssetLabel::Scene(0).from_asset(INVID_PATH));
    a.invid_parts = server.load(GltfAssetLabel::Scene(0).from_asset(INVID_PARTS_PATH));
    a.valk_flight = server.load(GltfAssetLabel::Scene(0).from_asset(VALK_FLIGHT_PATH));
    a.valk_blue = server.load(GltfAssetLabel::Scene(0).from_asset("models/valkyrie_blue.glb"));
    a.gerwalk = server.load(GltfAssetLabel::Scene(0).from_asset("models/valkyrie_gerwalk.glb"));
    a.battroid = server.load(GltfAssetLabel::Scene(0).from_asset("models/valkyrie_battroid.glb"));
    a.boss = server.load(GltfAssetLabel::Scene(0).from_asset("models/invid_boss.glb"));
    a.spartan = server.load(GltfAssetLabel::Scene(0).from_asset("models/destroid_spartan.glb"));
    a.shock = server.load(GltfAssetLabel::Scene(0).from_asset("models/invid_shock.glb"));
    a.hive = server.load(GltfAssetLabel::Scene(0).from_asset("models/invid_hive.glb"));
    a.carrier = server.load(GltfAssetLabel::Scene(0).from_asset(CARRIER_PATH));
    a.mothership = server.load(GltfAssetLabel::Scene(0).from_asset(MOTHER_PATH));
    commands.insert_resource(Models {
        valk: server.load(GltfAssetLabel::Scene(0).from_asset(VALK_PATH)),
        pilot: server.load(GltfAssetLabel::Scene(0).from_asset(PILOT_PATH)),
        graph: graphs.add(graph),
        run: idx[0],
        walk: idx[1],
        env_dusk: images.add(sky_cubemap(false)),
        env_space: images.add(sky_cubemap(true)),
    });
}

/// Luz del sol (para los modelos con materiales PBR) y reflejos del cielo.
pub fn setup_lighting(mut commands: Commands, models: Res<Models>, cam: Single<Entity, With<WorldCam>>, tcam: Query<Entity, With<crate::cockpit::TargetCam>>) {
    for e in &tcam {
        commands.entity(e).insert(GeneratedEnvironmentMapLight {
            environment_map: models.env_dusk.clone(),
            intensity: 1500.0,
            ..default()
        });
    }
    use bevy::light::CascadeShadowConfigBuilder;
    commands.spawn((
        DirectionalLight {
            illuminance: 6500.0,
            color: Color::srgb(1.0, 0.74, 0.58),
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 25.0,
            maximum_distance: 260.0,
            ..default()
        }
        .build(),
        Transform::default().looking_to(-SUN_DIR, Vec3::Y),
        SunLight,
    ));
    commands.entity(*cam).insert(GeneratedEnvironmentMapLight {
        environment_map: models.env_dusk.clone(),
        intensity: 1500.0,
        ..default()
    });
}

/// Ajusta sol y reflejos según la zona.
pub fn zone_lighting(
    game: Res<Game>,
    models: Res<Models>,
    mut sun: Single<(&mut DirectionalLight, &mut Transform), With<SunLight>>,
    mut envs: Query<&mut GeneratedEnvironmentMapLight>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut last: Local<Option<Zone>>,
) {
    if *last == Some(game.zone) {
        return;
    }
    *last = Some(game.zone);
    let (ref mut l, ref mut tf) = *sun;
    match game.zone {
        Zone::Atmosphere => {
            l.color = Color::srgb(1.0, 0.74, 0.58);
            l.illuminance = 6500.0;
            **tf = Transform::default().looking_to(-SUN_DIR, Vec3::Y);
            for mut env in &mut envs {
                env.environment_map = models.env_dusk.clone();
            }
            ambient.color = Color::srgb(0.75, 0.55, 0.85);
            ambient.brightness = 250.0;
        }
        Zone::Space => {
            l.color = Color::srgb(1.0, 0.97, 0.92);
            l.illuminance = 12000.0;
            **tf = Transform::default().looking_to(-SPACE_SUN, Vec3::Y);
            for mut env in &mut envs {
                env.environment_map = models.env_space.clone();
            }
            ambient.color = Color::srgb(0.5, 0.5, 0.9);
            ambient.brightness = 80.0;
        }
    }
}

// ---------------------------------------------------------------- piloto sentado

const ARM_TARGETS: [(&str, &str, &str, Vec3, Vec3); 2] = [
    // (brazo, antebrazo, mano, objetivo en la cabina, polo del codo en espacio modelo)
    ("mixamorig:RightArm", "mixamorig:RightForeArm", "mixamorig:RightHand", Vec3::new(0.25, -0.45, -0.47), Vec3::new(-0.7, -1.0, -0.3)),
    ("mixamorig:LeftArm", "mixamorig:LeftForeArm", "mixamorig:LeftHand", Vec3::new(-0.33, -0.45, -0.43), Vec3::new(0.7, -1.0, -0.3)),
];

/// Direcciones deseadas de los huesos (espacio del modelo: mira a +Z, su derecha es -X).
fn seated_dirs() -> HashMap<&'static str, (&'static str, Vec3)> {
    let mut m = HashMap::new();
    for (b, c) in [("mixamorig:Spine", "mixamorig:Spine1"), ("mixamorig:Spine1", "mixamorig:Spine2"), ("mixamorig:Spine2", "mixamorig:Neck")] {
        m.insert(b, (c, Vec3::new(0.0, 1.0, -0.12)));
    }
    m.insert("mixamorig:Neck", ("mixamorig:Head", Vec3::new(0.0, 1.0, 0.08)));
    m.insert("mixamorig:LeftUpLeg", ("mixamorig:LeftLeg", Vec3::new(0.14, -0.12, 1.0)));
    m.insert("mixamorig:RightUpLeg", ("mixamorig:RightLeg", Vec3::new(-0.14, -0.12, 1.0)));
    m.insert("mixamorig:LeftLeg", ("mixamorig:LeftFoot", Vec3::new(0.04, -1.0, 0.3)));
    m.insert("mixamorig:RightLeg", ("mixamorig:RightFoot", Vec3::new(-0.04, -1.0, 0.3)));
    m.insert("mixamorig:LeftFoot", ("mixamorig:LeftToeBase", Vec3::new(0.0, -0.3, 1.0)));
    m.insert("mixamorig:RightFoot", ("mixamorig:RightToeBase", Vec3::new(0.0, -0.3, 1.0)));
    m
}

struct Node {
    e: Entity,
    parent: Option<usize>,
    name: String,
    local: Transform,
}

/// Recorre la jerarquía del modelo, calcula la pose sentada y escribe las rotaciones.
pub fn pose_seated_pilot(
    ev: On<WorldInstanceReady>,
    seated: Query<(), With<SeatedPilot>>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut tfs: Query<&mut Transform>,
) {
    let root = ev.entity;
    if seated.get(root).is_err() {
        return;
    }
    // aplanar la jerarquía en orden padre → hijo
    let mut nodes: Vec<Node> = Vec::new();
    let mut stack: Vec<(Entity, Option<usize>)> = children.get(root).map(|c| c.iter().map(|e| (e, None)).collect()).unwrap_or_default();
    while let Some((e, parent)) = stack.pop() {
        let local = tfs.get(e).map(|t| *t).unwrap_or_default();
        let name = names.get(e).map(|n| n.as_str().to_string()).unwrap_or_default();
        nodes.push(Node { e, parent, name, local });
        let idx = nodes.len() - 1;
        if let Ok(ch) = children.get(e) {
            for c in ch.iter() {
                stack.push((c, Some(idx)));
            }
        }
    }
    let by_name: HashMap<String, usize> = nodes.iter().enumerate().map(|(i, n)| (n.name.clone(), i)).collect();
    // pose de reposo en espacio modelo
    let mut bind: Vec<Transform> = Vec::with_capacity(nodes.len());
    for i in 0..nodes.len() {
        let t = match nodes[i].parent {
            Some(p) => bind[p].mul_transform(nodes[i].local),
            None => nodes[i].local,
        };
        bind.push(t);
    }
    let dirs = seated_dirs();

    // Calcula la pose; `arm_goals` = (nombre hueso → dirección deseada) para los brazos.
    let solve = |extra: &HashMap<String, Vec3>| -> (Vec<Quat>, Vec<Transform>) {
        let mut c: Vec<Quat> = vec![Quat::IDENTITY; nodes.len()];
        let mut posed: Vec<Transform> = Vec::with_capacity(nodes.len());
        let mut local_rot: Vec<Quat> = vec![Quat::IDENTITY; nodes.len()];
        for i in 0..nodes.len() {
            let cp = nodes[i].parent.map(|p| c[p]).unwrap_or(Quat::IDENTITY);
            let child_name = dirs.get(nodes[i].name.as_str()).map(|d| d.0.to_string()).or_else(|| match nodes[i].name.as_str() {
                "mixamorig:RightArm" => Some("mixamorig:RightForeArm".into()),
                "mixamorig:RightForeArm" => Some("mixamorig:RightHand".into()),
                "mixamorig:LeftArm" => Some("mixamorig:LeftForeArm".into()),
                "mixamorig:LeftForeArm" => Some("mixamorig:LeftHand".into()),
                _ => None,
            });
            let desired = extra.get(&nodes[i].name).copied().or_else(|| dirs.get(nodes[i].name.as_str()).map(|d| d.1.normalize()));
            c[i] = match (desired, child_name.and_then(|n| by_name.get(&n).copied())) {
                (Some(want), Some(ci)) => {
                    let d0 = (cp * (bind[ci].translation - bind[i].translation)).normalize_or(Vec3::Y);
                    Quat::from_rotation_arc(d0, want) * cp
                }
                _ => cp,
            };
            let posed_rot = c[i] * bind[i].rotation;
            let (parent_tf, parent_rot) = match nodes[i].parent {
                Some(p) => (posed[p], posed[p].rotation),
                None => (Transform::IDENTITY, Quat::IDENTITY),
            };
            local_rot[i] = parent_rot.inverse() * posed_rot;
            let lt = Transform { rotation: local_rot[i], ..nodes[i].local };
            posed.push(parent_tf.mul_transform(lt));
        }
        (local_rot, posed)
    };

    // 1) cuerpo y piernas
    let (_, posed0) = solve(&HashMap::new());
    // ojos ≈ cabeza + un poco arriba y adelante; el origen del jugador son los ojos
    let head = by_name.get("mixamorig:Head").map(|i| posed0[*i].translation).unwrap_or(Vec3::new(0.0, 1.6, 0.0));
    let eye_model = head + Vec3::new(0.0, 0.09, 0.07);
    let cabin_eye = Vec3::new(0.0, 0.0, 0.06);
    // cabina = r0 + Ry(π)·modelo
    let ry = Quat::from_rotation_y(PI);
    let r0 = cabin_eye - ry * eye_model;
    let to_model = |c: Vec3| ry.inverse() * (c - r0);

    // 2) brazos con IK de dos huesos
    let mut extra: HashMap<String, Vec3> = HashMap::new();
    for (arm, fore, hand, target_cabin, pole) in ARM_TARGETS {
        let (Some(&ia), Some(&iff), Some(&ih)) = (by_name.get(arm), by_name.get(fore), by_name.get(hand)) else { continue };
        let s = posed0[ia].translation;
        let l1 = (bind[iff].translation - bind[ia].translation).length();
        let l2 = (bind[ih].translation - bind[iff].translation).length();
        let mut t = to_model(target_cabin);
        let mut d = t - s;
        if d.length() > (l1 + l2) * 0.98 {
            d = d.normalize() * (l1 + l2) * 0.98;
            t = s + d;
        }
        let dist = d.length().max(1e-4);
        let u = d / dist;
        let a = (l1 * l1 - l2 * l2 + dist * dist) / (2.0 * dist);
        let h = (l1 * l1 - a * a).max(0.0).sqrt();
        let p = (pole - u * pole.dot(u)).normalize_or(Vec3::NEG_Y);
        let elbow = s + u * a + p * h;
        extra.insert(arm.to_string(), (elbow - s).normalize());
        extra.insert(fore.to_string(), (t - elbow).normalize());
    }
    let (rots, _) = solve(&extra);
    for (i, n) in nodes.iter().enumerate() {
        if n.name.starts_with("mixamorig:") {
            if let Ok(mut tf) = tfs.get_mut(n.e) {
                tf.rotation = rots[i];
            }
        }
    }
    if let Ok(mut tf) = tfs.get_mut(root) {
        tf.translation = r0;
        tf.rotation = ry;
    }
}

/// Inicia la animación del piloto de la escena de título.
pub fn start_intro_anim(
    ev: On<WorldInstanceReady>,
    intro_pilot: Query<(), With<IntroPilot>>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    models: Res<Models>,
    mut commands: Commands,
) {
    if intro_pilot.get(ev.entity).is_err() {
        return;
    }
    for c in children.iter_descendants(ev.entity) {
        if let Ok(mut p) = players.get_mut(c) {
            p.play(models.walk).repeat();
            commands.entity(c).insert(AnimationGraphHandle(models.graph.clone()));
        }
    }
}

// ---------------------------------------------------------------- escena de título

pub fn setup_intro(mut commands: Commands, game: Res<Game>, models: Res<Models>, a: Res<GameAssets>) {
    // Pista: buscar la altura máxima del terreno en la zona para que nada la atraviese.
    let c = Vec2::new(250.0, 6300.0);
    let mut h: f32 = 0.0;
    for x in -8..=8 {
        for z in -8..=8 {
            h = h.max(ground_height(c.x + x as f32 * 8.0, c.y + z as f32 * 8.0));
        }
    }
    let pad = Vec3::new(c.x, h + 1.0, c.y);
    let active = !game.started;
    commands.insert_resource(Intro {
        phase: if active { IntroPhase::Title } else { IntroPhase::Off },
        t: 0.0,
        pad,
        pilot_pos: pad + Vec3::new(9.0, 0.0, 5.0),
        pilot_dir: Vec3::NEG_Z,
        angle: 0.6,
        anim_set: None,
    });
    let t = a.toon.clone();
    // plataforma de despegue (permanece en el mundo)
    commands
        .spawn((Transform::from_translation(pad), Visibility::default(), ZoneEntity))
        .with_children(|p| {
            t.part(p, Shape::Cube, &a.m_building2, Transform::from_xyz(0.0, -6.0, 0.0).with_scale(Vec3::new(70.0, 12.0, 110.0)), 0.25);
            t.part(p, Shape::Cube, &a.m_turret, Transform::from_xyz(0.0, 0.02, 0.0).with_scale(Vec3::new(14.0, 0.05, 100.0)), 0.0);
            for k in 0..9 {
                p.spawn((
                    Mesh3d(a.plain_cube.clone()),
                    MeshMaterial3d(a.m_window.clone()),
                    Transform::from_xyz(0.0, 0.08, -44.0 + k as f32 * 11.0).with_scale(Vec3::new(0.6, 0.05, 5.0)),
                ));
            }
            for s in [-1.0f32, 1.0] {
                for k in 0..6 {
                    p.spawn((
                        Mesh3d(a.plain_sphere.clone()),
                        MeshMaterial3d(a.m_red_light.clone()),
                        Transform::from_xyz(s * 9.0, 0.3, -45.0 + k as f32 * 18.0).with_scale(Vec3::splat(0.35)),
                    ));
                }
            }
        });
    if !active {
        return;
    }
    // Valkyrie aparcado
    let mut vt = valk_cockpit_transform();
    vt.translation = pad + Vec3::new(0.0, 0.26 * VALK_SCALE + 0.2, 0.0);
    commands.spawn((WorldAssetRoot(models.valk.clone()), vt, IntroEntity));
    // Piloto animado
    commands.spawn((WorldAssetRoot(models.pilot.clone()), Transform::from_translation(pad + Vec3::new(9.0, 0.0, 5.0)), IntroPilot, IntroEntity));
}

pub fn intro_update(
    mut diff: ResMut<crate::missions::Difficulty>,
    real: Res<Time<Real>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut intro: ResMut<Intro>,
    mut game: ResMut<Game>,
    mut dir: ResMut<Director>,
    models: Res<Models>,
    mut commands: Commands,
    mut cursor: Single<&mut bevy::window::CursorOptions>,
    mut pilot: Query<(Entity, &mut Transform), (With<IntroPilot>, Without<WorldCam>)>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    to_remove: Query<Entity, With<IntroEntity>>,
    mut cam: Single<(&mut Transform, &mut Projection), With<WorldCam>>,
) {
    if intro.phase == IntroPhase::Off {
        return;
    }
    let dt = real.delta_secs().min(0.1);
    intro.t += dt;
    let pad = intro.pad;
    let ship_side = pad + Vec3::new(-5.5, 0.0, 3.0);

    if intro.phase == IntroPhase::Title && (keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::Tab)) {
        diff.0 = if diff.0 == crate::missions::Level::Basica { crate::missions::Level::Media } else { crate::missions::Level::Basica };
    }
    let auto_go = real.elapsed_secs() > 22.0 && std::env::var("NAVE_INTRO").as_deref() == Ok("scramble");
    if intro.phase == IntroPhase::Title && intro.t > 0.3 && (auto_go || mouse.just_pressed(MouseButton::Left) || keys.just_pressed(KeyCode::Enter)) {
        game.fade_dir = 0.0;
        intro.phase = IntroPhase::Scramble;
        intro.t = 0.0;
        cursor.grab_mode = bevy::window::CursorGrabMode::Locked;
        cursor.visible = false;
    }

    // movimiento del piloto
    let running = intro.phase == IntroPhase::Scramble;
    if running {
        let to = ship_side - intro.pilot_pos;
        let to = Vec3::new(to.x, 0.0, to.z);
        if to.length() > 0.6 {
            intro.pilot_dir = to.normalize();
            let step = intro.pilot_dir * 5.2 * dt;
            intro.pilot_pos += step;
        }
    } else {
        // camina en círculo junto a la nave
        let center = pad + Vec3::new(6.5, 0.0, 6.0);
        intro.angle += dt * 1.35 / 4.0;
        let a = intro.angle;
        intro.pilot_pos = center + Vec3::new(a.cos(), 0.0, a.sin()) * 4.0;
        intro.pilot_dir = Vec3::new(-a.sin(), 0.0, a.cos());
    }
    if intro.anim_set != Some(running) {
        let mut done = false;
        for (e, _) in &pilot {
            for c in children.iter_descendants(e) {
                if let Ok(mut p) = players.get_mut(c) {
                    p.stop_all();
                    p.play(if running { models.run } else { models.walk }).repeat();
                    done = true;
                }
            }
        }
        if done {
            intro.anim_set = Some(running);
        }
    }
    for (_, mut tf) in &mut pilot {
        tf.translation = intro.pilot_pos;
        let yaw = intro.pilot_dir.x.atan2(intro.pilot_dir.z);
        tf.rotation = tf.rotation.slerp(Quat::from_rotation_y(yaw), (8.0 * dt).min(1.0));
    }

    // cámara
    let (ref mut ctf, ref mut proj) = *cam;
    let (pos, look, fov): (Vec3, Vec3, f32) = if running {
        let d = intro.pilot_dir;
        let side = Vec3::new(-d.z, 0.0, d.x);
        (intro.pilot_pos - d * 4.0 + side * 1.2 + Vec3::Y * 1.3, intro.pilot_pos + d * 6.0 + Vec3::Y * 1.6, 48.0)
    } else {
        let a = 1.1 + intro.t * 0.04;
        let focus = pad.lerp(intro.pilot_pos, 0.45) + Vec3::Y * 1.8;
        (focus + Vec3::new(a.sin() * 15.0, 2.2, a.cos() * 15.0), focus, 42.0)
    };
    **ctf = Transform::from_translation(pos).looking_at(look, Vec3::Y);
    dir.cam = **ctf;
    if let Projection::Perspective(p) = proj.as_mut() {
        p.fov = fov.to_radians();
    }

    // fin del arranque: fundido a negro y despegue
    if running {
        if intro.t > 1.8 {
            game.fade = ((intro.t - 1.8) / 0.6).min(1.0);
        }
        if intro.t > 2.45 {
            intro.phase = IntroPhase::Off;
            for e in &to_remove {
                commands.entity(e).try_despawn();
            }
            game.started = true;
            game.fade = 1.0;
            game.fade_dir = -1.0;
            game.ev_zone = true;
            game.fresh = true;
        }
    } else {
        game.fade = (game.fade - dt * 0.8).max(0.0);
    }
}


// ---------------------------------------------------------------- restos de Invid

pub fn spawn_wreck(commands: &mut Commands, a: &GameAssets, enemy_tf: &Transform, vel: Vec3) {
    let tf = enemy_tf.mul_transform(invid_split_transform());
    commands.spawn((
        WorldAssetRoot(a.invid_parts.clone()),
        tf,
        Wreck { vel: vel * 0.15 },
        Effect { kind: EffectKind::Wreck, t: 0.0, life: 3.5 },
    ));
}

/// Cuando el modelo partido se carga, cada pieza recibe velocidad y giro propios.
pub fn wreck_ready(
    ev: On<WorldInstanceReady>,
    wrecks: Query<(), With<Wreck>>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut commands: Commands,
) {
    if wrecks.get(ev.entity).is_err() {
        return;
    }
    let mut rng = rand::thread_rng();
    for c in children.iter_descendants(ev.entity) {
        let Ok(n) = names.get(c) else { continue };
        if !n.as_str().starts_with("model_part") {
            continue;
        }
        // dirección de salida según dónde está la pieza (centro aproximado por índice)
        let dir = match n.as_str() {
            "model_part0" => Vec3::new(-1.0, 0.2, 0.4),
            "model_part1" => Vec3::new(-0.8, 1.0, 0.0),
            "model_part2" => Vec3::new(0.0, -1.0, 0.1),
            "model_part3" => Vec3::new(0.0, 1.0, -0.3),
            "model_part4" => Vec3::new(0.8, 1.0, 0.0),
            _ => Vec3::new(1.0, 0.2, 0.4),
        }
        .normalize();
        // velocidades en unidades del modelo partido (escala ≈ 87)
        let speed = rng.gen_range(0.12..0.25);
        commands.entity(c).insert(Chunk {
            vel: dir * speed + Vec3::new(rng.gen_range(-0.05..0.05), rng.gen_range(0.0..0.08), rng.gen_range(-0.05..0.05)),
            spin: Vec3::new(rng.gen_range(-4.0..4.0), rng.gen_range(-4.0..4.0), rng.gen_range(-4.0..4.0)),
            smoke: 0.0,
            age: 0.0,
        });
    }
}

pub fn update_wrecks(
    time: Res<Time>,
    game: Res<Game>,
    a: Res<GameAssets>,
    mut commands: Commands,
    mut wrecks: Query<(&mut Transform, &mut Wreck), Without<Chunk>>,
    mut chunks: Query<(&mut Transform, &mut Chunk, &GlobalTransform), Without<Wreck>>,
) {
    let dt = time.delta_secs();
    for (mut tf, mut w) in &mut wrecks {
        if game.zone == Zone::Atmosphere {
            w.vel.y -= 9.8 * dt;
        }
        tf.translation += w.vel * dt;
    }
    for (mut tf, mut c, gt) in &mut chunks {
        c.age += dt;
        tf.translation += c.vel * dt;
        if game.zone == Zone::Atmosphere {
            c.vel.y -= 0.03 * dt;
        }
        tf.rotate(Quat::from_scaled_axis(c.spin * dt));
        c.smoke -= dt;
        if c.smoke <= 0.0 && c.age < 2.2 {
            c.smoke = 0.14;
            crate::gameplay::spawn_puff(&mut commands, &a, gt.translation(), 0.8 + c.age * 0.8, 0.9);
            if c.age < 0.6 {
                commands.spawn((
                    Mesh3d(a.plain_sphere.clone()),
                    MeshMaterial3d(a.m_fire.clone()),
                    Transform::from_translation(gt.translation()).with_scale(Vec3::splat(0.1)),
                    Effect { kind: EffectKind::Fireball { size: 2.5 }, t: 0.0, life: 0.35 },
                ));
            }
        }
    }
}

/// Los materiales del Invid vienen muy claros para la luz del atardecer:
/// los oscurecemos un poco (una sola vez, el material es compartido).
pub fn tame_invid_materials(
    ev: On<WorldInstanceReady>,
    parents: Query<&ChildOf>,
    enemies: Query<(), Or<(With<Enemy>, With<Wreck>)>>,
    children: Query<&Children>,
    mat_q: Query<&MeshMaterial3d<StandardMaterial>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut done: Local<std::collections::HashSet<AssetId<StandardMaterial>>>,
    enemy_kind: Query<&Enemy>,
    mut commands: Commands,
) {
    let is_enemy = enemies.get(ev.entity).is_ok() || parents.get(ev.entity).is_ok_and(|p| enemies.get(p.parent()).is_ok());
    if !is_enemy {
        return;
    }
    let fighter = enemies.get(ev.entity).is_err();
    for c in children.iter_descendants(ev.entity) {
        if let Ok(m) = mat_q.get(c) {
            if fighter && parents.get(ev.entity).is_ok_and(|p| enemy_kind.get(p.parent()).is_ok_and(|e| e.kind == EnemyKind::Fighter)) {
                commands.entity(c).insert(bevy::camera::visibility::VisibilityRange {
                    start_margin: 0.0..0.0,
                    end_margin: 1500.0..1700.0,
                    use_aabb: false,
                });
            }
            if done.insert(m.0.id()) {
                if let Some(mut mat) = mats.get_mut(&m.0) {
                    mat.base_color = Color::srgb(0.72, 0.70, 0.78);
                    mat.perceptual_roughness = 0.55;
                }
            }
        }
    }
}

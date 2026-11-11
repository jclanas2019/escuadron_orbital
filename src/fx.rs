//! Efectos cinematográficos:
//! * Misiles en racimo (micromisiles con trayectorias curvas, estilo "circo Itano").
//! * Estelas de humo continuas con contorno de tinta para todos los misiles.
//! * Explosiones por capas: destello, rayos, bolas de fuego de tres colores, onda de choque,
//!   humo oscuro, brasas, luz puntual y detonaciones secundarias.

use std::collections::VecDeque;
use std::f32::consts::TAU;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use rand::Rng;

use crate::allies::Ally;
use crate::components::*;
use crate::sfx::{sfx, SfxKind};
use crate::world::GameAssets;

// ---------------------------------------------------------------- recursos

#[derive(Resource)]
pub struct FxAssets {
    pub m_flash: Handle<StandardMaterial>,
    pub m_spike: Handle<StandardMaterial>,
    pub m_fire_hot: Handle<StandardMaterial>,
    pub m_fire_deep: Handle<StandardMaterial>,
    pub m_smoke_dark: Handle<StandardMaterial>,
    pub m_ember: Handle<StandardMaterial>,
    pub m_flare: Handle<StandardMaterial>,
    pub m_trail: Handle<StandardMaterial>,
    pub ring: Handle<Mesh>,
}

pub fn setup_fx(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    use crate::toon::{flat, glow};
    let m = &mut *mats;
    let fx = FxAssets {
        m_flash: glow(m, 16.0, 14.0, 11.0),
        m_spike: glow(m, 7.0, 4.0, 1.0),
        m_fire_hot: glow(m, 4.2, 2.1, 0.45),
        m_fire_deep: glow(m, 1.5, 0.32, 0.07),
        m_smoke_dark: flat(m, Color::srgb(0.30, 0.25, 0.33)),
        m_ember: glow(m, 12.0, 4.5, 0.7),
        m_flare: glow(m, 18.0, 13.0, 4.0),
        m_trail: m.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            ..default()
        }),
        ring: meshes.add(Annulus::new(0.93, 1.0).mesh().resolution(64)),
    };
    commands.insert_resource(fx);
    commands.insert_resource(ClusterQueue::default());
}

// ---------------------------------------------------------------- explosiones

/// Petición de explosión: la crea `gameplay::spawn_explosion` y la construye `booms`.
#[derive(Component)]
pub struct Boom {
    pub pos: Vec3,
    pub size: f32,
    pub vel: Vec3,
}

#[derive(Clone, Copy)]
pub enum PartKind {
    /// Bola que crece y se apaga.
    Pop { size: f32 },
    /// Rayo de luz que sale del centro (estrella de anime).
    Spike { dir: Vec3, len: f32, w: f32 },
    /// Onda de choque plana mirando a la cámara.
    Ring { size: f32 },
    /// Humo oscuro que crece y sube.
    Smoke { size: f32 },
    /// Brasa que cae.
    Ember { vel: Vec3 },
    /// Luz puntual del destello.
    Light { power: f32 },
    /// Detonación secundaria retardada.
    Delayed { size: f32, vel: Vec3 },
}

#[derive(Component)]
pub struct FxPart {
    pub kind: PartKind,
    pub t: f32,
    pub life: f32,
    pub origin: Vec3,
}

fn rv(rng: &mut impl Rng) -> Vec3 {
    loop {
        let v = Vec3::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0));
        let l = v.length_squared();
        if l > 0.01 && l <= 1.0 {
            return v / l.sqrt();
        }
    }
}

fn part(commands: &mut Commands, mesh: Handle<Mesh>, mat: Handle<StandardMaterial>, tf: Transform, kind: PartKind, life: f32, t0: f32) -> Entity {
    let origin = tf.translation;
    commands
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            tf.with_scale(Vec3::splat(0.001)),
            FxPart { kind, t: t0, life, origin },
            NotShadowCaster,
            ZoneEntity,
        ))
        .id()
}

#[allow(clippy::too_many_arguments)]
pub fn booms(
    mut commands: Commands,
    q: Query<(Entity, &Boom)>,
    a: Res<GameAssets>,
    fx: Res<FxAssets>,
    dir: Res<Director>,
    game: Res<Game>,
    lights: Query<(), With<FxLightTag>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let mut rng = rand::thread_rng();
    let mut n_lights = lights.iter().count();
    let cam = dir.cam.translation;
    for (e, b) in &q {
        commands.entity(e).despawn();
        let (pos, s, vel) = (b.pos, b.size, b.vel);
        sfx(&mut commands, SfxKind::Explosion { size: s }, Some(pos), 1.0);
        let sphere = a.plain_sphere.clone();
        // 1) destello blanco
        part(&mut commands, sphere.clone(), fx.m_flash.clone(), Transform::from_translation(pos), PartKind::Pop { size: s * 1.7 }, 0.13, 0.0);
        // 2) luz puntual (pocas a la vez)
        if s >= 8.0 && n_lights < 8 && pos.distance(cam) < 4000.0 {
            n_lights += 1;
            commands.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.72, 0.38),
                    intensity: 0.0,
                    range: s * 45.0,
                    ..default()
                },
                Transform::from_translation(pos),
                FxPart { kind: PartKind::Light { power: 2.5e7 * (s / 10.0).powi(2).min(40.0) }, t: 0.0, life: 0.35 + s * 0.006, origin: pos },
                FxLightTag,
                ZoneEntity,
            ));
        }
        // 3) rayos (estrella de anime)
        let spikes = if s >= 8.0 { 10 } else { 6 };
        for _ in 0..spikes {
            let d = rv(&mut rng);
            let len = s * rng.gen_range(1.4..3.2);
            let tf = Transform::from_translation(pos).with_rotation(Quat::from_rotation_arc(Vec3::Z, d));
            part(&mut commands, a.plain_cube.clone(), fx.m_spike.clone(), tf, PartKind::Spike { dir: d, len, w: s * rng.gen_range(0.035..0.07) }, rng.gen_range(0.18..0.32), 0.0);
        }
        // 4) bolas de fuego: núcleo amarillo, naranja y rojo profundo
        for _ in 0..4 {
            let o = rv(&mut rng) * s * 0.25;
            part(&mut commands, sphere.clone(), fx.m_fire_hot.clone(), Transform::from_translation(pos + o), PartKind::Pop { size: s * rng.gen_range(0.55..0.85) }, rng.gen_range(0.32..0.5), 0.0);
        }
        for _ in 0..6 {
            let o = rv(&mut rng) * s * 0.45;
            part(&mut commands, sphere.clone(), a.m_fire.clone(), Transform::from_translation(pos + o), PartKind::Pop { size: s * rng.gen_range(0.5..1.0) }, rng.gen_range(0.5..0.9), 0.0);
        }
        for _ in 0..4 {
            let o = rv(&mut rng) * s * 0.6;
            part(&mut commands, sphere.clone(), fx.m_fire_deep.clone(), Transform::from_translation(pos + o), PartKind::Pop { size: s * rng.gen_range(0.8..1.25) }, rng.gen_range(0.85..1.3), -0.08);
        }
        // 5) onda de choque
        if s >= 6.0 {
            let mat = mats.add(StandardMaterial {
                base_color: Color::LinearRgba(LinearRgba::new(1.6, 1.15, 0.8, 0.8)),
                unlit: true,
                alpha_mode: AlphaMode::Add,
                cull_mode: None,
                double_sided: true,
                fog_enabled: false,
                ..default()
            });
            let face = (cam - pos).normalize_or(Vec3::Z);
            let tf = Transform::from_translation(pos).with_rotation(Quat::from_rotation_arc(Vec3::Z, face));
            part(&mut commands, fx.ring.clone(), mat, tf, PartKind::Ring { size: s }, 0.45, 0.0);
            if s >= 20.0 {
                // segundo anillo horizontal, más lento
                let mat2 = mats.add(StandardMaterial {
                    base_color: Color::LinearRgba(LinearRgba::new(0.9, 0.8, 0.95, 0.5)),
                    unlit: true,
                    alpha_mode: AlphaMode::Add,
                    cull_mode: None,
                    double_sided: true,
                    fog_enabled: false,
                    ..default()
                });
                let tf2 = Transform::from_translation(pos).with_rotation(Quat::from_rotation_arc(Vec3::Z, Vec3::Y));
                part(&mut commands, fx.ring.clone(), mat2, tf2, PartKind::Ring { size: s * 1.6 }, 0.9, 0.0);
            }
        }
        // 6) humo claro (el de siempre) y humo oscuro con contorno, que aparece después
        for _ in 0..5 {
            let o = rv(&mut rng) * s * 0.5;
            crate::gameplay::spawn_puff(&mut commands, &a, pos + o, s * 0.8, 2.0);
        }
        for _ in 0..4 {
            let o = rv(&mut rng) * s * 0.55;
            let id = part(&mut commands, sphere.clone(), fx.m_smoke_dark.clone(), Transform::from_translation(pos + o), PartKind::Smoke { size: s * rng.gen_range(0.7..1.05) }, rng.gen_range(2.2..3.2), -rng.gen_range(0.2..0.45));
            commands.entity(id).with_children(|c| {
                c.spawn((Mesh3d(a.toon.sphere.clone()), MeshMaterial3d(a.toon.ink.clone()), Transform::from_scale(Vec3::splat(1.08)), NotShadowCaster));
            });
        }
        // 7) escombros oscuros (como antes) y brasas
        for i in 0..10 {
            let mat = if i % 3 == 0 { a.m_fire.clone() } else { a.m_debris.clone() };
            let sz = s * 0.06 * (0.5 + rng.gen_range(0.0..1.0));
            commands.spawn((
                Mesh3d(a.plain_cube.clone()),
                MeshMaterial3d(mat),
                Transform::from_translation(pos).with_scale(Vec3::splat(sz)),
                Effect { kind: EffectKind::Debris { vel: vel * 0.6 + rv(&mut rng) * s * 5.0, spin: rv(&mut rng) * 8.0 }, t: 0.0, life: 2.5 },
                NotShadowCaster,
            ));
        }
        for _ in 0..8 {
            let v = vel * 0.5 + rv(&mut rng) * s * rng.gen_range(4.0..9.0);
            let tf = Transform::from_translation(pos);
            let id = part(&mut commands, a.plain_cube.clone(), fx.m_ember.clone(), tf, PartKind::Ember { vel: v }, rng.gen_range(1.0..1.9), 0.0);
            commands.entity(id).insert(Transform::from_translation(pos).with_scale(Vec3::splat(s * 0.035)));
        }
        // 8) detonaciones secundarias en las grandes
        if s >= 18.0 {
            for _ in 0..rng.gen_range(2..4) {
                let o = rv(&mut rng) * s * 0.7;
                commands.spawn((
                    Transform::from_translation(pos + o),
                    FxPart { kind: PartKind::Delayed { size: s * rng.gen_range(0.35..0.55), vel }, t: 0.0, life: rng.gen_range(0.15..0.6), origin: pos + o },
                    ZoneEntity,
                ));
            }
        }
        let _ = &game;
    }
}

#[derive(Component)]
pub struct FxLightTag;

pub fn fx_parts(
    time: Res<Time>,
    mut commands: Commands,
    game: Res<Game>,
    dir: Res<Director>,
    mut q: Query<(Entity, &mut FxPart, &mut Transform, Option<&mut PointLight>, Option<&MeshMaterial3d<StandardMaterial>>, Option<&mut Visibility>)>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs();
    let cam = dir.cam.translation;
    let atmo = game.zone == Zone::Atmosphere;
    for (e, mut p, mut tf, light, mat, vis) in &mut q {
        p.t += dt;
        if p.t < 0.0 {
            tf.scale = Vec3::splat(0.001);
            continue;
        }
        let k = (p.t / p.life).clamp(0.0, 1.0);
        match p.kind {
            PartKind::Pop { size } => {
                let s = size * (0.25 + k.powf(0.35)) * (1.0 - k.powi(3));
                tf.scale = Vec3::splat(s.max(0.001));
            }
            PartKind::Spike { dir: d, len, w } => {
                let c = (std::f32::consts::PI * k.powf(0.6)).sin().max(0.0);
                let l = len * (0.3 + 0.7 * k.powf(0.4));
                tf.scale = Vec3::new(w * c, w * c, l * 0.5 + 0.001);
                tf.translation = p.origin + d * l * 0.5;
            }
            PartKind::Ring { size } => {
                let s = size * (0.4 + 4.2 * k.powf(0.5));
                tf.scale = Vec3::splat(s);
                if let Some(mut m) = mat.and_then(|m| mats.get_mut(&m.0)) {
                    let a = (1.0 - k).powf(2.2) * 0.8;
                    m.base_color.set_alpha(a);
                }
            }
            PartKind::Smoke { size } => {
                let s = size * (0.4 + 1.3 * k.sqrt()) * (1.0 - k.powi(4));
                tf.scale = Vec3::splat(s.max(0.001));
                if atmo {
                    tf.translation.y += 9.0 * dt;
                }
                if let Some(mut v) = vis {
                    *v = if tf.translation.distance(cam) < s * 1.2 + 0.5 { Visibility::Hidden } else { Visibility::Inherited };
                }
            }
            PartKind::Ember { mut vel } => {
                if atmo {
                    vel.y -= 14.0 * dt;
                }
                vel *= 1.0 - 0.8 * dt;
                tf.translation += vel * dt;
                tf.scale *= 1.0 - 0.9 * dt;
                p.kind = PartKind::Ember { vel };
            }
            PartKind::Light { power } => {
                if let Some(mut l) = light {
                    l.intensity = power * (1.0 - k).powi(2);
                }
            }
            PartKind::Delayed { .. } => {}
        }
        if p.t >= p.life {
            if let PartKind::Delayed { size, vel } = p.kind {
                commands.spawn(Boom { pos: tf.translation, size, vel });
            }
            commands.entity(e).try_despawn();
        }
    }
}

// ---------------------------------------------------------------- estelas

/// Marca un misil recién creado para que `attach_trails` le añada estela.
#[derive(Component)]
pub struct NeedsTrail {
    pub width: f32,
    pub life: f32,
}

#[derive(Component)]
pub struct Trail {
    pub owner: Entity,
    pub pts: VecDeque<(Vec3, f32)>,
    pub life: f32,
    pub w0: f32,
    pub alive: bool,
    pub mesh: Handle<Mesh>,
}

pub fn attach_trails(
    mut commands: Commands,
    q: Query<(Entity, &NeedsTrail, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    fx: Res<FxAssets>,
) {
    for (e, nt, tf) in &q {
        commands.entity(e).remove::<NeedsTrail>();
        // malla inicial con un triángulo degenerado (una malla vacía genera avisos)
        let p0 = tf.translation.to_array();
        let mesh = meshes.add(
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD)
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![p0; 3])
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 3])
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0, 1.0, 1.0, 0.0]; 3])
                .with_inserted_indices(Indices::U32(vec![0, 1, 2])),
        );
        let mut pts = VecDeque::new();
        pts.push_front((tf.translation, 0.0));
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(fx.m_trail.clone()),
            Transform::default(),
            Visibility::Hidden,
            NoFrustumCulling,
            NotShadowCaster,
            ZoneEntity,
            Trail { owner: e, pts, life: nt.life, w0: nt.width, alive: true, mesh },
        ));
    }
}

pub fn trails(
    time: Res<Time>,
    mut commands: Commands,
    dir: Res<Director>,
    owners: Query<&Transform, With<Missile>>,
    mut q: Query<(Entity, &mut Trail, &mut Visibility)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs();
    let cam = dir.cam.translation;
    if std::env::var("NAVE_DEBUGFX").is_ok() && !q.is_empty() {
        let pts: usize = q.iter().map(|t| t.1.pts.len()).sum();
        let first = q.iter().next().map(|t| (t.1.pts.front().map(|p| p.0), t.1.alive));
        info!("estelas {} puntos {} misiles {} cam {:?} primera {:?}", q.iter().count(), pts, owners.iter().count(), cam, first);
    }
    for (e, mut tr, mut vis) in &mut q {
        for p in tr.pts.iter_mut() {
            p.1 += dt;
        }
        if tr.alive {
            match owners.get(tr.owner) {
                Ok(otf) => {
                    let head = otf.translation;
                    let far = tr.pts.front().is_none_or(|p| p.0.distance(head) > 1.2);
                    if far {
                        tr.pts.push_front((head, 0.0));
                    } else if let Some(f) = tr.pts.front_mut() {
                        f.0 = head;
                    }
                }
                Err(_) => tr.alive = false,
            }
        }
        let life = tr.life;
        while tr.pts.back().is_some_and(|p| p.1 > life) {
            tr.pts.pop_back();
        }
        if tr.pts.len() > 400 {
            tr.pts.truncate(400);
        }
        if !tr.alive && tr.pts.len() < 2 {
            commands.entity(e).try_despawn();
            continue;
        }
        if tr.pts.len() < 2 {
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Inherited;
        let Some(mut mesh) = meshes.get_mut(&tr.mesh) else { continue };
        let n = tr.pts.len();
        let mut pos: Vec<[f32; 3]> = Vec::with_capacity(n * 6);
        let mut col: Vec<[f32; 4]> = Vec::with_capacity(n * 6);
        let mut nor: Vec<[f32; 3]> = Vec::with_capacity(n * 6);
        let ink = [0.10, 0.07, 0.16];
        for i in 0..n {
            let (p, age) = tr.pts[i];
            let a = tr.pts[i.saturating_sub(1)].0;
            let b = tr.pts[(i + 1).min(n - 1)].0;
            let tan = (a - b).normalize_or(Vec3::Z);
            let to_cam = (cam - p).normalize_or(Vec3::Y);
            let side = tan.cross(to_cam).normalize_or(Vec3::X);
            let k = age / life;
            let head = (0.25 + age / 0.07).min(1.0);
            let w = tr.w0 * (0.35 + 2.4 * k.powf(0.6)) * head;
            let fade = 1.0 - smooth(0.5, 1.0, k);
            let hot = (1.0 - age / 0.12).max(0.0);
            let core = [1.0, 0.97 - hot * 0.1, 0.93 - hot * 0.45, fade];
            let edge = [ink[0], ink[1], ink[2], fade * 0.85];
            let inner = w * 0.78;
            let off = [-w, -inner, -inner, inner, inner, w];
            let cs = [edge, edge, core, core, edge, edge];
            for j in 0..6 {
                pos.push((p + side * off[j]).to_array());
                col.push(cs[j]);
                nor.push(to_cam.to_array());
            }
        }
        let mut idx: Vec<u32> = Vec::with_capacity((n - 1) * 18);
        for i in 0..(n - 1) as u32 {
            let r0 = i * 6;
            let r1 = r0 + 6;
            for (c0, c1) in [(0u32, 1u32), (2, 3), (4, 5)] {
                idx.extend_from_slice(&[r0 + c0, r1 + c0, r0 + c1, r0 + c1, r1 + c0, r1 + c1]);
            }
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
        mesh.insert_indices(Indices::U32(idx));
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------- misiles en racimo

pub struct Pending {
    pub t: f32,
    pub launcher: Entity,
    pub target: Option<Entity>,
    pub i: u32,
    pub n: u32,
}

#[derive(Resource, Default)]
pub struct ClusterQueue(pub Vec<Pending>);

impl ClusterQueue {
    /// Encola una salva de `n` micromisiles repartidos entre `targets`.
    pub fn salvo(&mut self, launcher: Entity, targets: &[Entity], n: u32, spacing: f32) {
        for i in 0..n {
            let target = if targets.is_empty() { None } else { Some(targets[i as usize % targets.len()]) };
            self.0.push(Pending { t: i as f32 * spacing, launcher, target, i, n });
        }
    }
}

pub fn cluster_launch(
    time: Res<Time>,
    mut queue: ResMut<ClusterQueue>,
    mut commands: Commands,
    a: Res<GameAssets>,
    fx: Res<FxAssets>,
    launchers: Query<(&Transform, Option<&Player>, Option<&Ally>)>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    let mut keep = Vec::with_capacity(queue.0.len());
    for mut p in queue.0.drain(..) {
        p.t -= dt;
        if p.t > 0.0 {
            keep.push(p);
            continue;
        }
        let Ok((tf, pl, al)) = launchers.get(p.launcher) else { continue };
        let fwd = tf.forward().as_vec3();
        let base = match (pl, al) {
            (Some(pl), _) => fwd * pl.speed,
            (_, Some(al)) => al.vel,
            _ => Vec3::ZERO,
        };
        let side = if p.i % 2 == 0 { 1.0 } else { -1.0 };
        let local = Vec3::new(side * rng.gen_range(1.8..3.6), rng.gen_range(-1.6..0.2), rng.gen_range(-1.0..2.0));
        let pos = tf.transform_point(local);
        // eyección en abanico: hacia los lados, arriba y abajo, con algo de avance
        let ang = p.i as f32 / p.n.max(1) as f32 * TAU + rng.gen_range(-0.35..0.35);
        let radial = tf.right().as_vec3() * ang.cos() + tf.up().as_vec3() * ang.sin() * 0.8;
        let d0 = (radial + fwd * rng.gen_range(0.25..0.9)).normalize_or(fwd);
        let vel = base * 0.85 + d0 * rng.gen_range(170.0..270.0);
        let rot = Quat::from_rotation_arc(Vec3::NEG_Z, vel.normalize_or(fwd));
        commands
            .spawn((
                Mesh3d(a.toon.cube.clone()),
                MeshMaterial3d(a.m_missile.clone()),
                Transform::from_translation(pos).with_rotation(rot).with_scale(Vec3::new(0.32, 0.32, 1.9)),
                Missile { target: p.target, vel, life: 4.2, smoke: 0.0, cluster: true, age: 0.0, seed: rng.gen_range(0.0..100.0), damage: 40.0, aim: fwd },
                NeedsTrail { width: 1.05, life: 2.4 },
                NotShadowCaster,
                ZoneEntity,
            ))
            .with_children(|c| {
                c.spawn((Mesh3d(a.plain_sphere.clone()), MeshMaterial3d(fx.m_flare.clone()), Transform::from_xyz(0.0, 0.0, 0.55).with_scale(Vec3::new(2.6, 2.6, 0.32)), NotShadowCaster));
            });
        if p.i % 3 == 0 {
            crate::gameplay::spawn_puff(&mut commands, &a, pos, 2.2, 0.9);
        }
        if std::env::var("NAVE_DEBUGFX").is_ok() {
            info!("racimo {} pos {:?} vel {:?} blanco {:?}", p.i, pos, vel, p.target);
        }
    }
    queue.0 = keep;
}

/// Dirección deseada de un micromisil: fase de eyección, zigzag y guiado terminal.
pub fn steer_cluster(m: &mut Missile, pos: Vec3, target: Option<(Vec3, Vec3)>, dt: f32) -> (Vec3, f32) {
    m.age += dt;
    let mut dir = m.vel.normalize_or(Vec3::NEG_Z);
    let eject = 0.32;
    let speed;
    if m.age < eject {
        // se abren en abanico y frenan un poco antes de encender el motor principal
        speed = (m.vel.length() * (1.0 - 0.6 * dt)).max(150.0);
    } else {
        let a = m.age - eject;
        speed = (m.vel.length() + 900.0 * dt).min(880.0);
        // vector perpendicular que gira: trayectorias en espiral / zigzag
        let up = if dir.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let u = dir.cross(up).normalize();
        let v = dir.cross(u).normalize();
        let wob = (u * (a * 6.5 + m.seed).sin() + v * (a * 4.9 + m.seed * 1.7).cos()) * 0.75 * (1.0 - a / 1.8).max(0.0);
        let desired = match target {
            Some((tp, tv)) => {
                let to = tp - pos;
                let tt = to.length() / speed.max(1.0);
                (to + tv * tt).normalize_or(dir)
            }
            None => m.aim.try_normalize().unwrap_or(dir),
        };
        let want = (desired + wob).normalize_or(dir);
        let turn = 1.6 + 5.5 * (a / 1.1).min(1.0);
        let ang = dir.angle_between(want);
        if ang > 1e-4 {
            dir = dir.slerp(want, ((turn * dt) / ang).min(1.0)).normalize();
        }
    }
    (dir, speed)
}

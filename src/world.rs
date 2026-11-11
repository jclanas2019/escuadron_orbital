//! Mundo procedural con estética de animación cel de los 90: desierto al
//! atardecer, nubes rosadas, ciudad iluminada, órbita con nebulosa y galaxia.

use bevy::asset::RenderAssetUsages;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use rand::Rng;

use crate::components::*;
use crate::toon::*;

pub const TERRAIN_SIZE: f32 = 18000.0;
pub const TERRAIN_RES: usize = 288;
pub const ATMOS_LIMIT: f32 = 7500.0;
pub const CEILING: f32 = 3400.0;
pub const EARTH_CENTER: Vec3 = Vec3::new(0.0, -9000.0, -6500.0);
pub const EARTH_RADIUS: f32 = 7000.0;
pub const SPACE_LIMIT: f32 = 9000.0;
/// Sol del atardecer (bajo, delante del punto de partida).
pub const SUN_DIR: Vec3 = Vec3::new(-0.30, 0.10, -0.95);
/// Sol en órbita.
pub const SPACE_SUN: Vec3 = Vec3::new(-0.45, 0.55, -0.70);
pub const SKY_RADIUS: f32 = 70000.0;

// ---------------------------------------------------------------- ruido

fn hash2(x: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393) ^ (z as u32).wrapping_mul(668265263);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

fn value_noise(x: f32, z: f32) -> f32 {
    let xi = x.floor() as i32;
    let zi = z.floor() as i32;
    let fx = x - x.floor();
    let fz = z - z.floor();
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sz = fz * fz * (3.0 - 2.0 * fz);
    let a = hash2(xi, zi);
    let b = hash2(xi + 1, zi);
    let c = hash2(xi, zi + 1);
    let d = hash2(xi + 1, zi + 1);
    let ab = a + (b - a) * sx;
    let cd = c + (d - c) * sx;
    ab + (cd - ab) * sz
}

fn fbm(x: f32, z: f32, oct: u32) -> f32 {
    let mut s = 0.0;
    let mut amp = 0.5;
    let mut f = 1.0;
    for _ in 0..oct {
        s += value_noise(x * f, z * f) * amp;
        f *= 2.03;
        amp *= 0.5;
    }
    s
}



/// Altura del terreno.
pub fn terrain_height(x: f32, z: f32) -> f32 {
    let base = fbm(x * 0.00028 + 11.0, z * 0.00028 + 3.0, 5);
    let ridge = 1.0 - (fbm(x * 0.0006 + 40.0, z * 0.0006 - 7.0, 4) * 2.0 - 1.0).abs();
    let mountains = ridge.powf(3.0) * 900.0 * ((base - 0.42) * 3.0).clamp(0.0, 1.0);
    // mesetas escalonadas, típicas de desierto
    let mesa = (fbm(x * 0.0012 - 5.0, z * 0.0012 + 9.0, 3) - 0.55).max(0.0) * 900.0;
    let mesa = (mesa / 60.0).floor() * 60.0;
    let detail = fbm(x * 0.004, z * 0.004, 3) * 25.0;
    let valley = (-((x * x + z * z) / (2.0 * 2500.0 * 2500.0))).exp() * 120.0;
    (base - 0.40) * 650.0 + mountains + mesa + detail - valley
}

pub fn ground_height(x: f32, z: f32) -> f32 {
    terrain_height(x, z).max(0.0)
}

fn terrain_palette(h: f32, x: f32, z: f32) -> Vec3 {
    let v = value_noise(x * 0.004, z * 0.004) * 0.08;
    let c = if h < 4.0 {
        Vec3::new(0.86, 0.62, 0.48)
    } else if h > 650.0 {
        Vec3::new(1.0, 0.86, 0.82)
    } else if h > 300.0 {
        Vec3::new(0.80, 0.46, 0.42)
    } else if h > 120.0 {
        Vec3::new(0.88, 0.52, 0.40)
    } else {
        Vec3::new(0.94, 0.62, 0.45)
    };
    c * (0.95 + v)
}

/// Bandas de luz del atardecer: luz cálida, medio tono y sombra violeta.
fn sunset_band(ndl: f32) -> Vec3 {
    if ndl > 0.40 {
        Vec3::new(1.05, 0.95, 0.85)
    } else if ndl > 0.10 {
        Vec3::new(0.80, 0.66, 0.70)
    } else {
        Vec3::new(0.46, 0.34, 0.58)
    }
}

pub fn build_terrain_mesh() -> Mesh {
    let n = TERRAIN_RES + 1;
    let step = TERRAIN_SIZE / TERRAIN_RES as f32;
    let half = TERRAIN_SIZE * 0.5;
    let mut pos = Vec::with_capacity(n * n);
    for iz in 0..n {
        for ix in 0..n {
            let x = ix as f32 * step - half;
            let z = iz as f32 * step - half;
            pos.push([x, terrain_height(x, z), z]);
        }
    }
    let mut idx: Vec<u32> = Vec::with_capacity(TERRAIN_RES * TERRAIN_RES * 6);
    for iz in 0..TERRAIN_RES {
        for ix in 0..TERRAIN_RES {
            let a = (iz * n + ix) as u32;
            let b = a + 1;
            let c = a + n as u32;
            let d = c + 1;
            idx.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos.clone());
    m.insert_indices(Indices::U32(idx));
    m.compute_smooth_normals();
    let l = SUN_DIR.normalize();
    let cols: Vec<[f32; 4]> = match m.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(nr)) => nr
            .iter()
            .zip(pos.iter())
            .map(|(nn, p)| {
                let ndl = Vec3::from(*nn).dot(l) * 2.2; // sol bajo: realzar el contraste
                let c = terrain_palette(p[1], p[0], p[2]) * sunset_band(ndl);
                [c.x, c.y, c.z, 1.0]
            })
            .collect(),
        _ => vec![],
    };
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, cols);
    m
}

/// Cúpula de cielo con degradado pintado (atardecer) o nebulosa (espacio).
/// Color del cielo en una dirección (atardecer o nebulosa espacial).
pub fn sky_color(d: Vec3, space: bool) -> Vec3 {
    let sun = if space { SPACE_SUN.normalize() } else { SUN_DIR.normalize() };
    if space {
        let band_n = Vec3::new(0.3, 0.8, 0.5).normalize();
        let b = (1.0 - d.dot(band_n).abs() * 3.0).clamp(0.0, 1.0);
        let n1 = fbm(d.x * 3.0 + 4.0, d.z * 3.0 + d.y * 2.0, 5);
        let n2 = fbm(d.y * 4.0 - 2.0, d.x * 4.0 + d.z, 4);
        let violet = Vec3::new(0.30, 0.08, 0.42) * (n1 * b * 1.6).powf(1.5);
        let blue = Vec3::new(0.04, 0.16, 0.42) * (n2 * b * 1.8).powf(1.7);
        let s = d.dot(sun).max(0.0);
        Vec3::new(0.008, 0.008, 0.03) + violet + blue + Vec3::new(1.0, 0.95, 0.9) * s.powf(400.0) * 6.0
    } else {
        let y = d.y;
        let stops = [
            (-1.0, Vec3::new(0.30, 0.16, 0.30)),
            (-0.02, Vec3::new(0.62, 0.32, 0.42)),
            (0.0, Vec3::new(1.00, 0.60, 0.42)),
            (0.06, Vec3::new(0.98, 0.46, 0.50)),
            (0.22, Vec3::new(0.70, 0.30, 0.60)),
            (0.50, Vec3::new(0.40, 0.22, 0.60)),
            (1.0, Vec3::new(0.16, 0.13, 0.42)),
        ];
        let mut c = stops[6].1;
        for w in stops.windows(2) {
            if y >= w[0].0 && y <= w[1].0 {
                let k = (y - w[0].0) / (w[1].0 - w[0].0);
                c = w[0].1.lerp(w[1].1, k);
            }
        }
        let s = d.dot(sun).max(0.0);
        c + Vec3::new(1.0, 0.65, 0.35) * s.powf(12.0) * 0.8 + Vec3::new(0.5, 0.2, 0.1) * s.powf(3.0) * 0.35
    }
}

/// Cúpula de cielo con degradado pintado (atardecer) o nebulosa (espacio).
pub fn build_sky_mesh(space: bool) -> Mesh {
    let mut m = Sphere::new(1.0).mesh().uv(96, 48);
    let positions: Vec<Vec3> = match m.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(v)) => v.iter().map(|p| Vec3::from(*p)).collect(),
        _ => vec![],
    };
    let cols: Vec<[f32; 4]> = positions
        .iter()
        .map(|p| {
            let mut c = sky_color(p.normalize(), space);
            if space {
                // el disco del sol ya se dibuja aparte
                c = c.min(Vec3::splat(1.0));
            }
            [c.x, c.y, c.z, 1.0]
        })
        .collect();
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, cols);
    m
}

/// Galaxia espiral lejana hecha de puntos brillantes.
pub fn build_galaxy_mesh() -> Mesh {
    let mut rng = rand::thread_rng();
    let mut pos = Vec::new();
    let mut col = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for i in 0..2600 {
        let arm = (i % 2) as f32 * std::f32::consts::PI;
        let t: f32 = rng.gen_range(0.0..1.0f32).powf(0.7);
        let ang = arm + t * 9.0 + rng.gen_range(-0.35..0.35);
        let r = t * 1.0 + 0.02;
        let jitter = rng.gen_range(-0.06..0.06) * (1.0 - t * 0.5);
        let c = Vec3::new(ang.cos() * r + jitter, rng.gen_range(-0.02..0.02), ang.sin() * r + jitter) * 9000.0;
        let s = rng.gen_range(25.0..70.0) * (1.4 - t);
        let core = (1.0 - t).powf(3.0);
        let tint = Vec3::new(0.45, 0.65, 1.0).lerp(Vec3::new(1.0, 0.9, 0.95), core) * (1.2 + core * 3.0);
        let base = pos.len() as u32;
        for o in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z] {
            pos.push((c + o * s).to_array());
            col.push([tint.x, tint.y, tint.z, 1.0]);
        }
        for f in [[0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4], [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5]] {
            idx.extend(f.iter().map(|k| base + *k as u32));
        }
    }
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    m.insert_indices(Indices::U32(idx));
    m.compute_normals();
    m
}

/// Tierra vista desde la órbita, con sombreado cel según la posición del sol.
pub fn build_earth_mesh(radius: f32, rot: Quat) -> Mesh {
    let mut m = Sphere::new(radius).mesh().uv(160, 90);
    let positions: Vec<Vec3> = match m.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(v)) => v.iter().map(|p| Vec3::from(*p)).collect(),
        _ => vec![],
    };
    let l_obj = rot.inverse() * SPACE_SUN.normalize();
    let cols: Vec<[f32; 4]> = positions
        .iter()
        .map(|p| {
            let d = p.normalize();
            let land = fbm(d.x * 2.2 + d.y * 1.3 + 5.0, d.z * 2.2 - d.y * 1.1 + 2.0, 5)
                + fbm(d.y * 3.0 + 9.0, d.x * 3.0 - d.z, 3) * 0.3;
            let lat = d.y.abs();
            let c = if lat > 0.92 {
                Vec3::new(0.92, 0.95, 1.0)
            } else if land > 0.62 {
                if lat < 0.35 && fbm(d.x * 6.0, d.z * 6.0, 2) > 0.55 {
                    Vec3::new(0.85, 0.65, 0.40)
                } else {
                    Vec3::new(0.25, 0.55, 0.30)
                }
            } else if land > 0.58 {
                Vec3::new(0.15, 0.50, 0.70)
            } else {
                Vec3::new(0.08, 0.22, 0.62)
            };
            let ndl = d.dot(l_obj);
            let k = if ndl > 0.25 {
                Vec3::ONE
            } else if ndl > -0.05 {
                Vec3::new(0.55, 0.5, 0.8)
            } else {
                Vec3::new(0.10, 0.08, 0.22)
            };
            let c = c * k;
            [c.x, c.y, c.z, 1.0]
        })
        .collect();
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, cols);
    m
}

pub fn build_cloud_shell(radius: f32, rot: Quat) -> Mesh {
    let mut m = Sphere::new(radius).mesh().uv(160, 90);
    let positions: Vec<Vec3> = match m.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(v)) => v.iter().map(|p| Vec3::from(*p)).collect(),
        _ => vec![],
    };
    let l_obj = rot.inverse() * SPACE_SUN.normalize();
    let cols: Vec<[f32; 4]> = positions
        .iter()
        .map(|p| {
            let d = p.normalize();
            let c = fbm(d.x * 5.0 + 30.0, d.z * 5.0 + d.y * 4.0, 5);
            let a = if c > 0.56 { 0.85 } else { 0.0 };
            let k = if d.dot(l_obj) > 0.0 { 1.0 } else { 0.25 };
            [k, k, k * 1.05, a]
        })
        .collect();
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, cols);
    m
}

/// Campo de estrellas como una sola malla (octaedros diminutos).
pub fn build_star_mesh(count: usize, radius: f32) -> Mesh {
    let mut rng = rand::thread_rng();
    let mut pos = Vec::new();
    let mut col = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for _ in 0..count {
        let d = Vec3::new(
            rng.gen_range(-1.0..1.0),
            rng.gen_range(-1.0..1.0),
            rng.gen_range(-1.0..1.0),
        )
        .normalize_or(Vec3::Y);
        let c = d * radius;
        let s = rng.gen_range(18.0..55.0f32) * if rng.gen_bool(0.05) { 2.0 } else { 1.0 };
        let b = rng.gen_range(0.6..4.0f32);
        let tint = match rng.gen_range(0..4) {
            0 => [1.0, 0.85, 0.7],
            1 => [0.75, 0.85, 1.0],
            _ => [1.0, 1.0, 1.0],
        };
        let base = pos.len() as u32;
        for o in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z] {
            pos.push((c + o * s).to_array());
            col.push([tint[0] * b, tint[1] * b, tint[2] * b, 1.0]);
        }
        let faces = [
            [0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4],
            [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5],
        ];
        for f in faces {
            idx.extend(f.iter().map(|i| base + *i as u32));
        }
    }
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    m.insert_indices(Indices::U32(idx));
    m.compute_normals();
    m
}

/// Roca irregular con sombreado plano.
pub fn build_rock_mesh(seed: u32) -> Mesh {
    let mut m = Sphere::new(1.0).mesh().ico(2).unwrap();
    if let Some(VertexAttributeValues::Float32x3(v)) = m.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
        for p in v.iter_mut() {
            let d = Vec3::from(*p).normalize();
            let k = 0.7
                + value_noise(d.x * 2.5 + seed as f32 * 3.1, d.z * 2.5 + d.y * 1.7) * 0.5
                + value_noise(d.y * 5.0 + seed as f32, d.x * 5.0) * 0.15;
            *p = (d * k).to_array();
        }
    }
    m.duplicate_vertices();
    m.compute_flat_normals();
    bake_toon(m, KEY_LIGHT)
}



// ---------------------------------------------------------------- assets

#[derive(Resource)]
pub struct GameAssets {
    pub toon: ToonMeshes,
    /// Modelos glTF de los Invid (se rellenan en `models::load_models`).
    pub invid: Handle<bevy::world_serialization::WorldAsset>,
    pub invid_parts: Handle<bevy::world_serialization::WorldAsset>,
    pub valk_flight: Handle<bevy::world_serialization::WorldAsset>,
    pub valk_blue: Handle<bevy::world_serialization::WorldAsset>,
    pub gerwalk: Handle<bevy::world_serialization::WorldAsset>,
    pub battroid: Handle<bevy::world_serialization::WorldAsset>,
    pub boss: Handle<bevy::world_serialization::WorldAsset>,
    pub hive: Handle<bevy::world_serialization::WorldAsset>,
    pub spartan: Handle<bevy::world_serialization::WorldAsset>,
    pub shock: Handle<bevy::world_serialization::WorldAsset>,
    pub carrier: Handle<bevy::world_serialization::WorldAsset>,
    pub mothership: Handle<bevy::world_serialization::WorldAsset>,
    pub ring: Handle<Mesh>,
    pub m_ring: Handle<StandardMaterial>,
    pub m_ring_off: Handle<StandardMaterial>,
    pub plain_sphere: Handle<Mesh>,
    pub plain_cube: Handle<Mesh>,
    pub laser: Handle<Mesh>,
    pub terrain: Handle<Mesh>,
    pub water: Handle<Mesh>,
    pub sky_dusk: Handle<Mesh>,
    pub sky_space: Handle<Mesh>,
    pub galaxy: Handle<Mesh>,
    pub earth: Handle<Mesh>,
    pub earth_clouds: Handle<Mesh>,
    pub halo: Handle<Mesh>,
    pub stars: Handle<Mesh>,
    pub rocks: Vec<Handle<Mesh>>,

    pub m_eye: Handle<StandardMaterial>,
    pub m_invid_far: Handle<StandardMaterial>,
    pub m_thruster: Handle<StandardMaterial>,
    pub m_engine: Handle<StandardMaterial>,
    pub m_laser_player: Handle<StandardMaterial>,
    pub m_laser_enemy: Handle<StandardMaterial>,
    pub m_flak: Handle<StandardMaterial>,
    pub m_missile: Handle<StandardMaterial>,
    pub m_fire: Handle<StandardMaterial>,
    pub m_smoke: Handle<StandardMaterial>,
    pub m_debris: Handle<StandardMaterial>,
    pub m_terrain: Handle<StandardMaterial>,
    pub m_water: Handle<StandardMaterial>,
    pub m_cloud: Handle<StandardMaterial>,
    pub m_sky: Handle<StandardMaterial>,
    pub m_earth: Handle<StandardMaterial>,
    pub m_earth_clouds: Handle<StandardMaterial>,
    pub m_halo: Handle<StandardMaterial>,
    pub m_stars: Handle<StandardMaterial>,
    pub m_moon: Handle<StandardMaterial>,
    pub m_rock: Handle<StandardMaterial>,
    pub m_building: Handle<StandardMaterial>,
    pub m_building2: Handle<StandardMaterial>,
    pub m_window: Handle<StandardMaterial>,
    pub m_red_light: Handle<StandardMaterial>,
    pub m_turret: Handle<StandardMaterial>,
    pub m_mother_lights: Handle<StandardMaterial>,
    pub m_sun: Handle<StandardMaterial>,
    pub m_dust: Handle<StandardMaterial>,
}

pub fn setup_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let m = &mut *mats;
    let earth_rot = Quat::from_rotation_x(1.25) * Quat::from_rotation_y(0.7);
    let toon = ToonMeshes::new(&mut meshes, m);
    let sky = m.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        fog_enabled: false,
        cull_mode: None,
        ..default()
    });
    let a = GameAssets {
        toon,
        invid: Handle::default(),
        invid_parts: Handle::default(),
        valk_flight: Handle::default(),
        valk_blue: Handle::default(),
        gerwalk: Handle::default(),
        battroid: Handle::default(),
        boss: Handle::default(),
        hive: Handle::default(),
        spartan: Handle::default(),
        shock: Handle::default(),
        carrier: Handle::default(),
        mothership: Handle::default(),
        ring: meshes.add(Torus::new(24.0, 28.0).mesh().major_resolution(48).minor_resolution(10)),
        m_ring: glow(m, 0.6, 6.0, 3.0),
        m_ring_off: glow(m, 1.5, 0.5, 0.3),
        plain_sphere: meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()),
        plain_cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        laser: meshes.add(Cuboid::new(0.45, 0.45, 22.0)),
        terrain: meshes.add(build_terrain_mesh()),
        water: meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(TERRAIN_SIZE * 2.0))),
        sky_dusk: meshes.add(build_sky_mesh(false)),
        sky_space: meshes.add(build_sky_mesh(true)),
        galaxy: meshes.add(build_galaxy_mesh()),
        earth: meshes.add(build_earth_mesh(EARTH_RADIUS, earth_rot)),
        earth_clouds: meshes.add(build_cloud_shell(EARTH_RADIUS * 1.012, earth_rot)),
        halo: meshes.add(Sphere::new(EARTH_RADIUS * 1.05).mesh().uv(96, 48)),
        stars: meshes.add(build_star_mesh(2600, 40000.0)),
        rocks: (0..5).map(|i| meshes.add(build_rock_mesh(i + 1))).collect(),

        m_eye: glow(m, 12.0, 1.2, 6.0),
        m_invid_far: flat(m, Color::srgb(0.30, 0.20, 0.45)),
        m_thruster: glow(m, 4.0, 3.2, 8.0),
        m_engine: glow(m, 2.5, 6.0, 12.0),
        m_laser_player: glow(m, 1.5, 10.0, 3.5),
        m_laser_enemy: glow(m, 14.0, 0.8, 0.6),
        m_flak: glow(m, 12.0, 5.0, 0.8),
        m_missile: flat(m, Color::srgb(0.95, 0.95, 0.95)),
        m_fire: glow(m, 9.0, 3.2, 0.6),
        m_smoke: flat(m, Color::srgb(0.95, 0.90, 0.92)),
        m_debris: flat(m, Color::srgb(0.12, 0.10, 0.14)),
        m_terrain: flat(m, Color::WHITE),
        m_water: flat(m, Color::srgb(0.36, 0.24, 0.50)),
        m_cloud: flat(m, Color::srgb(1.0, 0.80, 0.84)),
        m_sky: sky,
        m_earth: m.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            ..default()
        }),
        m_earth_clouds: m.add(StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            fog_enabled: false,
            ..default()
        }),
        m_halo: m.add(StandardMaterial {
            base_color: Color::srgba(0.3, 0.6, 1.0, 0.3),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            fog_enabled: false,
            ..default()
        }),
        m_stars: m.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            ..default()
        }),
        m_moon: flat(m, Color::srgb(0.80, 0.76, 0.86)),
        m_rock: flat(m, Color::srgb(0.52, 0.38, 0.32)),
        m_building: flat(m, Color::srgb(0.34, 0.26, 0.40)),
        m_building2: flat(m, Color::srgb(0.20, 0.16, 0.28)),
        m_window: glow(m, 3.5, 2.2, 0.8),
        m_red_light: glow(m, 9.0, 0.5, 0.3),
        m_turret: flat(m, Color::srgb(0.42, 0.42, 0.36)),
        m_mother_lights: glow(m, 10.0, 1.8, 1.2),
        m_sun: glow(m, 14.0, 9.0, 5.0),
        m_dust: flat(m, Color::srgb(0.8, 0.8, 0.9)),
    };
    commands.insert_resource(a);
}

// ---------------------------------------------------------------- naves

/// Explorador Invid (modelo glTF del usuario) con ojo sensor y propulsores brillantes.
pub fn spawn_fighter(commands: &mut Commands, a: &GameAssets, pos: Vec3, look: Vec3, wave: u32) -> Entity {
    let mut rng = rand::thread_rng();
    let hp = 40.0 + wave as f32 * 4.0;
    let tf = Transform::from_translation(pos).looking_at(look, Vec3::Y);
    let mt = crate::models::invid_transform();
    commands
        .spawn((
            tf,
            Visibility::default(),
            Enemy {
                kind: EnemyKind::Fighter,
                hp,
                max_hp: hp,
                radius: 8.0,
                half_extents: None,
                cd: rng.gen_range(1.0..3.0),
                speed: rng.gen_range(170.0..230.0) + wave as f32 * 6.0,
                vel: Vec3::ZERO,
                evade: 0.0,
                evade_dir: Vec3::ZERO,
                score: 100,
                hit_flash: 0.0,
            },
        ))
        .with_children(|p| {
            p.spawn((bevy::world_serialization::WorldAssetRoot(a.invid.clone()), mt));
            // LOD lejano: silueta simplificada (el modelo detallado se oculta a más de 1.6 km)
            let lod = bevy::camera::visibility::VisibilityRange {
                start_margin: 1500.0..1700.0,
                end_margin: 1.0e6..1.0e6 + 1.0,
                use_aabb: false,
            };
            p.spawn((Mesh3d(a.toon.sphere.clone()), MeshMaterial3d(a.m_invid_far.clone()), Transform::from_xyz(0.0, 2.0, 0.5).with_scale(Vec3::new(3.6, 3.0, 4.0)), lod.clone()));
            for x in [-3.0f32, 3.0] {
                p.spawn((Mesh3d(a.toon.capsule.clone()), MeshMaterial3d(a.m_invid_far.clone()), Transform::from_xyz(x, -1.5, 1.0).with_scale(Vec3::new(1.6, 4.0, 1.6)), lod.clone()));
            }
            // ojo sensor rosado en el frente de la cabeza
            p.spawn((
                Mesh3d(a.plain_sphere.clone()),
                MeshMaterial3d(a.m_eye.clone()),
                Transform::from_translation(mt.transform_point(Vec3::new(0.0, 0.62, 0.30))).with_scale(Vec3::splat(0.4)),
            ));
            // propulsores en la espalda
            for x in [-0.30f32, 0.30] {
                p.spawn((
                    Mesh3d(a.plain_sphere.clone()),
                    MeshMaterial3d(a.m_thruster.clone()),
                    Transform::from_translation(mt.transform_point(Vec3::new(x, 0.45, -0.62))).with_scale(Vec3::new(0.35, 0.35, 0.6)),
                ));
            }
        })
        .id()
}

/// Caza del jugador para las vistas externas: el modelo Valkyrie del usuario
/// (materiales PBR con reflejos) más el brillo de las toberas.
pub fn spawn_player_model(p: &mut ChildSpawnerCommands, a: &GameAssets, valk: &Handle<bevy::world_serialization::WorldAsset>) {
    let vt = crate::models::valk_flight_transform();
    p.spawn((Transform::default(), Visibility::Hidden, PlayerModel))
        .with_children(|p| {
            p.spawn((Transform::default(), Visibility::Inherited, crate::mecha::FormModel(crate::mecha::Mode::Fighter))).with_children(|p| {
                p.spawn((bevy::world_serialization::WorldAssetRoot(valk.clone()), vt));
                for z in [-0.2f32, 0.2] {
                    let pos = vt.transform_point(Vec3::new(0.97, -0.05, z));
                    p.spawn((Mesh3d(a.plain_sphere.clone()), MeshMaterial3d(a.m_engine.clone()), Transform::from_translation(pos).with_scale(Vec3::new(0.55, 0.55, 0.3)), EngineGlow));
                }
            });
            crate::mecha::spawn_form_models(p, &a.gerwalk, &a.battroid);
        });
}

fn spawn_turret(commands: &mut Commands, a: &GameAssets, pos: Vec3) {
    let t = a.toon.clone();
    commands.spawn((Transform::from_translation(pos + Vec3::Y * 3.0), Visibility::default(), ZoneEntity)).with_children(|p| {
        t.part(p, Shape::Cyl, &a.m_turret, Transform::from_scale(Vec3::new(6.0, 6.0, 6.0)), 0.4);
    });
    commands
        .spawn((
            Transform::from_translation(pos + Vec3::Y * 8.0),
            Visibility::default(),
            Enemy {
                kind: EnemyKind::Turret,
                hp: 70.0,
                max_hp: 70.0,
                radius: 7.0,
                half_extents: None,
                cd: rand::thread_rng().gen_range(1.0..4.0),
                speed: 0.0,
                vel: Vec3::ZERO,
                evade: 0.0,
                evade_dir: Vec3::ZERO,
                score: 150,
                hit_flash: 0.0,
            },
        ))
        .with_children(|p| {
            t.part(p, Shape::Sphere, &a.m_turret, Transform::from_scale(Vec3::splat(3.5)), 0.3);
            for s in [-1.0f32, 1.0] {
                t.part(p, Shape::Cube, &a.m_building2, Transform::from_xyz(s * 1.2, 0.5, -5.0).with_scale(Vec3::new(0.7, 0.7, 8.0)), 0.2);
            }
            p.spawn((Mesh3d(a.plain_sphere.clone()), MeshMaterial3d(a.m_red_light.clone()), Transform::from_xyz(0.0, 3.5, 0.0).with_scale(Vec3::splat(0.5))));
        });
}

// ---------------------------------------------------------------- zonas

pub fn spawn_atmosphere(commands: &mut Commands, a: &GameAssets, carrier_alive: bool) {
    if carrier_alive {
        spawn_carrier(commands, a, Zone::Atmosphere);
    }
    crate::allies::spawn_squad(commands, a, Zone::Atmosphere);
    let mut rng = rand::thread_rng();
    let t = a.toon.clone();
    commands.spawn((Mesh3d(a.terrain.clone()), MeshMaterial3d(a.m_terrain.clone()), Transform::default(), ZoneEntity, Ground));
    commands.spawn((Mesh3d(a.water.clone()), MeshMaterial3d(a.m_water.clone()), Transform::default(), ZoneEntity));
    commands.spawn((Mesh3d(a.sky_dusk.clone()), MeshMaterial3d(a.m_sky.clone()), Transform::from_scale(Vec3::splat(SKY_RADIUS)), SkyDome, ZoneEntity, NotShadowCaster));
    // Sol del atardecer
    commands.spawn((
        Mesh3d(a.plain_sphere.clone()),
        MeshMaterial3d(a.m_sun.clone()),
        Transform::from_scale(Vec3::splat(2600.0)),
        SunDisc,
        ZoneEntity,
        NotShadowCaster,
    ));
    // Nubes alargadas de atardecer: arriba claras, abajo violetas.
    for _ in 0..110 {
        let c = Vec3::new(rng.gen_range(-9000.0..9000.0), rng.gen_range(1000.0..2400.0), rng.gen_range(-9000.0..9000.0));
        let n = rng.gen_range(3..7);
        let stretch = rng.gen_range(1.8..3.5);
        for _ in 0..n {
            let off = Vec3::new(rng.gen_range(-260.0..260.0), rng.gen_range(-20.0..30.0), rng.gen_range(-90.0..90.0));
            let s = rng.gen_range(60.0..140.0);
            commands.spawn((
                Mesh3d(t.sphere.clone()),
                MeshMaterial3d(a.m_cloud.clone()),
                Transform::from_translation(c + off).with_scale(Vec3::new(s * stretch, s * 0.35, s * 0.9)),
                NotShadowCaster,
                NotShadowReceiver,
                ZoneEntity,
            ));
        }
    }
    // Ciudad iluminada hacia el atardecer
    let city_c = Vec2::new(-1800.0, -6200.0);
    for _ in 0..170 {
        let o = Vec2::new(rng.gen_range(-900.0..900.0), rng.gen_range(-700.0..700.0));
        let fall = 1.0 - (o.length() / 1100.0).min(1.0);
        let pos = city_c + o;
        let gy = terrain_height(pos.x, pos.y).max(0.0);
        let h = rng.gen_range(30.0..80.0) + fall * fall * rng.gen_range(60.0..320.0);
        let w = rng.gen_range(30.0..70.0);
        let d = rng.gen_range(30.0..70.0);
        let mat = if rng.gen_bool(0.5) { a.m_building.clone() } else { a.m_building2.clone() };
        let win = a.m_window.clone();
        let red = a.m_red_light.clone();
        let ps = a.plain_cube.clone();
        let ps2 = a.plain_sphere.clone();
        let tt = t.clone();
        let rows = (h / 14.0) as i32;
        let mut lit: Vec<(i32, f32)> = Vec::new();
        for r in 0..rows {
            if rng.gen_bool(0.45) {
                lit.push((r, rng.gen_range(-0.35..0.35)));
            }
        }
        commands
            .spawn((Transform::from_xyz(pos.x, gy, pos.y), Visibility::default(), ZoneEntity))
            .with_children(|p| {
                tt.part(p, Shape::Cube, &mat, Transform::from_xyz(0.0, h * 0.5, 0.0).with_scale(Vec3::new(w, h, d)), 1.2);
                for (r, x) in &lit {
                    for face in [-1.0f32, 1.0] {
                        p.spawn((
                            Mesh3d(ps.clone()),
                            MeshMaterial3d(win.clone()),
                            Transform::from_xyz(x * w, 8.0 + *r as f32 * 14.0, face * (d * 0.5 + 0.6)).with_scale(Vec3::new(w * 0.35, 3.0, 0.5)),
                        ));
                    }
                }
                if h > 150.0 {
                    p.spawn((Mesh3d(ps2.clone()), MeshMaterial3d(red.clone()), Transform::from_xyz(0.0, h + 3.0, 0.0).with_scale(Vec3::splat(2.5))));
                }
            });
    }
    // Bases enemigas con torretas antiaéreas
    let bases = [
        Vec2::new(-2600.0, -2400.0),
        Vec2::new(2900.0, -3100.0),
        Vec2::new(300.0, -5200.0),
        Vec2::new(-4600.0, 1400.0),
        Vec2::new(4300.0, 2000.0),
        Vec2::new(-900.0, 4600.0),
    ];
    for b in bases {
        let mut c = b;
        for _ in 0..30 {
            let h = terrain_height(c.x, c.y);
            if h > 15.0 && h < 350.0 {
                break;
            }
            c = b + Vec2::new(rng.gen_range(-900.0..900.0), rng.gen_range(-900.0..900.0));
        }
        let gy = ground_height(c.x, c.y);
        let top = gy + 17.0;
        let mut blds: Vec<(Vec3, Vec3, bool)> = Vec::new();
        for _ in 0..7 {
            let o = Vec2::new(rng.gen_range(-100.0..100.0), rng.gen_range(-100.0..100.0));
            let h = rng.gen_range(15.0..70.0);
            let w = rng.gen_range(14.0..34.0);
            blds.push((Vec3::new(o.x, top - gy + h * 0.5, o.y), Vec3::new(w, h, w * rng.gen_range(0.7..1.5)), rng.gen_bool(0.5)));
        }
        let tt = t.clone();
        commands.spawn((Transform::from_xyz(c.x, gy, c.y), Visibility::default(), ZoneEntity)).with_children(|p| {
            tt.part(p, Shape::Cube, &a.m_building2, Transform::from_xyz(0.0, 2.0, 0.0).with_scale(Vec3::new(260.0, 30.0, 260.0)), 1.0);
            for (pp, sc, alt) in &blds {
                tt.part(p, Shape::Cube, if *alt { &a.m_building } else { &a.m_turret }, Transform::from_translation(*pp).with_scale(*sc), 0.6);
                p.spawn((Mesh3d(a.plain_sphere.clone()), MeshMaterial3d(a.m_red_light.clone()), Transform::from_xyz(pp.x, pp.y + sc.y * 0.5 + 1.5, pp.z).with_scale(Vec3::splat(1.4))));
            }
        });
        for tp in [Vec2::new(-110.0, -110.0), Vec2::new(110.0, 110.0), Vec2::new(110.0, -110.0)] {
            spawn_turret(commands, a, Vec3::new(c.x + tp.x, top, c.y + tp.y));
        }
    }
}

pub fn spawn_space(commands: &mut Commands, a: &GameAssets, mothership_alive: bool, carrier_alive: bool) {
    if carrier_alive {
        spawn_carrier(commands, a, Zone::Space);
    }
    crate::allies::spawn_squad(commands, a, Zone::Space);
    let mut rng = rand::thread_rng();
    let earth_rot = Quat::from_rotation_x(1.25) * Quat::from_rotation_y(0.7);
    commands.spawn((Mesh3d(a.sky_space.clone()), MeshMaterial3d(a.m_sky.clone()), Transform::from_scale(Vec3::splat(SKY_RADIUS)), SkyDome, ZoneEntity, NotShadowCaster));
    commands.spawn((Mesh3d(a.galaxy.clone()), MeshMaterial3d(a.m_stars.clone()), Transform::default(), Galaxy, ZoneEntity, NotShadowCaster));
    commands.spawn((Mesh3d(a.earth.clone()), MeshMaterial3d(a.m_earth.clone()), Transform::from_translation(EARTH_CENTER).with_rotation(earth_rot), ZoneEntity, NotShadowCaster));
    commands.spawn((
        Mesh3d(a.earth.clone()),
        MeshMaterial3d(a.toon.ink.clone()),
        Transform::from_translation(EARTH_CENTER).with_rotation(earth_rot).with_scale(Vec3::splat(1.006)),
        ZoneEntity,
        NotShadowCaster,
    ));
    commands.spawn((
        Mesh3d(a.earth_clouds.clone()),
        MeshMaterial3d(a.m_earth_clouds.clone()),
        Transform::from_translation(EARTH_CENTER).with_rotation(earth_rot),
        ZoneEntity,
        NotShadowCaster,
        EarthClouds,
    ));
    commands.spawn((Mesh3d(a.halo.clone()), MeshMaterial3d(a.m_halo.clone()), Transform::from_translation(EARTH_CENTER), ZoneEntity, NotShadowCaster));
    // Sol y luna
    commands.spawn((Mesh3d(a.plain_sphere.clone()), MeshMaterial3d(a.m_sun.clone()), Transform::from_scale(Vec3::splat(1500.0)), SunDisc, ZoneEntity, NotShadowCaster));
    let t = a.toon.clone();
    commands.spawn((Transform::from_xyz(14000.0, 6000.0, -24000.0), Visibility::default(), ZoneEntity)).with_children(|p| {
        t.part(p, Shape::Sphere, &a.m_moon, Transform::from_scale(Vec3::splat(1500.0)), 25.0);
    });
    // Asteroides con sombreado plano y contorno
    for i in 0..170 {
        let mut p;
        loop {
            p = Vec3::new(rng.gen_range(-6500.0..6500.0), rng.gen_range(-1500.0..3500.0), rng.gen_range(-6500.0..6500.0));
            if p.length() > 600.0 && p.distance(EARTH_CENTER) > EARTH_RADIUS + 1200.0 {
                break;
            }
        }
        let r = if rng.gen_bool(0.1) { rng.gen_range(90.0..200.0) } else { rng.gen_range(12.0..60.0) };
        let mesh = a.rocks[i % a.rocks.len()].clone();
        commands
            .spawn((
                Transform::from_translation(p)
                    .with_rotation(Quat::from_euler(EulerRot::XYZ, rng.gen_range(0.0..6.0), rng.gen_range(0.0..6.0), 0.0))
                    .with_scale(Vec3::splat(r)),
                Visibility::default(),
                Rock {
                    radius: r * 0.9,
                    spin: Vec3::new(rng.gen_range(-0.3..0.3), rng.gen_range(-0.3..0.3), rng.gen_range(-0.3..0.3)),
                },
                ZoneEntity,
            ))
            .with_children(|c| {
                c.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(a.m_rock.clone()), NotShadowCaster));
                c.spawn((Mesh3d(mesh), MeshMaterial3d(a.toon.ink.clone()), Transform::from_scale(Vec3::splat(1.06)), NotShadowCaster));
            });
    }
    for _ in 0..260 {
        commands.spawn((
            Mesh3d(a.plain_cube.clone()),
            MeshMaterial3d(a.m_dust.clone()),
            Transform::from_xyz(rng.gen_range(-300.0..300.0), rng.gen_range(-300.0..300.0), rng.gen_range(-300.0..300.0)).with_scale(Vec3::splat(0.35)),
            SpaceDust,
            NotShadowCaster,
            ZoneEntity,
        ));
    }
    if mothership_alive {
        spawn_mothership(commands, a);
    }
}

/// Nave nodriza Invid (modelo del usuario, ~880 m) con torretas sobre el casco.
fn spawn_mothership(commands: &mut Commands, a: &GameAssets) {
    use crate::models::*;
    let pos = Vec3::new(600.0, 900.0, -4200.0);
    let rot = Quat::from_rotation_y(0.6);
    let half = MOTHER_HALF * MOTHER_SCALE;
    commands
        .spawn((
            Transform::from_translation(pos).with_rotation(rot),
            Visibility::default(),
            Enemy {
                kind: EnemyKind::Mothership,
                hp: 3000.0,
                max_hp: 3000.0,
                radius: half.max_element(),
                half_extents: Some(half),
                cd: 6.0,
                speed: 0.0,
                vel: Vec3::ZERO,
                evade: 0.0,
                evade_dir: Vec3::ZERO,
                score: 5000,
                hit_flash: 0.0,
            },
        ))
        .with_children(|p| {
            p.spawn((bevy::world_serialization::WorldAssetRoot(a.mothership.clone()), mothership_model_transform()));
            // luces rojas pulsantes repartidas por el casco
            for t in MOTHER_TURRETS {
                p.spawn((
                    Mesh3d(a.plain_sphere.clone()),
                    MeshMaterial3d(a.m_mother_lights.clone()),
                    Transform::from_translation(Vec3::new(t.x, t.y - 0.03, t.z) * MOTHER_SCALE).with_scale(Vec3::splat(4.0)),
                ));
            }
        });
    for t in MOTHER_TURRETS {
        spawn_turret_space(commands, a, pos + rot * (t * MOTHER_SCALE + Vec3::Y * 6.0));
    }
}

/// Nave nodriza terrestre (modelo del usuario, ~630 m) con su anillo de recarga.
pub fn spawn_carrier(commands: &mut Commands, a: &GameAssets, zone: Zone) {
    use crate::models::*;
    let pos = match zone {
        Zone::Atmosphere => CARRIER_POS_ATMO,
        Zone::Space => CARRIER_POS_SPACE,
    };
    let half = CARRIER_HALF * CARRIER_SCALE;
    commands
        .spawn((
            Transform::from_translation(pos),
            Visibility::default(),
            CarrierShip { half, gun_cd: 1.0 },
            ZoneEntity,
        ))
        .with_children(|p| {
            p.spawn((bevy::world_serialization::WorldAssetRoot(a.carrier.clone()), carrier_model_transform()));
            // luces de navegación
            for (x, z, m) in [(-0.40f32, -0.85f32, &a.m_red_light), (0.40, -0.85, &a.m_ring), (-0.42, 0.6, &a.m_red_light), (0.42, 0.6, &a.m_ring)] {
                p.spawn((
                    Mesh3d(a.plain_sphere.clone()),
                    MeshMaterial3d(m.clone()),
                    Transform::from_translation(Vec3::new(x, 0.02, z) * CARRIER_SCALE).with_scale(Vec3::splat(3.0)),
                    BlinkLight(x + z),
                ));
            }
        });
    // anillo de recarga detrás de la cubierta, alineado con el eje de la nave
    commands
        .spawn((
            Mesh3d(a.ring.clone()),
            MeshMaterial3d(a.m_ring.clone()),
            Transform::from_translation(pos + REARM_OFFSET).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            RearmRing,
            ZoneEntity,
            bevy::light::NotShadowCaster,
        ))
        .with_children(|r| {
            // flechas guía
            for k in 0..4 {
                let ang = k as f32 * std::f32::consts::FRAC_PI_2;
                r.spawn((
                    Mesh3d(a.plain_cube.clone()),
                    MeshMaterial3d(a.m_ring.clone()),
                    Transform::from_xyz(ang.cos() * 34.0, 0.0, ang.sin() * 34.0).with_rotation(Quat::from_rotation_y(-ang)).with_scale(Vec3::new(6.0, 1.5, 1.5)),
                ));
            }
        });
}

fn spawn_turret_space(commands: &mut Commands, a: &GameAssets, pos: Vec3) {
    let t = a.toon.clone();
    commands
        .spawn((
            Transform::from_translation(pos),
            Visibility::default(),
            Enemy {
                kind: EnemyKind::Turret,
                hp: 90.0,
                max_hp: 90.0,
                radius: 8.0,
                half_extents: None,
                cd: rand::thread_rng().gen_range(1.0..4.0),
                speed: 0.0,
                vel: Vec3::ZERO,
                evade: 0.0,
                evade_dir: Vec3::ZERO,
                score: 200,
                hit_flash: 0.0,
            },
            MothershipTurret,
        ))
        .with_children(|p| {
            t.part(p, Shape::Sphere, &a.m_turret, Transform::from_scale(Vec3::splat(5.0)), 0.4);
            for s in [-1.0f32, 1.0] {
                t.part(p, Shape::Cube, &a.m_building2, Transform::from_xyz(s * 1.6, 0.5, -7.0).with_scale(Vec3::new(0.9, 0.9, 10.0)), 0.25);
            }
            p.spawn((Mesh3d(a.plain_sphere.clone()), MeshMaterial3d(a.m_red_light.clone()), Transform::from_xyz(0.0, 5.0, 0.0).with_scale(Vec3::splat(0.8))));
        });
}

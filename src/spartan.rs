//! Destroid Spartan: mechas terrestres aliados (NPC) que defienden sectores del desierto
//! contra grupos de Invid de asalto. Disparan sus cañones antiaéreos y salvas de
//! micromisiles; los Invid los atacan en picado. Es una batalla de fondo: el jugador puede
//! ayudar, pero no cambia los objetivos de las misiones.

use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};
use rand::Rng;

use crate::audio::{play, Sounds};
use crate::components::*;
use crate::gameplay::{spawn_bullet, spawn_explosion};
use crate::world::{ground_height, spawn_fighter, GameAssets, ATMOS_LIMIT};

pub const SPARTAN_SCALE: f32 = 6.5;
pub const SPARTAN_HP: f32 = 260.0;

/// Sectores de combate terrestre (x, z) y nombre.
pub const SECTORES: [(f32, f32, &str); 4] = [(-1460.0, 1940.0, "OESTE"), (1420.0, 2520.0, "NORTE"), (3240.0, -2000.0, "COLMENA"), (-2160.0, -2160.0, "SUR")];

#[derive(Component)]
pub struct Spartan {
    pub hp: f32,
    pub sector: usize,
    pub gun_cd: f32,
    pub msl_cd: f32,
    pub home: Vec3,
    pub walk: f32,
    pub yaw: f32,
    pub recoil: f32,
}

/// Invid que ataca un sector terrestre.
#[derive(Component)]
pub struct AsaltoTerrestre {
    pub sector: usize,
    pub centro: Vec3,
}

/// Modelo del Spartan dentro de su entidad (para retroceso y caminar).
#[derive(Component)]
pub struct SpartanModel;

#[derive(Resource, Default)]
pub struct Frentes {
    pub refuerzo: [f32; 4],
    pub oleada: [f32; 4],
    pub creados: bool,
}

pub fn spartan_model_transform() -> Transform {
    Transform::from_xyz(0.0, 0.95 * SPARTAN_SCALE, 0.0).with_rotation(Quat::from_rotation_y(crate::spartan::MODEL_YAW)).with_scale(Vec3::splat(SPARTAN_SCALE))
}

/// Giro del modelo para que mire hacia -Z (adelante en Bevy).
pub const MODEL_YAW: f32 = std::f32::consts::PI;

fn sector_center(i: usize) -> Vec3 {
    let (x, z, _) = SECTORES[i];
    Vec3::new(x, ground_height(x, z), z)
}

pub fn spawn_spartan(commands: &mut Commands, a: &GameAssets, sector: usize, k: usize) -> Entity {
    let c = sector_center(sector);
    let ang = k as f32 * 2.1 + sector as f32;
    let off = Vec3::new(ang.cos(), 0.0, ang.sin()) * (30.0 + 18.0 * k as f32);
    let mut p = c + off;
    p.y = ground_height(p.x, p.z);
    let yaw = ang + std::f32::consts::PI;
    commands
        .spawn((
            Transform::from_translation(p).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            Spartan { hp: SPARTAN_HP, sector, gun_cd: 1.0 + k as f32 * 0.3, msl_cd: 5.0 + k as f32 * 3.0, home: p, walk: k as f32, yaw, recoil: 0.0 },
            ZoneEntity,
        ))
        .with_children(|ch| {
            ch.spawn((Transform::default(), Visibility::default(), SpartanModel)).with_children(|m| {
                m.spawn((WorldAssetRoot(a.spartan.clone()), spartan_model_transform()));
            });
        })
        .id()
}

/// Crea los sectores al entrar en la atmósfera, envía refuerzos y oleadas de asalto.
#[allow(clippy::too_many_arguments)]
pub fn frentes(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    game: Res<Game>,
    mut fr: ResMut<Frentes>,
    diff: Res<crate::missions::Difficulty>,
    spartans: Query<&Spartan>,
    asaltos: Query<&AsaltoTerrestre>,
) {
    if std::env::var("NAVE_SIN_SPARTAN").is_ok() {
        return;
    }
    let dt = time.delta_secs();
    if game.zone != Zone::Atmosphere || game.pending_zone.is_some() || !game.started {
        fr.creados = false;
        return;
    }
    if !fr.creados && spartans.is_empty() {
        fr.creados = true;
        for s in 0..SECTORES.len() {
            for k in 0..3 {
                spawn_spartan(&mut commands, &a, s, k);
            }
            fr.oleada[s] = 6.0 + s as f32 * 7.0;
            fr.refuerzo[s] = 0.0;
        }
        return;
    }
    fr.creados = true;
    let mut rng = rand::thread_rng();
    for s in 0..SECTORES.len() {
        let vivos = spartans.iter().filter(|x| x.sector == s && x.hp > 0.0).count();
        // refuerzos: un Spartan nuevo cada 35 s si el sector perdió alguno
        if vivos < 3 {
            fr.refuerzo[s] -= dt;
            if fr.refuerzo[s] <= 0.0 {
                fr.refuerzo[s] = 35.0;
                spawn_spartan(&mut commands, &a, s, rng.gen_range(0..3));
            }
        }
        // oleadas de asalto Invid sobre el sector
        let n = asaltos.iter().filter(|x| x.sector == s).count();
        let max = if diff.media() { 3 } else { 2 };
        if n < max {
            fr.oleada[s] -= dt;
            if fr.oleada[s] <= 0.0 {
                fr.oleada[s] = rng.gen_range(18.0..30.0);
                let c = sector_center(s);
                debug!("Asalto Invid sobre el sector {}", SECTORES[s].2);
                for _ in n..max {
                    let ang = rng.gen_range(0.0..std::f32::consts::TAU);
                    let mut pos = c + Vec3::new(ang.cos(), 0.0, ang.sin()) * rng.gen_range(1300.0..1800.0);
                    pos.x = pos.x.clamp(-ATMOS_LIMIT, ATMOS_LIMIT);
                    pos.z = pos.z.clamp(-ATMOS_LIMIT, ATMOS_LIMIT);
                    pos.y = ground_height(pos.x, pos.z) + rng.gen_range(250.0..450.0);
                    let e = spawn_fighter(&mut commands, &a, pos, c, 1);
                    commands.entity(e).insert(AsaltoTerrestre { sector: s, centro: c });
                }
            }
        }
    }
}

/// Comportamiento de los Spartan: buscan al Invid más cercano, giran el cuerpo, disparan
/// ráfagas antiaéreas y de vez en cuando una salva de micromisiles.
#[allow(clippy::too_many_arguments)]
pub fn spartan_ai(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    sounds: Res<Sounds>,
    dir: Res<Director>,
    mut q: Query<(Entity, &mut Transform, &mut Spartan, &Children)>,
    mut models: Query<&mut Transform, (With<SpartanModel>, Without<Spartan>)>,
    enemies: Query<(Entity, &Transform, &Enemy), (Without<Spartan>, Without<SpartanModel>)>,
    mut cluster: ResMut<crate::fx::ClusterQueue>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    let cam = dir.cam.translation;
    for (se, mut tf, mut sp, children) in &mut q {
        if sp.hp <= 0.0 {
            spawn_explosion(&mut commands, &a, tf.translation + Vec3::Y * 8.0, 18.0, Vec3::ZERO);
            commands.entity(se).despawn();
            continue;
        }
        sp.gun_cd -= dt;
        sp.msl_cd -= dt;
        sp.recoil = (sp.recoil - dt * 4.0).max(0.0);
        let eye = tf.translation + Vec3::Y * 10.0;
        let target = enemies
            .iter()
            .filter(|(_, _, e)| e.kind == EnemyKind::Fighter && e.hp > 0.0)
            .map(|(e, t, en)| (e, t.translation, en.vel, t.translation.distance(eye)))
            .filter(|x| x.3 < 1500.0)
            .min_by(|x, y| x.3.total_cmp(&y.3));
        let mut walking = false;
        if let Some((te, tp, tv, d)) = target {
            // girar el cuerpo hacia el blanco
            let to = tp - tf.translation;
            let want = (-to.x).atan2(-to.z);
            let mut dy = want - sp.yaw;
            while dy > std::f32::consts::PI {
                dy -= std::f32::consts::TAU;
            }
            while dy < -std::f32::consts::PI {
                dy += std::f32::consts::TAU;
            }
            sp.yaw += dy.clamp(-1.6 * dt, 1.6 * dt);
            // cañones antiaéreos (ráfagas)
            if sp.gun_cd <= 0.0 && dy.abs() < 0.6 {
                sp.gun_cd = rng.gen_range(0.12..0.2);
                let lead = tp + tv * (d / 900.0);
                for s in [-1.0f32, 1.0] {
                    let gun = tf.transform_point(Vec3::new(s * 2.4, 10.5, -1.5));
                    let dirv = (lead - gun).normalize_or(Vec3::Y);
                    let spread = Vec3::new(rng.gen_range(-0.03..0.03), rng.gen_range(-0.03..0.03), rng.gen_range(-0.03..0.03));
                    spawn_bullet(&mut commands, &a, gun, (dirv + spread).normalize() * 1100.0, 4.0, true, false);
                }
                sp.recoil = 1.0;
                let dc = tf.translation.distance(cam);
                if dc < 900.0 && rng.gen_bool(0.5) {
                    play(&mut commands, &sounds.flak, 0.22 * (1.0 - dc / 900.0));
                }
            }
            // salva de micromisiles desde los lanzadores de los hombros
            if sp.msl_cd <= 0.0 && d > 250.0 {
                sp.msl_cd = rng.gen_range(9.0..15.0);
                debug!("Spartan del sector {} lanza misiles a {:.0} m", sp.sector, d);
                cluster.salvo(se, &[te], 6, 0.08);
                crate::sfx::sfx(&mut commands, crate::sfx::SfxKind::ClusterLaunch, Some(eye), 0.6);
                if tf.translation.distance(cam) < 2200.0 && rng.gen_bool(0.3) {
                    crate::sfx::bark(&mut commands, if sp.hp < SPARTAN_HP * 0.5 { "#s_ayuda" } else { "#s_fuego" });
                }
            }
        } else {
            // sin enemigos: patrulla a paso lento alrededor de su puesto
            sp.walk += dt * 0.25;
            let goal = sp.home + Vec3::new(sp.walk.cos(), 0.0, sp.walk.sin()) * 15.0;
            let to = goal - tf.translation;
            if to.length() > 2.0 {
                let want = (-to.x).atan2(-to.z);
                let mut dy = want - sp.yaw;
                while dy > std::f32::consts::PI {
                    dy -= std::f32::consts::TAU;
                }
                while dy < -std::f32::consts::PI {
                    dy += std::f32::consts::TAU;
                }
                sp.yaw += dy.clamp(-0.8 * dt, 0.8 * dt);
                let f = Quat::from_rotation_y(sp.yaw) * Vec3::NEG_Z;
                tf.translation += f * 4.0 * dt;
                walking = true;
            }
        }
        tf.rotation = Quat::from_rotation_y(sp.yaw);
        tf.translation.y = ground_height(tf.translation.x, tf.translation.z);
        // retroceso al disparar y balanceo al caminar
        let t = time.elapsed_secs() + sp.home.x * 0.01;
        for c in children.iter() {
            if let Ok(mut m) = models.get_mut(c) {
                let bob = if walking { (t * 5.0).sin().abs() * 0.35 } else { 0.0 };
                let sway = if walking { (t * 2.5).sin() * 0.04 } else { 0.0 };
                m.translation = Vec3::new(0.0, bob, sp.recoil * 0.35);
                m.rotation = Quat::from_rotation_z(sway) * Quat::from_rotation_x(-sp.recoil * 0.03);
            }
        }
    }
}

/// Los Spartan lejanos se dejan de dibujar (rendimiento).
pub fn spartan_lod(
    ev: On<WorldInstanceReady>,
    parents: Query<&ChildOf>,
    spartan_models: Query<(), With<SpartanModel>>,
    children: Query<&Children>,
    meshes: Query<(), With<Mesh3d>>,
    mat_q: Query<&MeshMaterial3d<StandardMaterial>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut done: Local<std::collections::HashSet<AssetId<StandardMaterial>>>,
    mut commands: Commands,
) {
    let ok = parents.get(ev.entity).is_ok_and(|p| spartan_models.get(p.parent()).is_ok());
    if !ok {
        return;
    }
    for c in children.iter_descendants(ev.entity) {
        if meshes.get(c).is_ok() {
            commands.entity(c).insert(bevy::camera::visibility::VisibilityRange { start_margin: 0.0..0.0, end_margin: 3500.0..3800.0, use_aabb: false });
        }
        // la textura viene oscura para la luz del atardecer: la aclaramos (una vez)
        if let Ok(m) = mat_q.get(c) {
            if done.insert(m.0.id()) {
                if let Some(mut mat) = mats.get_mut(&m.0) {
                    mat.base_color = Color::LinearRgba(LinearRgba::rgb(1.7, 1.7, 1.7));
                    mat.metallic = 0.0;
                    mat.perceptual_roughness = 0.6;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Busca puntos secos y planos cerca de cada sector deseado.
    #[test]
    fn buscar_sectores() {
        for (x0, z0) in [(-2600.0f32, 1400.0f32), (1900.0, 2700.0), (2400.0, -2600.0), (-1500.0, -3300.0)] {
            let mut best = (f32::MAX, 0.0, 0.0, 0.0);
            for i in -20..=20 {
                for j in -20..=20 {
                    let (x, z) = (x0 + i as f32 * 60.0, z0 + j as f32 * 60.0);
                    let h = ground_height(x, z);
                    if h < 20.0 {
                        continue;
                    }
                    let mut slope: f32 = 0.0;
                    for (dx, dz) in [(80.0, 0.0), (-80.0, 0.0), (0.0, 80.0), (0.0, -80.0)] {
                        let h2 = ground_height(x + dx, z + dz);
                        slope = slope.max((h2 - h).abs());
                        if h2 < 10.0 {
                            slope = 1e9;
                        }
                    }
                    let score = slope + ((x - x0).powi(2) + (z - z0).powi(2)).sqrt() * 0.01 + (h - 80.0).abs() * 0.05;
                    if score < best.0 {
                        best = (score, x, z, h);
                    }
                }
            }
            println!("SECTOR {x0},{z0} -> ({}, {}) h={} score={}", best.1, best.2, best.3, best.0);
        }
    }
}

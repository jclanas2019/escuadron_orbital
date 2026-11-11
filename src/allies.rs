//! Escuadrón Azul: Valkyries aliados (NPC) que vuelan en formación contigo,
//! defienden la nave nodriza terrestre y combaten a los Invid.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use rand::Rng;

use crate::audio::{play, Sounds};
use crate::components::*;
use crate::gameplay::{spawn_bullet, spawn_explosion};
use crate::models::*;
use crate::world::*;

pub const SQUAD_SIZE: usize = 4;
const ALLY_HP: f32 = 120.0;
/// Posiciones de formación relativas a tu nave (x derecha, y arriba, z atrás): a tus costados, algo adelantados.
const FORMATION: [Vec3; SQUAD_SIZE] = [
    Vec3::new(-32.0, -3.0, -28.0),
    Vec3::new(32.0, -3.0, -28.0),
    Vec3::new(-64.0, -6.0, -8.0),
    Vec3::new(64.0, -6.0, -8.0),
];

#[derive(Component)]
pub struct Ally {
    /// 1..=4 → "AZUL 1"...
    pub slot: usize,
    pub hp: f32,
    pub speed: f32,
    pub vel: Vec3,
    pub cd: f32,
    pub evade: f32,
    pub evade_dir: Vec3,
    pub target: Option<Entity>,
    pub retarget: f32,
    /// false = acaba de entrar a la zona: se coloca en formación junto al jugador.
    pub joined: bool,
    /// Tiempo hasta la próxima salva de micromisiles.
    pub cluster_cd: f32,
}

pub fn ally_name(slot: usize) -> String {
    format!("AZUL {}", slot + 1)
}

/// El modelo del Escuadrón Azul tiene el morro hacia -X, igual que el Valkyrie de vuelo.
fn blue_transform() -> Transform {
    Transform::from_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)).with_scale(Vec3::splat(VALK_SCALE))
}

pub fn spawn_ally(commands: &mut Commands, a: &GameAssets, pos: Vec3, rot: Quat, slot: usize, joined: bool) {
    let vt = blue_transform();
    commands
        .spawn((
            Transform::from_translation(pos).with_rotation(rot),
            Visibility::default(),
            Ally {
                slot,
                hp: ALLY_HP,
                speed: 220.0,
                vel: rot * Vec3::NEG_Z * 220.0,
                cd: 1.0,
                evade: 0.0,
                evade_dir: Vec3::Y,
                target: None,
                retarget: 0.0,
                joined,
                cluster_cd: 8.0 + slot as f32 * 4.0,
            },
            crate::maniobras::Maniobra::default(),
            ZoneEntity,
        ))
        .with_children(|p| {
            p.spawn((WorldAssetRoot(a.valk_blue.clone()), vt));
            for z in [-0.2f32, 0.2] {
                let pos = vt.transform_point(Vec3::new(0.87, -0.05, z));
                p.spawn((
                    Mesh3d(a.plain_sphere.clone()),
                    MeshMaterial3d(a.m_engine.clone()),
                    Transform::from_translation(pos).with_scale(Vec3::new(0.55, 0.55, 0.45)),
                    NotShadowCaster,
                ));
            }
        });
}

/// Al entrar en una zona, el escuadrón despega de la nave nodriza terrestre.
pub fn spawn_squad(commands: &mut Commands, a: &GameAssets, zone: Zone) {
    let base = match zone {
        Zone::Atmosphere => CARRIER_POS_ATMO,
        Zone::Space => CARRIER_POS_SPACE,
    };
    for slot in 0..SQUAD_SIZE {
        let side = if slot % 2 == 0 { -1.0 } else { 1.0 };
        let pos = base + Vec3::new(side * (90.0 + slot as f32 * 25.0), 110.0 + slot as f32 * 12.0, -380.0 - slot as f32 * 40.0);
        spawn_ally(commands, a, pos, Quat::IDENTITY, slot, false);
    }
}

pub fn ally_ai(
    time: Res<Time>,
    mut commands: Commands,
    mut cluster: ResMut<crate::fx::ClusterQueue>,
    a: Res<GameAssets>,
    sounds: Res<Sounds>,
    game: Res<Game>,
    player: Single<(&Transform, &Player), Without<Ally>>,
    mut allies: Query<(Entity, &mut Transform, &mut Ally, Option<&mut crate::maniobras::Maniobra>), Without<Player>>,
    enemies: Query<(Entity, &Transform, &Enemy, Has<AttackCarrier>, Has<crate::missions::Elite>), (Without<Ally>, Without<Player>)>,
    carrier: Query<&Transform, (With<CarrierShip>, Without<Ally>, Without<Player>, Without<Enemy>)>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    let (ptf, pl) = *player;
    let cpos = carrier.iter().next().map(|c| c.translation);
    let enemy_fwd: Vec<(Vec3, Vec3)> = enemies.iter().filter(|e| e.2.kind == EnemyKind::Fighter && e.2.hp > 0.0).map(|(_, t, _, _, _)| (t.translation, t.forward().as_vec3())).collect();
    for (ally_e, mut tf, mut al, mut man) in &mut allies {
        if !al.joined && (al.slot < 2 || cpos.is_none()) {
            // entrada a la zona: los escoltas aparecen en formación a tu alrededor
            al.joined = true;
            tf.translation = ptf.transform_point(FORMATION[al.slot % SQUAD_SIZE]);
            tf.rotation = ptf.rotation;
            al.speed = pl.speed;
            continue;
        }
        al.joined = true;
        al.cd -= dt;
        al.cluster_cd -= dt;
        al.evade -= dt;
        al.retarget -= dt;
        // Azul 1-2 escoltan al jugador; Azul 3-4 defienden la nave nodriza terrestre.
        let escort = al.slot < 2 || cpos.is_none();
        if al.retarget <= 0.0 || al.target.is_some_and(|t| enemies.get(t).is_err()) {
            al.retarget = rng.gen_range(1.5..3.0);
            al.target = enemies
                .iter()
                .filter(|(_, _, e, _, elite)| e.kind == EnemyKind::Fighter && e.hp > 0.0 && !elite)
                .filter_map(|(ent, t, _, vs_c, _)| {
                    let p = t.translation;
                    if escort {
                        let d = p.distance(ptf.translation);
                        (d < 1000.0).then_some((ent, d))
                    } else {
                        let c = cpos.unwrap_or(ptf.translation);
                        let d = p.distance(c);
                        let near = d < 1800.0 || (vs_c && d < 3000.0);
                        near.then_some((ent, p.distance(tf.translation) * if vs_c { 0.5 } else { 1.0 }))
                    }
                })
                .min_by(|x, y| x.1.total_cmp(&y.1))
                .map(|x| x.0);
        }
        let target = al.target.and_then(|t| enemies.get(t).ok()).map(|(_, t, e, _, _)| (t.translation, e.vel));
        let mut desired;
        let mut want_speed;
        match target {
            Some((tp, tv)) => {
                let to = tp - tf.translation;
                let dist = to.length();
                if dist < 110.0 && al.evade <= 0.0 {
                    al.evade = rng.gen_range(1.2..2.2);
                    let r = Vec3::new(rng.gen_range(-1.0..1.0), rng.gen_range(-0.3..1.0), rng.gen_range(-1.0..1.0));
                    al.evade_dir = (r - to.normalize_or(Vec3::Z) * 0.7).normalize_or(Vec3::Y);
                }
                desired = if al.evade > 0.0 { al.evade_dir } else { (to + tv * (dist / 1500.0)).normalize_or(Vec3::NEG_Z) };
                want_speed = if dist > 900.0 { 330.0 } else { 240.0 };
                let fwd = tf.forward().as_vec3();
                // salva de micromisiles en racimo (circo Itano)
                if al.cluster_cd <= 0.0 && dist > 300.0 && dist < 1800.0 && fwd.angle_between(to) < 0.7 {
                    al.cluster_cd = rng.gen_range(18.0..32.0);
                    if let Some(t) = al.target {
                        cluster.salvo(ally_e, &[t], 6, 0.06);
                        crate::sfx::sfx(&mut commands, crate::sfx::SfxKind::ClusterLaunch, Some(tf.translation), 0.7);
                        if tf.translation.distance(ptf.translation) < 1800.0 && rng.gen_bool(0.6) {
                            crate::sfx::bark(&mut commands, ["#a1_racimo", "#a2_engage", "#a3_racimo", "#a4_racimo"][al.slot.min(3)]);
                        }
                    }
                }
                if al.cd <= 0.0 && dist < 1000.0 && fwd.angle_between(to) < 0.10 {
                    al.cd = rng.gen_range(0.18..0.35);
                    for s in [-1.0f32, 1.0] {
                        let gun = tf.transform_point(Vec3::new(s * 3.2, -1.2, -4.0));
                        let dir = (tp + tv * (dist / 1500.0) - gun).normalize_or(fwd);
                        let spread = Vec3::new(rng.gen_range(-0.012..0.012), rng.gen_range(-0.012..0.012), rng.gen_range(-0.012..0.012));
                        spawn_bullet(&mut commands, &a, gun, (dir + spread).normalize() * 1500.0 + al.vel * 0.5, 7.0, true, false);
                    }
                    let d = tf.translation.distance(ptf.translation);
                    if d < 600.0 {
                        play(&mut commands, &sounds.laser, 0.08 * (1.0 - d / 600.0));
                    }
                }
            }
            None if !escort => {
                // patrulla en círculo alrededor de la nave nodriza
                let c = cpos.unwrap_or(ptf.translation);
                let rel = tf.translation - c;
                let ang = rel.z.atan2(rel.x) + 0.35;
                let r = 750.0 + al.slot as f32 * 60.0;
                let goal = c + Vec3::new(ang.cos() * r, 180.0 + al.slot as f32 * 40.0, ang.sin() * r);
                desired = (goal - tf.translation).normalize_or(tf.forward().as_vec3());
                want_speed = 230.0;
            }
            None => {
                // formación con el jugador
                let slot_pos = ptf.transform_point(FORMATION[al.slot % SQUAD_SIZE]);
                let to = slot_pos - tf.translation;
                let dist = to.length();
                let pf = ptf.forward().as_vec3();
                desired = if dist > 25.0 { (to.normalize() + pf * (40.0 / dist.max(1.0)).min(1.5)).normalize() } else { pf };
                want_speed = (pl.speed + (to.dot(pf) * 0.8).clamp(-80.0, 200.0)).clamp(90.0, 600.0);
                if game.dead {
                    want_speed = 220.0;
                }
            }
        }
        // evitar el suelo y el techo de la atmósfera
        if game.zone == Zone::Atmosphere {
            let g = ground_height(tf.translation.x, tf.translation.z);
            let clr = tf.translation.y - g;
            if clr < 200.0 {
                desired = (desired + Vec3::Y * ((200.0 - clr) / 50.0)).normalize();
            }
            if tf.translation.y > CEILING - 250.0 {
                desired = (desired - Vec3::Y).normalize();
            }
        }
        // evitar el casco de la nave nodriza
        if let Some(c) = cpos {
            let away = tf.translation - c;
            let d = away.length();
            if d < 420.0 {
                desired = (desired + away.normalize_or(Vec3::Y) * ((420.0 - d) / 120.0)).normalize();
            }
        }
        // maniobras evasivas cuando un Invid se les pone a la cola
        let mut guia = None;
        if let Some(m) = man.as_mut() {
            if !m.activa() && m.cd <= 0.0 {
                let f = tf.forward().as_vec3();
                if let Some((ep, _)) = enemy_fwd.iter().find(|(ep, ef)| crate::maniobras::en_la_cola(tf.translation, f, *ep, *ef, 750.0)) {
                    if rng.gen_bool((4.0 * dt).min(1.0) as f64) {
                        let alt = if game.zone == Zone::Atmosphere { tf.translation.y - ground_height(tf.translation.x, tf.translation.z) } else { 1e5 };
                        m.empezar(&tf, crate::maniobras::Amenaza { desde: *ep, misil: false }, alt, &mut rng, 1.0);
                        if al.slot == 1 && rng.gen_bool(0.3) {
                            crate::sfx::bark(&mut commands, "#a2_rapido");
                        }
                    }
                }
            }
            guia = m.guiar(&tf, dt);
        }
        let fwd = tf.forward().as_vec3();
        if let Some((d, u)) = guia {
            let mut d = d;
            if game.zone == Zone::Atmosphere {
                let clr = tf.translation.y - ground_height(tf.translation.x, tf.translation.z);
                if clr < 160.0 {
                    d = (d + Vec3::Y * ((160.0 - clr) / 40.0)).normalize();
                }
            }
            let target_rot = Transform::IDENTITY.looking_to(d, u).rotation;
            tf.rotation = tf.rotation.slerp(target_rot, (3.6 * dt).min(1.0));
            want_speed = want_speed.max(330.0);
        } else {
            let up = if desired.y.abs() > 0.98 { Vec3::Z } else { Vec3::Y };
            let target_rot = Transform::IDENTITY.looking_to(desired, up).rotation;
            tf.rotation = tf.rotation.slerp(target_rot, (2.2 * dt).min(1.0));
            // alabeo en los virajes
            let turn = fwd.cross(desired).y;
            tf.rotate_local_z(turn.clamp(-1.0, 1.0) * 3.5 * dt);
        }
        al.speed += (want_speed - al.speed) * (1.0 - (-1.2 * dt).exp());
        let f = tf.forward().as_vec3();
        al.vel = f * al.speed;
        tf.translation += al.vel * dt;
        if game.zone == Zone::Atmosphere {
            let g = ground_height(tf.translation.x, tf.translation.z);
            tf.translation.y = tf.translation.y.max(g + 25.0);
        }
    }
}

/// Derribos de aliados y reemplazos que despegan desde la nave nodriza terrestre.
pub fn ally_status(
    time: Res<Time>,
    mut commands: Commands,
    a: Res<GameAssets>,
    sounds: Res<Sounds>,
    mut game: ResMut<Game>,
    mut respawn: Local<f32>,
    allies: Query<(Entity, &Transform, &Ally)>,
    carrier: Query<&Transform, (With<CarrierShip>, Without<Ally>)>,
) {
    let dt = time.delta_secs();
    let mut alive = [false; SQUAD_SIZE];
    for (e, tf, al) in &allies {
        if al.hp <= 0.0 {
            spawn_explosion(&mut commands, &a, tf.translation, 16.0, al.vel);
            play(&mut commands, &sounds.explosion, 0.6);
            play(&mut commands, &sounds.kick, 0.54);
            game.message(format!("¡{} derribado! Reemplazo en camino", ally_name(al.slot)), 3.0);
            commands.entity(e).try_despawn();
        } else {
            alive[al.slot % SQUAD_SIZE] = true;
        }
    }
    *respawn -= dt;
    if *respawn <= 0.0 {
        if let (Some(slot), Some(c)) = (alive.iter().position(|x| !x), carrier.iter().next()) {
            *respawn = 15.0;
            let pos = c.transform_point(Vec3::new(0.0, 0.18 * CARRIER_SCALE, -0.6 * CARRIER_SCALE));
            spawn_ally(&mut commands, &a, pos, c.rotation, slot, true);
            if game.msg_timer <= 0.0 {
                game.message(format!("{} despega desde la nave nodriza", ally_name(slot)), 2.5);
            }
        } else {
            *respawn = 2.0;
        }
    }
}

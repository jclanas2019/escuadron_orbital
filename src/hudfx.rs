//! HUD vectorial verde proyectado en el espacio (como en las cabinas del anime):
//! escalera de cabeceo anclada al horizonte, cintas de velocidad y altitud,
//! retículas con púas sobre los enemigos, y líneas de velocidad al usar el turbo.

use bevy::prelude::*;
use rand::Rng;

use crate::components::*;
use crate::gameplay::LASER_SPEED;
use crate::toon::seg7;
use crate::world::*;

const D: f32 = 60.0;

fn hud_col(a: f32) -> Color {
    Color::LinearRgba(LinearRgba::new(0.35, 2.6, 1.0, a))
}
fn red_col() -> Color {
    Color::LinearRgba(LinearRgba::new(3.2, 0.3, 0.25, 1.0))
}

pub fn setup_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (cfg, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    cfg.line.width = 2.2;
    // el HUD vectorial sólo en la cámara principal (no en la cámara de puntería)
    cfg.render_layers = bevy::camera::visibility::RenderLayers::layer(1);
}

/// Números de 7 segmentos dibujados con líneas.
fn num(g: &mut Gizmos, origin: Vec3, right: Vec3, up: Vec3, size: f32, s: &str, col: Color) {
    let w = size * 0.55;
    let mut x = 0.0;
    for ch in s.chars() {
        let segs = seg7(ch);
        let o = origin + right * x;
        let p = |a: f32, b: f32| o + right * (a * w) + up * (b * size);
        let lines = [
            (p(0.0, 1.0), p(1.0, 1.0)),
            (p(1.0, 1.0), p(1.0, 0.5)),
            (p(1.0, 0.5), p(1.0, 0.0)),
            (p(0.0, 0.0), p(1.0, 0.0)),
            (p(0.0, 0.5), p(0.0, 0.0)),
            (p(0.0, 1.0), p(0.0, 0.5)),
            (p(0.0, 0.5), p(1.0, 0.5)),
        ];
        for (i, (a, b)) in lines.iter().enumerate() {
            if segs & (1 << i) != 0 {
                g.line(*a, *b, col);
            }
        }
        x += w + size * 0.3;
    }
}

pub fn draw_hud(
    time: Res<Time<Real>>,
    game: Res<Game>,
    dir: Res<Director>,
    player: Single<(&Transform, &Player)>,
    enemies: Query<(Entity, &Transform, &Enemy), Without<Player>>,
    rings: Query<&Transform, (With<RearmRing>, Without<Player>)>,
    allies: Query<(&Transform, &crate::allies::Ally), Without<Player>>,
    mut g: Gizmos,
) {
    if !game.started || game.dead {
        return;
    }
    let view = dir.current();
    let (tf, pl) = *player;
    let t = time.elapsed_secs();

    // ------- líneas de velocidad (turbo) en todas las vistas cercanas
    if pl.boosting && view != CamView::Flyby {
        let mut rng = rand::thread_rng();
        let c = dir.cam.translation;
        let f = dir.cam.forward().as_vec3();
        let r = dir.cam.right().as_vec3();
        let u = dir.cam.up().as_vec3();
        for _ in 0..26 {
            let a = rng.gen_range(0.0..std::f32::consts::TAU);
            let rad = rng.gen_range(3.0..7.0);
            let start = c + f * rng.gen_range(4.0..40.0) + (r * a.cos() + u * a.sin()) * rad;
            g.line(start, start - f * rng.gen_range(6.0..16.0), Color::srgba(1.0, 1.0, 1.0, 0.55));
        }
    }

    if !(view == CamView::Pov || view == CamView::Shoulder) {
        return;
    }
    let flicker = if game.hit_flash > 0.2 && (t * 30.0).sin() > 0.0 { 0.3 } else { 1.0 };
    let col = hud_col(flicker);
    let col_dim = hud_col(0.55 * flicker);
    let eye = tf.translation;
    let f = tf.forward().as_vec3();
    let r = tf.right().as_vec3();
    let u = tf.up().as_vec3();

    // ------- escalera de cabeceo (anclada al horizonte real)
    let fh = Vec3::new(f.x, 0.0, f.z).normalize_or(Vec3::NEG_Z);
    let rh = fh.cross(Vec3::Y).normalize();
    let pitch = f.y.clamp(-1.0, 1.0).asin().to_degrees();
    let mut deg = ((pitch / 10.0).round() * 10.0 - 30.0) as i32;
    while deg <= (pitch + 30.0) as i32 {
        if deg.abs() <= 90 {
            let th = (deg as f32).to_radians();
            let dvec = fh * th.cos() + Vec3::Y * th.sin();
            let upp = -fh * th.sin() + Vec3::Y * th.cos();
            let cpos = eye + dvec * D;
            let (w, gap) = if deg == 0 { (D * 0.40, D * 0.05) } else { (D * 0.11, D * 0.045) };
            for s in [-1.0f32, 1.0] {
                let a = cpos + rh * s * gap;
                let b = cpos + rh * s * w;
                if deg < 0 {
                    for k in 0..3 {
                        let k0 = k as f32 / 3.0;
                        let k1 = k0 + 0.2;
                        g.line(a.lerp(b, k0), a.lerp(b, k1), col);
                    }
                } else {
                    g.line(a, b, col);
                }
                if deg != 0 {
                    let tick = -upp * (deg.signum() as f32) * D * 0.014;
                    g.line(b, b + tick, col);
                    let label = format!("{}", deg.abs());
                    let lpos = b + rh * s * D * 0.012 + if s < 0.0 { -rh * D * 0.026 } else { Vec3::ZERO } - upp * D * 0.006;
                    num(&mut g, lpos, rh, upp, D * 0.013, &label, col_dim);
                }
            }
        }
        deg += 10;
    }

    // ------- marcador de morro "W" (fijo al avión)
    let n = eye + f * D;
    let s = D * 0.012;
    g.linestrip([n - r * s * 3.0, n - r * s * 1.5, n - r * s * 0.75 - u * s, n, n + r * s * 0.75 - u * s, n + r * s * 1.5, n + r * s * 3.0], col);

    // ------- cinta de rumbo (arriba)
    let hdg = (f.x.atan2(-f.z).to_degrees() + 360.0) % 360.0;
    let top = eye + f * D + u * D * 0.30;
    let span = 30.0;
    let base_deg = (hdg / 5.0).floor() as i32 * 5;
    for k in -7..=7 {
        let dd = base_deg + k * 5;
        let off = (dd as f32 - hdg) / span * D * 0.35;
        if off.abs() > D * 0.35 {
            continue;
        }
        let p = top + r * off;
        let big = dd.rem_euclid(10) == 0;
        g.line(p, p + u * D * if big { 0.018 } else { 0.009 }, col);
        if dd.rem_euclid(30) == 0 {
            let lbl = format!("{:02}", (dd.rem_euclid(360)) / 10);
            num(&mut g, p + u * D * 0.026 - r * D * 0.008, r, u, D * 0.013, &lbl, col);
        }
    }
    g.line(top - r * D * 0.35, top + r * D * 0.35, col_dim);
    g.linestrip([top - u * D * 0.012 - r * D * 0.008, top, top - u * D * 0.012 + r * D * 0.008], col);

    // ------- cintas de velocidad (izq.) y altitud (der.)
    let (alt, alt_step) = match game.zone {
        Zone::Atmosphere => (eye.y, 50.0),
        Zone::Space => ((eye.distance(EARTH_CENTER) - EARTH_RADIUS) * 12.0, 1000.0),
    };
    for (side, value, step) in [(-1.0f32, pl.speed, 10.0f32), (1.0, alt, alt_step)] {
        let c0 = eye + f * D + r * side * D * 0.30 - u * D * 0.02;
        let range = 8.0;
        let base = (value / step).floor();
        for k in -6..=6 {
            let v = (base + k as f32) * step;
            let off = (v - value) / (step * range) * D * 0.22;
            if off.abs() > D * 0.22 {
                continue;
            }
            let p = c0 + u * off;
            let major = ((v / step).round() as i32).rem_euclid(5) == 0;
            g.line(p, p + r * side * D * if major { 0.022 } else { 0.012 }, col);
            if major {
                let lbl = format!("{}", (v / if step >= 1000.0 { 1000.0 } else { 1.0 }).round() as i32);
                let len = lbl.len() as f32 * D * 0.012;
                let lp = if side < 0.0 { p - r * (D * 0.006 + len) } else { p + r * D * 0.03 };
                num(&mut g, lp - u * D * 0.005, r, u, D * 0.011, &lbl, col_dim);
            }
        }
        g.line(c0 - u * D * 0.22, c0 + u * D * 0.22, col_dim);
        // caja con el valor actual
        let bx = c0 - r * side * D * 0.075;
        let hw = D * 0.055;
        let hh = D * 0.014;
        g.linestrip([bx - r * hw - u * hh, bx + r * hw - u * hh, bx + r * hw + u * hh, bx - r * hw + u * hh, bx - r * hw - u * hh], col);
        let lbl = format!("{}", value.max(0.0).round() as i64 % 100000);
        num(&mut g, bx - r * hw * 0.85 - u * hh * 0.75, r, u, D * 0.02, &lbl, col);
    }

    // ------- misiles y calor (abajo a la izquierda)
    let bl = eye + f * D - r * D * 0.30 - u * D * 0.26;
    for i in 0..8 {
        let p = bl + r * (i as f32) * D * 0.016;
        let on = (i as u32) < game.missiles;
        let h = if on { D * 0.022 } else { D * 0.006 };
        g.line(p, p + u * h, if on { col } else { col_dim });
    }
    // salvas de racimo: tres rombos a la derecha de los misiles
    for i in 0..3 {
        let c = bl + r * (D * 0.145 + i as f32 * D * 0.018) + u * D * 0.011;
        let on = (i as u32) < game.cluster;
        let k = D * if on { 0.007 } else { 0.004 };
        g.linestrip([c + u * k, c + r * k, c - u * k, c - r * k, c + u * k], if on { col } else { col_dim });
    }
    let hb = bl - u * D * 0.02;
    g.line(hb, hb + r * D * 0.13 * game.heat.min(1.0), if game.overheated { red_col() } else { col });

    // ------- advertencia: "X" gigante para levantar el morro
    if game.zone == Zone::Atmosphere {
        let agl = eye.y - ground_height(eye.x, eye.z);
        let sink = -f.y * pl.speed;
        if agl < 250.0 && sink > 0.0 && agl / sink.max(1.0) < 5.0 && (t * 6.0).sin() > 0.0 {
            let x = D * 0.14;
            g.line(n - r * x - u * x, n + r * x + u * x, red_col());
            g.line(n - r * x + u * x, n + r * x - u * x, red_col());
        }
    }

    // ------- marcador del punto de recarga (rombo verde con distancia)
    for rt in &rings {
        let rp = rt.translation;
        let to = rp - eye;
        let dist = to.length();
        if to.dot(f) > 0.0 && dist > 40.0 {
            let view_dir = (dir.cam.translation - rp).normalize_or(Vec3::Z);
            let rot = Quat::from_rotation_arc(Vec3::Z, view_dir);
            let (rr, ru) = (rot * Vec3::X, rot * Vec3::Y);
            let k = dist * 0.025;
            let c = if game.rearm_cd <= 0.0 { Color::LinearRgba(LinearRgba::new(0.4, 3.0, 2.0, 1.0)) } else { Color::LinearRgba(LinearRgba::new(1.5, 0.4, 0.3, 0.8)) };
            g.linestrip([rp + ru * k, rp + rr * k, rp - ru * k, rp - rr * k, rp + ru * k], c);
            num(&mut g, rp - ru * k * 1.9 - rr * k * 0.6, rr, ru, k * 0.5, &format!("{}", (dist / 10.0).round() as i32), c);
        }
    }

    // ------- Escuadrón Azul: chevrón azul con su número
    for (at, al) in &allies {
        let ap = at.translation;
        let to = ap - eye;
        let dist = to.length();
        if to.dot(f) > 0.0 && dist < 4000.0 {
            let view_dir = (dir.cam.translation - ap).normalize_or(Vec3::Z);
            let rot = Quat::from_rotation_arc(Vec3::Z, view_dir);
            let (rr, ru) = (rot * Vec3::X, rot * Vec3::Y);
            let k = (dist * 0.018).max(3.0);
            let c = Color::LinearRgba(LinearRgba::new(0.5, 1.6, 4.0, 1.0));
            let top = ap + ru * k * 1.6;
            g.linestrip([top - rr * k * 0.7 + ru * k * 0.6, top, top + rr * k * 0.7 + ru * k * 0.6], c);
            num(&mut g, top + ru * k * 0.9 - rr * k * 0.15, rr, ru, k * 0.5, &format!("{}", al.slot + 1), c);
        }
    }

    // ------- retículas sobre los enemigos (parpadean con la interferencia)
    if game.jam > 0.0 && (t * 7.0).sin() > -0.6 {
        return;
    }
    let cam = dir.cam.translation;
    let mut lead_done = false;
    for (e, etf, en) in &enemies {
        if en.hp <= 0.0 {
            continue;
        }
        let pos = etf.translation;
        let to = pos - eye;
        let dist = to.length();
        if dist > 6000.0 || to.dot(f) <= 0.0 {
            continue;
        }
        if en.kind == EnemyKind::Turret && dist > 3000.0 && game.lock != Some(e) {
            continue;
        }
        let view_dir = (cam - pos).normalize_or(Vec3::Z);
        let rot = Quat::from_rotation_arc(Vec3::Z, view_dir);
        let base_r = dist * if en.kind == EnemyKind::Mothership { 0.06 } else { 0.022 };
        let rr = rot * Vec3::X;
        let ru = rot * Vec3::Y;
        let is_lock = game.lock == Some(e);
        let c = if is_lock && game.locked { red_col() } else { col };
        if is_lock {
            // anillo con púas que gira, más hexágono interior al fijar
            let shrink = if game.locked { 1.0 } else { 1.0 + (1.0 - (game.lock_time / 0.8).min(1.0)) * 1.5 };
            let r0 = base_r * shrink;
            g.circle(Isometry3d::new(pos, rot), r0, c).resolution(32);
            let spin = t * 1.5;
            for k in 0..8 {
                let a = spin + k as f32 * std::f32::consts::TAU / 8.0;
                let dvec = rr * a.cos() + ru * a.sin();
                let side = rr * (-a.sin()) + ru * a.cos();
                let p0 = pos + dvec * r0;
                g.linestrip([p0 - side * r0 * 0.12, p0 + dvec * r0 * 0.28, p0 + side * r0 * 0.12], c);
            }
            if game.locked {
                let hex: Vec<Vec3> = (0..=6)
                    .map(|k| {
                        let a = -spin * 0.8 + k as f32 * std::f32::consts::TAU / 6.0;
                        pos + (rr * a.cos() + ru * a.sin()) * r0 * 0.55
                    })
                    .collect();
                g.linestrip(hex, c);
            }
        } else {
            // corchetes en las esquinas
            let k = base_r * 0.8;
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = pos + rr * sx * k + ru * sy * k;
                g.line(corner, corner - rr * sx * k * 0.45, c);
                g.line(corner, corner - ru * sy * k * 0.45, c);
            }
        }
        // distancia en dígitos
        if dist < 3000.0 || is_lock {
            let lbl = format!("{}", (dist / 10.0).round() as i32);
            num(&mut g, pos - ru * base_r * 1.5 - rr * base_r * 0.5, rr, ru, base_r * 0.35, &lbl, c);
        }
        // punto de tiro anticipado
        if !lead_done && en.kind == EnemyKind::Fighter && dist < 1800.0 && (is_lock || game.lock.is_none()) {
            let lp = pos + en.vel * (dist / LASER_SPEED);
            g.circle(Isometry3d::new(lp, rot), base_r * 0.25, red_col()).resolution(12);
            lead_done = true;
        }
    }
}

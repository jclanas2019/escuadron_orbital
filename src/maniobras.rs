//! Maniobras de combate aéreo estilo "Top Gun" para los Invid y el Escuadrón Azul:
//! viraje de ruptura (break), tonel (barrel roll), Split-S, Immelmann y zigzag (jink).
//! Se disparan cuando alguien se les pone a la cola apuntándoles, cuando un misil los
//! persigue o al cruzarse de frente.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use rand::Rng;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tipo {
    #[default]
    Ninguna,
    Ruptura,
    Tonel,
    SplitS,
    Immelmann,
    Zigzag,
}

#[derive(Component, Clone, Copy, Default, Debug)]
pub struct Maniobra {
    pub tipo: Tipo,
    pub t: f32,
    pub dur: f32,
    pub lado: f32,
    pub dir0: Vec3,
    /// enfriamiento hasta poder iniciar otra maniobra
    pub cd: f32,
}

/// Amenaza detectada: dirección desde la que viene y si es un misil.
#[derive(Clone, Copy)]
pub struct Amenaza {
    pub desde: Vec3,
    pub misil: bool,
}

impl Maniobra {
    pub fn activa(&self) -> bool {
        self.t > 0.0
    }

    /// Elige y empieza una maniobra según la amenaza y la altura disponible.
    pub fn empezar(&mut self, tf: &Transform, am: Amenaza, altura_libre: f32, rng: &mut impl Rng, agresividad: f32) {
        let fwd = tf.forward().as_vec3();
        let right = tf.right().as_vec3();
        // girar hacia el lado contrario a la amenaza (o hacia ella si viene de frente)
        let rel = am.desde - tf.translation;
        self.lado = if rel.dot(right) > 0.0 { -1.0 } else { 1.0 };
        if rng.gen_bool(0.2) {
            self.lado = -self.lado;
        }
        let detras = rel.normalize_or(Vec3::Z).dot(fwd) < -0.3;
        let mut opciones: Vec<(Tipo, f32)> = vec![(Tipo::Ruptura, 3.0), (Tipo::Zigzag, 1.5), (Tipo::Tonel, if detras { 2.5 } else { 1.0 })];
        if altura_libre > 600.0 {
            opciones.push((Tipo::SplitS, if detras { 2.0 } else { 0.8 }));
        }
        opciones.push((Tipo::Immelmann, if detras { 0.8 } else { 1.8 }));
        if am.misil {
            // contra un misil: viraje cerrado perpendicular o tonel defensivo
            opciones = vec![(Tipo::Ruptura, 3.0), (Tipo::Tonel, 2.0)];
        }
        let total: f32 = opciones.iter().map(|o| o.1).sum();
        let mut x = rng.gen_range(0.0..total);
        let mut tipo = Tipo::Ruptura;
        for (t, w) in &opciones {
            if x < *w {
                tipo = *t;
                break;
            }
            x -= w;
        }
        self.tipo = tipo;
        self.dur = match tipo {
            Tipo::Ruptura => rng.gen_range(1.2..1.9),
            Tipo::Tonel => rng.gen_range(1.4..1.9),
            Tipo::SplitS | Tipo::Immelmann => rng.gen_range(2.0..2.6),
            Tipo::Zigzag => rng.gen_range(1.0..1.6),
            Tipo::Ninguna => 0.0,
        };
        self.t = self.dur;
        self.dir0 = Vec3::new(fwd.x, if matches!(tipo, Tipo::SplitS | Tipo::Immelmann) { 0.0 } else { fwd.y }, fwd.z).normalize_or(fwd);
        self.cd = self.dur + rng.gen_range(1.5..4.0) / agresividad.max(0.3);
    }

    /// Dirección deseada y vector "arriba" (para alabear) mientras dura la maniobra.
    pub fn guiar(&mut self, tf: &Transform, dt: f32) -> Option<(Vec3, Vec3)> {
        self.cd -= dt;
        if self.t <= 0.0 {
            return None;
        }
        self.t -= dt;
        let p = (1.0 - self.t / self.dur.max(0.01)).clamp(0.0, 1.0);
        let fwd = tf.forward().as_vec3();
        // ejes horizontales del mundo (no los de la nave, que giran al alabear)
        let hr = fwd.cross(Vec3::Y).normalize_or(Vec3::X);
        let r0 = self.dir0.cross(Vec3::Y).normalize_or(Vec3::X);
        let u0 = r0.cross(self.dir0).normalize_or(Vec3::Y);
        let s = self.lado;
        let r = match self.tipo {
            Tipo::Ruptura => {
                // alabeo de ~80° y tirón: giro continuo y cerrado
                let d = (fwd * 0.35 + hr * s).normalize();
                let u = (Vec3::Y * 0.25 + hr * s).normalize();
                (d, u)
            }
            Tipo::Tonel => {
                let ph = p * TAU * s;
                let off = r0 * ph.cos() + u0 * ph.sin();
                let d = (self.dir0 + off * 0.55).normalize();
                let u = (Quat::from_axis_angle(self.dir0, -ph) * u0).normalize();
                (d, u)
            }
            Tipo::SplitS => {
                // medio tonel invertido y medio rizo hacia abajo: invierte el rumbo bajando
                let a = PI * p;
                let d = (self.dir0 * a.cos() - Vec3::Y * a.sin()).normalize();
                let u = (-self.dir0 * a.sin() - Vec3::Y * a.cos()).normalize();
                (d, u)
            }
            Tipo::Immelmann => {
                // medio rizo hacia arriba y medio tonel al final
                let a = PI * p.min(0.85) / 0.85;
                let d = (self.dir0 * a.cos() + Vec3::Y * a.sin()).normalize_or(fwd);
                let u0 = (-self.dir0 * a.sin() + Vec3::Y * a.cos()).normalize_or(Vec3::Y);
                let u = if p > 0.85 { u0.lerp(Vec3::Y, (p - 0.85) / 0.15).normalize_or(Vec3::Y) } else { u0 };
                (d, u)
            }
            Tipo::Zigzag => {
                let k = (p * 9.0 * PI).sin().signum();
                let d = (self.dir0 + r0 * k * s * 0.7 + u0 * (p * 5.0 * PI).cos() * 0.35).normalize();
                let u = (Vec3::Y * 0.4 + r0 * k * s).normalize();
                (d, u)
            }
            Tipo::Ninguna => return None,
        };
        if self.t <= 0.0 {
            self.tipo = Tipo::Ninguna;
        }
        Some(r)
    }
}

/// ¿Está `atacante` (con dirección `fwd_atacante`) a la cola de `pos` y apuntándole?
pub fn en_la_cola(pos: Vec3, fwd_obj: Vec3, atacante: Vec3, fwd_atacante: Vec3, rango: f32) -> bool {
    let to = pos - atacante;
    let d = to.length();
    if d > rango || d < 1.0 {
        return false;
    }
    let apunta = fwd_atacante.angle_between(to) < 0.28;
    let detras = (atacante - pos).normalize().dot(fwd_obj) < 0.2;
    apunta && detras
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simular(tipo: Tipo) -> (Transform, Transform) {
        let mut tf = Transform::from_xyz(0.0, 1000.0, 0.0);
        let ini = tf;
        let mut m = Maniobra { tipo, t: 2.0, dur: 2.0, lado: 1.0, dir0: Vec3::NEG_Z, cd: 0.0 };
        let dt = 1.0 / 60.0;
        while let Some((d, u)) = m.guiar(&tf, dt) {
            assert!(d.is_finite() && u.is_finite());
            let target = Transform::IDENTITY.looking_to(d, u).rotation;
            tf.rotation = tf.rotation.slerp(target, (3.4 * dt).min(1.0));
            let f = tf.forward().as_vec3();
            tf.translation += f * 200.0 * dt;
        }
        (ini, tf)
    }

    #[test]
    fn split_s_invierte_rumbo_y_baja() {
        let (ini, fin) = simular(Tipo::SplitS);
        assert!(fin.forward().z > 0.3, "rumbo final {:?}", fin.forward());
        assert!(fin.translation.y < ini.translation.y - 50.0);
    }

    #[test]
    fn immelmann_invierte_rumbo_y_sube() {
        let (ini, fin) = simular(Tipo::Immelmann);
        assert!(fin.forward().z > 0.3, "rumbo final {:?}", fin.forward());
        assert!(fin.translation.y > ini.translation.y + 50.0);
    }

    #[test]
    fn ruptura_gira() {
        let (_, fin) = simular(Tipo::Ruptura);
        assert!(fin.forward().angle_between(Vec3::NEG_Z) > 1.0);
    }

    #[test]
    fn tonel_y_zigzag_estables() {
        for t in [Tipo::Tonel, Tipo::Zigzag] {
            let (_, fin) = simular(t);
            assert!(fin.forward().angle_between(Vec3::NEG_Z) < 1.2, "{t:?} se fue de rumbo");
        }
    }
}

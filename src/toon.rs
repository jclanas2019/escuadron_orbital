//! Estética de animación cel (anime de los 90):
//! - sombreado de dos tonos "horneado" en colores de vértice (luz / sombra violeta)
//! - contornos de tinta negros con la técnica de "casco invertido"
//! - un pequeño rasterizador 2D para dibujar las pantallas de la cabina.

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, Face, TextureDimension, TextureFormat};

/// Luz "clave" fija en espacio de objeto para las piezas que se mueven.
pub const KEY_LIGHT: Vec3 = Vec3::new(0.35, 0.85, 0.40);
/// Tinte de las sombras de animación (violeta azulado).
pub const SHADOW: [f32; 3] = [0.50, 0.44, 0.70];

pub fn toon_band(ndl: f32) -> [f32; 4] {
    if ndl > 0.12 {
        [1.0, 1.0, 1.0, 1.0]
    } else {
        [SHADOW[0], SHADOW[1], SHADOW[2], 1.0]
    }
}

/// Graba el sombreado cel en los colores de vértice del mesh.
pub fn bake_toon(mut m: Mesh, light: Vec3) -> Mesh {
    let l = light.normalize();
    if m.attribute(Mesh::ATTRIBUTE_NORMAL).is_none() {
        m.compute_normals();
    }
    let cols: Vec<[f32; 4]> = match m.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(n)) => n.iter().map(|n| toon_band(Vec3::from(*n).dot(l))).collect(),
        _ => vec![],
    };
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, cols);
    m
}

/// Material "plano" (sin iluminación): el color es exactamente el pintado.
pub fn flat(mats: &mut Assets<StandardMaterial>, c: Color) -> Handle<StandardMaterial> {
    mats.add(StandardMaterial {
        base_color: c,
        unlit: true,
        ..default()
    })
}

/// Material que brilla (valores HDR > 1 → bloom).
pub fn glow(mats: &mut Assets<StandardMaterial>, r: f32, g: f32, b: f32) -> Handle<StandardMaterial> {
    mats.add(StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::rgb(r, g, b)),
        unlit: true,
        fog_enabled: false,
        ..default()
    })
}

/// Material de contorno: negro, dibujando sólo las caras traseras.
pub fn ink(mats: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    mats.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.02, 0.05),
        unlit: true,
        cull_mode: Some(Face::Front),
        ..default()
    })
}

#[derive(Clone, Copy, PartialEq)]
pub enum Shape {
    Cube,
    Sphere,
    Cyl,
    Capsule,
}

/// Meshes compartidos con sombreado cel.
#[derive(Clone)]
pub struct ToonMeshes {
    pub cube: Handle<Mesh>,
    pub sphere: Handle<Mesh>,
    pub cyl: Handle<Mesh>,
    pub capsule: Handle<Mesh>,
    pub ink: Handle<StandardMaterial>,
}

impl ToonMeshes {
    pub fn new(meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>) -> Self {
        Self {
            cube: meshes.add(bake_toon(Cuboid::new(1.0, 1.0, 1.0).into(), KEY_LIGHT)),
            sphere: meshes.add(bake_toon(Sphere::new(1.0).mesh().uv(24, 14), KEY_LIGHT)),
            cyl: meshes.add(bake_toon(Cylinder::new(1.0, 1.0).mesh().resolution(16).into(), KEY_LIGHT)),
            capsule: meshes.add(bake_toon(Capsule3d::new(0.5, 1.0).mesh().into(), KEY_LIGHT)),
            ink: ink(mats),
        }
    }
    pub fn mesh(&self, s: Shape) -> Handle<Mesh> {
        match s {
            Shape::Cube => self.cube.clone(),
            Shape::Sphere => self.sphere.clone(),
            Shape::Cyl => self.cyl.clone(),
            Shape::Capsule => self.capsule.clone(),
        }
    }

    /// Crea una pieza con contorno de tinta de grosor `t` (en metros).
    pub fn part(&self, p: &mut ChildSpawnerCommands, s: Shape, mat: &Handle<StandardMaterial>, tf: Transform, t: f32) -> Entity {
        let id = p
            .spawn((Mesh3d(self.mesh(s)), MeshMaterial3d(mat.clone()), tf, NotShadowCaster))
            .id();
        if t > 0.0 {
            let grow = match s {
                Shape::Cube => Vec3::splat(2.0 * t),
                Shape::Sphere => Vec3::splat(t),
                Shape::Cyl => Vec3::new(t, 2.0 * t, t),
                Shape::Capsule => Vec3::splat(2.0 * t),
            };
            p.spawn((
                Mesh3d(self.mesh(s)),
                MeshMaterial3d(self.ink.clone()),
                tf.with_scale(tf.scale + grow),
                NotShadowCaster,
            ));
        }
        id
    }

    /// Pieza que une dos puntos (brazos, barras del marco...).
    pub fn beam(&self, p: &mut ChildSpawnerCommands, s: Shape, mat: &Handle<StandardMaterial>, a: Vec3, b: Vec3, thick: f32, t: f32) -> Entity {
        let d = b - a;
        let tf = Transform::from_translation((a + b) * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, d.normalize()))
            .with_scale(Vec3::new(thick, d.length(), thick));
        self.part(p, s, mat, tf, t)
    }
}

// ---------------------------------------------------------------- lienzo 2D

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

pub type Rgb = [u8; 3];

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        Self { w, h, data: vec![0; w * h * 4] }
    }
    pub fn clear(&mut self, c: Rgb) {
        for px in self.data.chunks_exact_mut(4) {
            px[0] = c[0];
            px[1] = c[1];
            px[2] = c[2];
            px[3] = 255;
        }
    }
    #[inline]
    pub fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 4;
        self.data[i] = c[0];
        self.data[i + 1] = c[1];
        self.data[i + 2] = c[2];
        self.data[i + 3] = 255;
    }
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, c);
            }
        }
    }
    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, t: i32, c: Rgb) {
        self.rect(x, y, w, t, c);
        self.rect(x, y + h - t, w, t, c);
        self.rect(x, y, t, h, c);
        self.rect(x + w - t, y, t, h, c);
    }
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, c: Rgb) {
        self.ring(cx, cy, 0.0, r, 0.0, std::f32::consts::TAU, c);
    }
    /// Anillo (o sector) entre radios r0..r1 y ángulos a0..a1 (radianes, 0 = derecha, sentido horario en pantalla).
    #[allow(clippy::too_many_arguments)]
    pub fn ring(&mut self, cx: f32, cy: f32, r0: f32, r1: f32, a0: f32, a1: f32, c: Rgb) {
        let full = (a1 - a0) >= std::f32::consts::TAU - 1e-3;
        for y in (cy - r1).floor() as i32..=(cy + r1).ceil() as i32 {
            for x in (cx - r1).floor() as i32..=(cx + r1).ceil() as i32 {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let d = (dx * dx + dy * dy).sqrt();
                if d < r0 || d > r1 {
                    continue;
                }
                if !full {
                    let mut a = dy.atan2(dx);
                    while a < a0 {
                        a += std::f32::consts::TAU;
                    }
                    if a > a1 {
                        continue;
                    }
                }
                self.put(x, y, c);
            }
        }
    }
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, t: f32, c: Rgb) {
        let n = ((x1 - x0).abs().max((y1 - y0).abs()) * 1.5) as i32 + 1;
        let r = (t * 0.5).max(0.5);
        for i in 0..=n {
            let k = i as f32 / n as f32;
            let x = x0 + (x1 - x0) * k;
            let y = y0 + (y1 - y0) * k;
            if r <= 0.75 {
                self.put(x as i32, y as i32, c);
            } else {
                self.disc(x, y, r, c);
            }
        }
    }
    /// Trapecio horizontal (para el indicador en pirámide invertida).
    pub fn trapezoid(&mut self, cx: f32, y: i32, h: i32, w_top: f32, w_bot: f32, c: Rgb) {
        for yy in 0..h {
            let k = yy as f32 / h.max(1) as f32;
            let w = w_top + (w_bot - w_top) * k;
            self.rect((cx - w / 2.0) as i32, y + yy, w as i32, 1, c);
        }
    }
    /// Dígitos de 7 segmentos.
    pub fn digits(&mut self, x: i32, y: i32, size: i32, s: &str, c: Rgb) {
        let mut cx = x;
        let w = size * 6 / 10;
        let t = (size / 8).max(1);
        for ch in s.chars() {
            let segs = seg7(ch);
            let hw = w;
            let hh = size / 2;
            let on = |i: usize| segs & (1 << i) != 0;
            if on(0) { self.rect(cx, y, hw, t, c); }
            if on(1) { self.rect(cx + hw - t, y, t, hh, c); }
            if on(2) { self.rect(cx + hw - t, y + hh, t, hh, c); }
            if on(3) { self.rect(cx, y + size - t, hw, t, c); }
            if on(4) { self.rect(cx, y + hh, t, hh, c); }
            if on(5) { self.rect(cx, y, t, hh, c); }
            if on(6) { self.rect(cx, y + hh - t / 2, hw, t, c); }
            cx += w + size / 4;
        }
    }
    /// Pinta un píxel con transparencia propia (para capas superpuestas).
    pub fn put_a(&mut self, x: i32, y: i32, c: Rgb, a: u8) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 4;
        self.data[i] = c[0];
        self.data[i + 1] = c[1];
        self.data[i + 2] = c[2];
        self.data[i + 3] = a;
    }
    /// Deja todo el lienzo transparente.
    pub fn clear_transparent(&mut self) {
        for v in self.data.iter_mut() {
            *v = 0;
        }
    }
    /// Texto con una fuente de 5×7 píxeles (mayúsculas, dígitos y algunos signos).
    pub fn text(&mut self, x: i32, y: i32, scale: i32, s: &str, c: Rgb) {
        let mut cx = x;
        for ch in s.chars() {
            let g = glyph(ch.to_ascii_uppercase());
            for (row, bits) in g.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0x10 >> col) != 0 {
                        self.rect(cx + col * scale, y + row as i32 * scale, scale, scale, c);
                    }
                }
            }
            cx += 6 * scale;
        }
    }
    pub fn text_width(s: &str, scale: i32) -> i32 {
        s.chars().count() as i32 * 6 * scale - scale
    }
    /// Relleno de polígono (prueba de punto dentro, suficiente para siluetas).
    pub fn poly(&mut self, pts: &[(f32, f32)], c: Rgb) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in pts {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
        for y in y0.floor() as i32..=y1.ceil() as i32 {
            for x in x0.floor() as i32..=x1.ceil() as i32 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let mut inside = false;
                let n = pts.len();
                let mut j = n - 1;
                for i in 0..n {
                    let (xi, yi) = pts[i];
                    let (xj, yj) = pts[j];
                    if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
                        inside = !inside;
                    }
                    j = i;
                }
                if inside {
                    self.put(x, y, c);
                }
            }
        }
    }
    pub fn poly_outline(&mut self, pts: &[(f32, f32)], t: f32, c: Rgb) {
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            self.line(a.0, a.1, b.0, b.1, t, c);
        }
    }
    pub fn to_image(&self) -> Image {
        Image::new(
            Extent3d { width: self.w as u32, height: self.h as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            self.data.clone(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
    }
}

/// Bits a..g de un display de 7 segmentos.
pub fn seg7(c: char) -> u8 {
    match c {
        '0' => 0b0111111,
        '1' => 0b0000110,
        '2' => 0b1011011,
        '3' => 0b1001111,
        '4' => 0b1100110,
        '5' => 0b1101101,
        '6' => 0b1111101,
        '7' => 0b0000111,
        '8' => 0b1111111,
        '9' => 0b1101111,
        '-' => 0b1000000,
        _ => 0,
    }
}

/// Textura de franjas de peligro naranja/negro.
pub fn hazard_image() -> Image {
    let mut c = Canvas::new(128, 32);
    for y in 0..32 {
        for x in 0..128 {
            let band = ((x + y) / 12) % 2 == 0;
            c.put(x, y, if band { [235, 120, 20] } else { [18, 14, 20] });
        }
    }
    c.to_image()
}

/// Fuente de 5×7 píxeles: cada fila son 5 bits.
pub fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        '-' => [0, 0, 0, 0x1F, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 0x0C, 0x0C],
        ':' => [0, 0x0C, 0x0C, 0, 0x0C, 0x0C, 0],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        '%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03],
        '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        '<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '+' => [0, 0x04, 0x04, 0x1F, 0x04, 0x04, 0],
        '[' => [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E],
        ']' => [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E],
        _ => [0; 7],
    }
}

/// Esfera de instrumento analógico: borde, marcas y zona roja.
pub fn gauge_face_image() -> Image {
    let n = 128usize;
    let mut c = Canvas::new(n, n);
    c.clear([12, 12, 18]);
    let (cx, cy) = (64.0, 64.0);
    c.disc(cx, cy, 60.0, [235, 232, 220]);
    c.disc(cx, cy, 54.0, [20, 20, 28]);
    let pi = std::f32::consts::PI;
    // arco de 270° desde abajo-izquierda
    for i in 0..=10 {
        let a = pi * 0.75 + i as f32 / 10.0 * pi * 1.5;
        let r0 = if i % 2 == 0 { 38.0 } else { 44.0 };
        let col = if i >= 8 { [240, 70, 60] } else { [235, 232, 220] };
        c.line(cx + a.cos() * r0, cy + a.sin() * r0, cx + a.cos() * 52.0, cy + a.sin() * 52.0, 3.0, col);
    }
    c.ring(cx, cy, 48.0, 52.0, pi * 0.75 + pi * 1.2, pi * 2.25, [240, 70, 60]);
    c.disc(cx, cy, 6.0, [80, 80, 90]);
    c.to_image()
}

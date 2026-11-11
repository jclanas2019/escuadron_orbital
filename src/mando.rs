//! Mando de juego (gamepad): Nintendo (Switch Pro / Joy-Con), PlayStation (DualSense PS5 /
//! DualShock) y genéricos tipo Xbox.
//!
//! * Las asignaciones se leen de `assets/mando.cfg` (una acción por línea, nombres de botón de
//!   Bevy por POSICIÓN: South = abajo, East = derecha...).
//! * El diseño (`diseño = auto | nintendo | ps5 | xbox`) sólo cambia los nombres que se muestran
//!   (✕ ○ □ △ / B A Y X / A B X Y). F8 lo cambia en el juego. En `auto` se detecta por el
//!   fabricante del mando.
//! * Los botones se traducen a las mismas teclas del teclado, así todo el juego responde igual.
//!   Los sticks y gatillos se suman al ratón y a las teclas como entradas analógicas.
//! * Vibración en impactos, explosiones cercanas y lanzamientos.
//! * F7: modo prueba, muestra el nombre de cada botón o eje que pulsas.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton, GamepadInput, GamepadRumbleIntensity, GamepadRumbleRequest};
use bevy::prelude::*;

use crate::audio::AssetsDir;
use crate::components::*;

pub const CFG_DEFECTO: &str = include_str!("../assets/mando.cfg");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Estilo {
    Auto,
    Nintendo,
    PlayStation,
    Xbox,
}

impl Estilo {
    pub fn nombre(self) -> &'static str {
        match self {
            Estilo::Auto => "automático",
            Estilo::Nintendo => "Nintendo",
            Estilo::PlayStation => "PlayStation 5",
            Estilo::Xbox => "Xbox / genérico",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Accion {
    CabecearArriba,
    CabecearAbajo,
    AlabearIzquierda,
    AlabearDerecha,
    GuinarIzquierda,
    GuinarDerecha,
    Acelerar,
    Frenar,
    Disparar,
    Misil,
    Racimo,
    Transformar,
    Turbo,
    Camara,
    Cortes,
    Ayuda,
    Musica,
    Pausa,
    InvertirY,
}

/// (acción, nombre en mando.cfg, tecla equivalente, descripción para la ayuda)
const ACCIONES: &[(Accion, &str, Option<KeyCode>, &str)] = &[
    (Accion::CabecearArriba, "cabecear_arriba", None, ""),
    (Accion::CabecearAbajo, "cabecear_abajo", None, ""),
    (Accion::AlabearIzquierda, "alabear_izquierda", None, ""),
    (Accion::AlabearDerecha, "alabear_derecha", None, ""),
    (Accion::GuinarIzquierda, "guinar_izquierda", None, ""),
    (Accion::GuinarDerecha, "guinar_derecha", None, ""),
    (Accion::Acelerar, "acelerar", None, "acelerar"),
    (Accion::Frenar, "frenar", None, "frenar"),
    (Accion::Disparar, "disparar", Some(KeyCode::Space), "láseres"),
    (Accion::Misil, "misil", Some(KeyCode::KeyF), "misil"),
    (Accion::Racimo, "racimo", Some(KeyCode::KeyV), "racimo"),
    (Accion::Transformar, "transformar", Some(KeyCode::KeyG), "transformar"),
    (Accion::Turbo, "turbo", Some(KeyCode::ShiftLeft), "turbo"),
    (Accion::Camara, "camara", Some(KeyCode::KeyC), "cámara"),
    (Accion::Cortes, "cortes", Some(KeyCode::KeyX), "cortes cine"),
    (Accion::Ayuda, "ayuda", Some(KeyCode::KeyH), "ayuda"),
    (Accion::Musica, "musica", Some(KeyCode::KeyM), "música"),
    (Accion::Pausa, "pausa", Some(KeyCode::Escape), "pausa"),
    (Accion::InvertirY, "invertir_y", Some(KeyCode::KeyI), "invertir Y"),
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Entrada {
    Boton(GamepadButton),
    Eje(GamepadAxis, f32),
}

#[derive(Resource)]
pub struct Mando {
    pub estilo_cfg: Estilo,
    pub estilo: Estilo,
    pub intercambiar_ab: bool,
    pub map: Vec<(Accion, Vec<Entrada>)>,
    pub umbral: f32,
    pub zona_muerta: f32,
    pub curva: f32,
    pub vibracion: f32,
    pub nombre: Option<String>,
    pub activo: Option<Entity>,
    // entradas analógicas de este fotograma
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
    pub acel: f32,
    pub freno: f32,
    pub prueba: bool,
    estado: HashMap<Accion, bool>,
    teclas: HashSet<KeyCode>,
    ult_shake: f32,
    ult_flash: f32,
    ult_misiles: u32,
    ult_racimo: u32,
    /// valores de reposo medidos al conectar (sticks descentrados, gatillos que reposan en -1)
    pub base: HashMap<GamepadInput, f32>,
    /// rango (mín, máx) visto durante la calibración, para detectar ejes inestables
    cal_rango: HashMap<GamepadInput, (f32, f32)>,
    /// ejes que se mueven solos (giroscopio / acelerómetro): se ignoran
    pub ruido: HashSet<GamepadInput>,
    /// segundos de calibración que faltan
    cal_t: f32,
    /// el mando no actúa hasta que se pulsa un botón en él
    pub armado: bool,
    pub asistente: Option<Asistente>,
    /// texto del mando.cfg general (para recargar perfiles)
    pub cfg_texto: String,
    pub dir_perfiles: std::path::PathBuf,
    pub perfil: Option<String>,
}

/// Asistente de configuración (F7): pregunta cada control y guarda un perfil del mando.
#[derive(Clone, Debug)]
pub struct Asistente {
    pub paso: usize,
    pub espera_suelto: bool,
    pub res: Vec<(Accion, Entrada)>,
}

/// Pasos del asistente: acción y lo que se le pide al jugador.
pub const PASOS: &[(Accion, &str)] = &[
    (Accion::Disparar, "pulsa el botón para DISPARAR LÁSERES"),
    (Accion::Misil, "pulsa el botón para el MISIL"),
    (Accion::Racimo, "pulsa el botón para MISILES EN RACIMO"),
    (Accion::Transformar, "pulsa el botón para TRANSFORMAR"),
    (Accion::Turbo, "pulsa el botón de TURBO"),
    (Accion::Camara, "pulsa el botón para CAMBIAR CÁMARA"),
    (Accion::Pausa, "pulsa el botón de PAUSA (Start / + / OPTIONS)"),
    (Accion::CabecearArriba, "empuja el STICK IZQUIERDO hacia ARRIBA"),
    (Accion::CabecearAbajo, "empuja el STICK IZQUIERDO hacia ABAJO"),
    (Accion::AlabearIzquierda, "empuja el STICK IZQUIERDO a la IZQUIERDA"),
    (Accion::AlabearDerecha, "empuja el STICK IZQUIERDO a la DERECHA"),
    (Accion::GuinarIzquierda, "empuja el STICK DERECHO a la IZQUIERDA"),
    (Accion::GuinarDerecha, "empuja el STICK DERECHO a la DERECHA"),
    (Accion::Acelerar, "aprieta el GATILLO para ACELERAR (ZR / R2)"),
    (Accion::Frenar, "aprieta el GATILLO para FRENAR (ZL / L2)"),
];

/// Dispositivos que no son mandos de juego (el sensor de movimiento del Switch Pro y de los
/// Joy-Con aparece como un "mando" aparte y sus ejes no paran de moverse).
pub fn es_sensor(nombre: &str) -> bool {
    let n = nombre.to_lowercase();
    ["imu", "motion", "gyro", "accel", "sensor", "touchpad", "trackpad"].iter().any(|k| n.contains(k))
}

pub fn nombre_boton(b: GamepadButton) -> String {
    use GamepadButton::*;
    match b {
        Other(n) => format!("Otro{n}"),
        o => format!("{o:?}"),
    }
}

pub fn nombre_entrada(e: &Entrada) -> String {
    match *e {
        Entrada::Boton(b) => nombre_boton(b),
        Entrada::Eje(ax, s) => {
            let base = match ax {
                GamepadAxis::Other(n) => format!("Eje{n}"),
                o => format!("{o:?}"),
            };
            format!("{base}{}", if s > 0.0 { "+" } else { "-" })
        }
    }
}

fn slug(nombre: &str) -> String {
    let s: String = nombre.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    s.trim_matches('_').to_string()
}

fn parse_boton(s: &str) -> Option<GamepadButton> {
    use GamepadButton::*;
    Some(match s {
        "South" => South,
        "East" => East,
        "North" => North,
        "West" => West,
        "LeftTrigger" => LeftTrigger,
        "RightTrigger" => RightTrigger,
        "LeftTrigger2" => LeftTrigger2,
        "RightTrigger2" => RightTrigger2,
        "Select" => Select,
        "Start" => Start,
        "Mode" => Mode,
        "LeftThumb" => LeftThumb,
        "RightThumb" => RightThumb,
        "DPadUp" => DPadUp,
        "DPadDown" => DPadDown,
        "DPadLeft" => DPadLeft,
        "DPadRight" => DPadRight,
        "C" => C,
        "Z" => Z,
        o if o.starts_with("Otro") => Other(o[4..].parse().ok()?),
        _ => return None,
    })
}

fn parse_entrada(s: &str) -> Option<Entrada> {
    if let Some(b) = parse_boton(s) {
        return Some(Entrada::Boton(b));
    }
    let (name, sign) = if let Some(n) = s.strip_suffix('+') {
        (n, 1.0)
    } else if let Some(n) = s.strip_suffix('-') {
        (n, -1.0)
    } else {
        return None;
    };
    use GamepadAxis::*;
    let ax = match name {
        "LeftStickX" => LeftStickX,
        "LeftStickY" => LeftStickY,
        "RightStickX" => RightStickX,
        "RightStickY" => RightStickY,
        "LeftZ" => LeftZ,
        "RightZ" => RightZ,
        o if o.starts_with("Eje") => Other(o[3..].parse().ok()?),
        _ => return None,
    };
    Some(Entrada::Eje(ax, sign))
}

fn si(v: &str) -> bool {
    matches!(v.trim().to_lowercase().as_str(), "si" | "sí" | "yes" | "true" | "1")
}

impl Mando {
    pub fn desde_texto(txt: &str) -> Self {
        let mut m = Mando {
            estilo_cfg: Estilo::Auto,
            estilo: Estilo::Xbox,
            intercambiar_ab: false,
            map: Vec::new(),
            umbral: 0.5,
            zona_muerta: 0.12,
            curva: 1.6,
            vibracion: 1.0,
            nombre: None,
            activo: None,
            pitch: 0.0,
            yaw: 0.0,
            roll: 0.0,
            acel: 0.0,
            freno: 0.0,
            prueba: false,
            estado: HashMap::new(),
            teclas: HashSet::new(),
            ult_shake: 0.0,
            ult_flash: 0.0,
            ult_misiles: 8,
            ult_racimo: 3,
            base: HashMap::new(),
            cal_rango: HashMap::new(),
            ruido: HashSet::new(),
            cal_t: 0.0,
            armado: false,
            asistente: None,
            cfg_texto: txt.to_string(),
            dir_perfiles: std::path::PathBuf::from("assets/mandos"),
            perfil: None,
        };
        for raw in txt.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "diseño" | "diseno" => {
                    m.estilo_cfg = match v.to_lowercase().as_str() {
                        "nintendo" | "switch" => Estilo::Nintendo,
                        "ps5" | "ps4" | "playstation" | "dualsense" => Estilo::PlayStation,
                        "xbox" | "generico" | "genérico" => Estilo::Xbox,
                        _ => Estilo::Auto,
                    }
                }
                "intercambiar_ab" => m.intercambiar_ab = si(v),
                "umbral" => m.umbral = v.parse::<f32>().unwrap_or(0.5).clamp(0.3, 0.9),
                "zona_muerta" => m.zona_muerta = v.parse::<f32>().unwrap_or(0.12).clamp(0.0, 0.5),
                "curva" => m.curva = v.parse::<f32>().unwrap_or(1.6).clamp(1.0, 3.0),
                "vibracion" | "vibración" => m.vibracion = v.parse::<f32>().unwrap_or(1.0).clamp(0.0, 1.0),
                _ => {
                    if let Some((a, _, _, _)) = ACCIONES.iter().find(|x| x.1 == k) {
                        let ents: Vec<Entrada> = v.split_whitespace().filter_map(parse_entrada).collect();
                        m.map.push((*a, ents));
                    }
                }
            }
        }
        m
    }

    fn entradas(&self, a: Accion) -> &[Entrada] {
        self.map.iter().find(|x| x.0 == a).map(|x| x.1.as_slice()).unwrap_or(&[])
    }

    /// Con `intercambiar_ab` (algunos controladores de Nintendo informan los botones por
    /// letra y no por posición) se cambian A↔B y X↔Y.
    fn fisico(&self, b: GamepadButton) -> GamepadButton {
        use GamepadButton::*;
        if !self.intercambiar_ab {
            return b;
        }
        match b {
            South => East,
            East => South,
            North => West,
            West => North,
            o => o,
        }
    }

    fn valor(&self, g: &Gamepad, a: Accion) -> f32 {
        let mut v: f32 = 0.0;
        for e in self.entradas(a) {
            v = v.max(self.valor_entrada(g, e));
        }
        v
    }

    /// Valor 0..1 de una entrada, medido desde su posición de reposo.
    pub fn valor_entrada(&self, g: &Gamepad, e: &Entrada) -> f32 {
        match *e {
            Entrada::Boton(b) => {
                let b = self.fisico(b);
                let pressed = if g.pressed(b) { 1.0 } else { 0.0 };
                let base = self.base.get(&GamepadInput::Button(b)).copied().unwrap_or(0.0);
                let analog = g.get(b).map(|x| ((x - base) / (1.0 - base).max(0.2)).max(0.0)).unwrap_or(0.0);
                // un botón que ya estaba "pulsado" en reposo se ignora
                let pressed = if base > 0.5 { 0.0 } else { pressed };
                analog.max(pressed).min(1.0)
            }
            Entrada::Eje(ax, s) => {
                if self.ruido.contains(&GamepadInput::Axis(ax)) {
                    return 0.0;
                }
                let raw = g.get_unclamped(ax).or_else(|| g.get(ax)).unwrap_or(0.0);
                let base = self.base.get(&GamepadInput::Axis(ax)).copied().unwrap_or(0.0);
                // gatillos analógicos que reposan en -1 (o +1): recorrido de 2
                let rango = if base.abs() > 0.6 { 2.0 } else { 1.0 };
                ((raw - base) * s / rango).clamp(0.0, 1.0)
            }
        }
    }

    /// Aplica un perfil de mando (sólo las líneas de acciones; el resto sigue igual).
    pub fn aplicar_perfil(&mut self, txt: &str) {
        let p = Mando::desde_texto(txt);
        for (a, ents) in p.map {
            if let Some(x) = self.map.iter_mut().find(|x| x.0 == a) {
                x.1 = ents;
            } else {
                self.map.push((a, ents));
            }
        }
        if txt.contains("intercambiar_ab") {
            self.intercambiar_ab = p.intercambiar_ab;
        }
    }

    /// Desviación de cada entrada respecto al reposo (para el asistente).
    fn desviaciones(&self, g: &Gamepad) -> Vec<(Entrada, f32)> {
        let mut v = Vec::new();
        for b in g.get_pressed() {
            let base = self.base.get(&GamepadInput::Button(*b)).copied().unwrap_or(0.0);
            if base < 0.5 {
                v.push((Entrada::Boton(*b), 1.0));
            }
        }
        for (inp, val) in g.analog().all_axes_and_values() {
            if let GamepadInput::Axis(ax) = inp {
                if self.ruido.contains(inp) {
                    continue;
                }
                let base = self.base.get(inp).copied().unwrap_or(0.0);
                let d = val - base;
                let rango = if base.abs() > 0.6 { 2.0 } else { 1.0 };
                v.push((Entrada::Eje(*ax, if d >= 0.0 { 1.0 } else { -1.0 }), d.abs() / rango));
            }
        }
        v
    }

    fn analog(&self, v: f32) -> f32 {
        let dz = self.zona_muerta;
        if v <= dz {
            0.0
        } else {
            ((v - dz) / (1.0 - dz)).clamp(0.0, 1.0).powf(self.curva)
        }
    }

    pub fn etiqueta(&self, b: GamepadButton) -> String {
        use GamepadButton::*;
        let s = match (self.estilo, b) {
            (Estilo::PlayStation, South) => "✕",
            (Estilo::PlayStation, East) => "○",
            (Estilo::PlayStation, West) => "□",
            (Estilo::PlayStation, North) => "△",
            (Estilo::PlayStation, LeftTrigger) => "L1",
            (Estilo::PlayStation, RightTrigger) => "R1",
            (Estilo::PlayStation, LeftTrigger2) => "L2",
            (Estilo::PlayStation, RightTrigger2) => "R2",
            (Estilo::PlayStation, Select) => "CREATE",
            (Estilo::PlayStation, Start) => "OPTIONS",
            (Estilo::PlayStation, Mode) => "PS",
            (Estilo::PlayStation, LeftThumb) => "L3",
            (Estilo::PlayStation, RightThumb) => "R3",
            (Estilo::Nintendo, South) => "B",
            (Estilo::Nintendo, East) => "A",
            (Estilo::Nintendo, West) => "Y",
            (Estilo::Nintendo, North) => "X",
            (Estilo::Nintendo, LeftTrigger) => "L",
            (Estilo::Nintendo, RightTrigger) => "R",
            (Estilo::Nintendo, LeftTrigger2) => "ZL",
            (Estilo::Nintendo, RightTrigger2) => "ZR",
            (Estilo::Nintendo, Select) => "−",
            (Estilo::Nintendo, Start) => "+",
            (Estilo::Nintendo, Mode) => "HOME",
            (Estilo::Nintendo, LeftThumb) => "L-stick↓",
            (Estilo::Nintendo, RightThumb) => "R-stick↓",
            (_, South) => "A",
            (_, East) => "B",
            (_, West) => "X",
            (_, North) => "Y",
            (_, LeftTrigger) => "LB",
            (_, RightTrigger) => "RB",
            (_, LeftTrigger2) => "LT",
            (_, RightTrigger2) => "RT",
            (_, Select) => "VIEW",
            (_, Start) => "MENU",
            (_, Mode) => "GUIDE",
            (_, LeftThumb) => "LS↓",
            (_, RightThumb) => "RS↓",
            (_, DPadUp) => "cruceta↑",
            (_, DPadDown) => "cruceta↓",
            (_, DPadLeft) => "cruceta←",
            (_, DPadRight) => "cruceta→",
            (_, Other(n)) => return format!("Otro{n}"),
            _ => "?",
        };
        s.to_string()
    }

    fn etiqueta_entrada(&self, e: &Entrada) -> String {
        match *e {
            Entrada::Boton(b) => self.etiqueta(b),
            Entrada::Eje(ax, s) => {
                let (st, horiz) = match ax {
                    GamepadAxis::LeftStickX => ("stick izq", true),
                    GamepadAxis::LeftStickY => ("stick izq", false),
                    GamepadAxis::RightStickX => ("stick der", true),
                    GamepadAxis::RightStickY => ("stick der", false),
                    GamepadAxis::LeftZ => ("LeftZ", false),
                    GamepadAxis::RightZ => ("RightZ", false),
                    GamepadAxis::Other(n) => return format!("Eje{n}{}", if s > 0.0 { "+" } else { "-" }),
                };
                let flecha = match (horiz, s > 0.0) {
                    (true, true) => "→",
                    (true, false) => "←",
                    (false, true) => "↑",
                    (false, false) => "↓",
                };
                format!("{st}{flecha}")
            }
        }
    }

    fn etiquetas(&self, a: Accion) -> String {
        let v: Vec<String> = self.entradas(a).iter().filter(|e| matches!(e, Entrada::Boton(_))).map(|e| self.etiqueta_entrada(e)).collect();
        if v.is_empty() {
            self.entradas(a).first().map(|e| self.etiqueta_entrada(e)).unwrap_or_else(|| "—".into())
        } else {
            v.join("/")
        }
    }

    /// Texto de ayuda con los nombres del diseño elegido (se muestra si hay mando).
    pub fn ayuda(&self) -> String {
        let Some(n) = &self.nombre else {
            return "\n\nMANDO: conecta uno (Nintendo, PS5 o Xbox)".into();
        };
        let e = |a| self.etiquetas(a);
        format!(
            "\n\nMANDO {} · diseño {} (F8)\n\
             stick izq  cabeceo / alabeo   stick der  guiñada\n\
             {} acelerar   {} frenar   {} turbo\n\
             {} láseres   {} misil   {} racimo\n\
             {} transformar   {} cámara   {} cortes\n\
             {} pausa · empezar · reiniciar   {} ayuda   {} música",
            n.chars().take(22).collect::<String>(),
            self.estilo.nombre(),
            e(Accion::Acelerar),
            e(Accion::Frenar),
            e(Accion::Turbo),
            e(Accion::Disparar),
            e(Accion::Misil),
            e(Accion::Racimo),
            e(Accion::Transformar),
            e(Accion::Camara),
            e(Accion::Cortes),
            e(Accion::Pausa),
            e(Accion::Ayuda),
            e(Accion::Musica),
        )
    }
}

fn detectar(g: &Gamepad, nombre: &str) -> Estilo {
    let n = nombre.to_lowercase();
    match g.vendor_id() {
        Some(0x057e) => return Estilo::Nintendo,
        Some(0x054c) => return Estilo::PlayStation,
        Some(0x045e) => return Estilo::Xbox,
        _ => {}
    }
    if n.contains("nintendo") || n.contains("switch") || n.contains("joy-con") || n.contains("pro controller") {
        Estilo::Nintendo
    } else if n.contains("dualsense") || n.contains("sony") || n.contains("ps5") || n.contains("ps4") || n.contains("playstation") || n.contains("dualshock") || n == "wireless controller" {
        Estilo::PlayStation
    } else {
        Estilo::Xbox
    }
}

pub fn setup_mando(mut commands: Commands, dir: Res<AssetsDir>) {
    let path = dir.0.join("mando.cfg");
    let txt = std::fs::read_to_string(&path).unwrap_or_else(|_| CFG_DEFECTO.to_string());
    let mut m = Mando::desde_texto(&txt);
    m.dir_perfiles = dir.0.join("mandos");
    commands.insert_resource(m);
}

/// Se ejecuta en PreUpdate justo después de leer el teclado: convierte el mando en teclas
/// virtuales y en entradas analógicas.
#[allow(clippy::too_many_arguments)]
pub fn mando_input(
    time: Res<Time<Real>>,
    mut m: ResMut<Mando>,
    pads: Query<(Entity, &Gamepad, Option<&Name>)>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut game: ResMut<Game>,
    invert: Option<Res<crate::gameplay::InvertY>>,
    intro: Option<Res<crate::models::Intro>>,
    mut rumble: MessageWriter<GamepadRumbleRequest>,
) {
    // F8: cambiar diseño de los nombres de botón · F6: modo prueba · F7: asistente
    if keys.just_pressed(KeyCode::F8) {
        m.estilo_cfg = match m.estilo_cfg {
            Estilo::Auto => Estilo::Nintendo,
            Estilo::Nintendo => Estilo::PlayStation,
            Estilo::PlayStation => Estilo::Xbox,
            Estilo::Xbox => Estilo::Auto,
        };
        let txt = format!("Diseño del mando: {}", m.estilo_cfg.nombre());
        game.message(txt, 2.0);
    }
    if keys.just_pressed(KeyCode::F6) {
        m.prueba = !m.prueba;
        game.message(if m.prueba { "PRUEBA DEL MANDO: pulsa botones (F6 para salir)" } else { "Prueba del mando: desactivada" }, 2.5);
    }

    // mando activo: se ignoran los sensores de movimiento; si hay varios, manda el último
    // en el que se pulsó un botón
    let candidatos: Vec<(Entity, &Gamepad, String)> = pads
        .iter()
        .map(|(e, g, n)| (e, g, n.map(|n| n.as_str().to_string()).unwrap_or_else(|| "Mando".into())))
        .filter(|(_, _, n)| !es_sensor(n))
        .collect();
    let pulsado = candidatos.iter().find(|(_, g, _)| g.get_just_pressed().next().is_some()).map(|c| c.0);
    let actual = m.activo.filter(|a| candidatos.iter().any(|c| c.0 == *a));
    let elegido = match (actual, pulsado) {
        (Some(a), Some(p)) if a != p && m.armado => Some(p),
        (Some(a), _) => Some(a),
        (None, p) => p.or_else(|| candidatos.first().map(|c| c.0)),
    };
    let Some((ent, g, nombre)) = elegido.and_then(|e| candidatos.iter().find(|c| c.0 == e)).map(|c| (c.0, c.1, c.2.clone())) else {
        if m.activo.is_some() {
            m.activo = None;
            m.nombre = None;
            m.armado = false;
            m.asistente = None;
            game.message("Mando desconectado", 2.0);
        }
        release_all(&mut m, &mut keys);
        m.pitch = 0.0;
        m.yaw = 0.0;
        m.roll = 0.0;
        m.acel = 0.0;
        m.freno = 0.0;
        return;
    };
    let auto = detectar(g, &nombre);
    m.estilo = if m.estilo_cfg == Estilo::Auto { auto } else { m.estilo_cfg };
    if m.activo != Some(ent) {
        release_all(&mut m, &mut keys);
        m.activo = Some(ent);
        m.nombre = Some(nombre.clone());
        m.armado = false;
        m.cal_t = 1.0;
        m.base.clear();
        m.cal_rango.clear();
        m.ruido.clear();
        // perfil guardado por el asistente para este mando
        let txt0 = m.cfg_texto.clone();
        let mut nuevo = Mando::desde_texto(&txt0);
        nuevo.dir_perfiles = m.dir_perfiles.clone();
        let ruta = m.dir_perfiles.join(format!("{}.cfg", slug(&nombre)));
        m.map = nuevo.map;
        m.intercambiar_ab = nuevo.intercambiar_ab;
        m.perfil = None;
        if let Ok(p) = std::fs::read_to_string(&ruta) {
            m.aplicar_perfil(&p);
            m.perfil = Some(ruta.display().to_string());
        }
        let txt = format!("Mando detectado: {nombre} · diseño {} — pulsa cualquier botón para activarlo (F7: configurarlo)", m.estilo.nombre());
        game.message(txt, 4.0);
    }
    // calibración: los primeros 0,6 s se mide la posición de reposo de cada eje
    if m.cal_t > 0.0 {
        m.cal_t -= time.delta_secs();
        let vals: Vec<(GamepadInput, f32)> = g.analog().all_axes_and_values().map(|(i, v)| (*i, v)).collect();
        for (inp, val) in vals {
            m.base.insert(inp, val);
            let r = m.cal_rango.entry(inp).or_insert((val, val));
            r.0 = r.0.min(val);
            r.1 = r.1.max(val);
        }
        if m.cal_t <= 0.0 {
            // ejes que se movieron sin tocar el mando: sensores de movimiento
            let ruidosos: Vec<GamepadInput> = m.cal_rango.iter().filter(|(i, r)| matches!(i, GamepadInput::Axis(_)) && r.1 - r.0 > 0.12).map(|(i, _)| *i).collect();
            if !ruidosos.is_empty() {
                let txt = format!("Mando: se ignoran {} ejes que se mueven solos (sensor de movimiento)", ruidosos.len());
                game.message(txt, 3.0);
            }
            m.ruido = ruidosos.into_iter().collect();
        }
        return;
    }
    // asistente de configuración (F7)
    if keys.just_pressed(KeyCode::F7) {
        if m.asistente.is_some() {
            m.asistente = None;
            game.message("Asistente del mando cancelado", 2.0);
        } else {
            release_all(&mut m, &mut keys);
            m.asistente = Some(Asistente { paso: 0, espera_suelto: true, res: Vec::new() });
        }
    }
    if m.asistente.is_some() {
        asistente(&mut m, g, &nombre, &mut keys, &mut game);
        return;
    }
    // el mando no actúa hasta pulsar un botón en él (evita que un mando mal reconocido
    // o descalibrado mueva la nave sola al conectarlo)
    if !m.armado {
        if g.get_just_pressed().next().is_some() {
            m.armado = true;
            let txt = match &m.perfil {
                Some(_) => format!("Mando ACTIVO: {nombre} (perfil propio)"),
                None => format!("Mando ACTIVO: {nombre}"),
            };
            game.message(txt, 2.5);
        }
        m.pitch = 0.0;
        m.yaw = 0.0;
        m.roll = 0.0;
        m.acel = 0.0;
        m.freno = 0.0;
        return;
    }

    // ---- analógico
    let inv = if invert.is_some_and(|i| i.0) { -1.0 } else { 1.0 };
    let v = |m: &Mando, a| m.analog(m.valor(g, a));
    m.pitch = (v(&m, Accion::CabecearArriba) - v(&m, Accion::CabecearAbajo)) * inv;
    m.roll = v(&m, Accion::AlabearIzquierda) - v(&m, Accion::AlabearDerecha);
    m.yaw = v(&m, Accion::GuinarIzquierda) - v(&m, Accion::GuinarDerecha);
    m.acel = v(&m, Accion::Acelerar);
    m.freno = v(&m, Accion::Frenar);

    // ---- digital → teclas virtuales (con histéresis en los ejes)
    let umbral = m.umbral;
    let mut want: HashSet<KeyCode> = HashSet::new();
    let mut on: HashMap<Accion, bool> = HashMap::new();
    for (a, _, key, _) in ACCIONES {
        let val = m.valor(g, *a);
        let prev = m.estado.get(a).copied().unwrap_or(false);
        let now = if prev { val > umbral * 0.6 } else { val > umbral };
        on.insert(*a, now);
        if now {
            if let Some(k) = key {
                want.insert(*k);
            }
        }
    }
    m.estado = on;
    // contexto: título, pausa y derribado
    let in_title = !game.started || intro.as_ref().is_some_and(|i| i.phase != crate::models::IntroPhase::Off);
    use GamepadButton::*;
    let b = |x: GamepadButton| g.pressed(m.fisico(x));
    if in_title {
        want.clear();
        if b(Start) || b(South) || b(East) {
            want.insert(KeyCode::Enter);
        }
        if b(DPadLeft) || m.valor_entrada(g, &Entrada::Eje(GamepadAxis::LeftStickX, -1.0)) > 0.6 {
            want.insert(KeyCode::ArrowLeft);
        }
        if b(DPadRight) || m.valor_entrada(g, &Entrada::Eje(GamepadAxis::LeftStickX, 1.0)) > 0.6 {
            want.insert(KeyCode::ArrowRight);
        }
        if b(DPadUp) || b(Select) {
            want.insert(KeyCode::KeyH);
        }
    } else if game.paused {
        if b(South) {
            want.insert(KeyCode::Enter);
        }
    } else if game.dead && (b(Start) || b(South)) {
        want.remove(&KeyCode::Escape);
        want.insert(KeyCode::KeyR);
    }
    let prev = std::mem::take(&mut m.teclas);
    for k in want.difference(&prev) {
        keys.press(*k);
    }
    for k in prev.difference(&want) {
        keys.release(*k);
    }
    m.teclas = want;

    // ---- modo prueba: nombre de lo que se pulsa
    if m.prueba {
        let all = [South, East, North, West, LeftTrigger, RightTrigger, LeftTrigger2, RightTrigger2, Select, Start, Mode, LeftThumb, RightThumb, DPadUp, DPadDown, DPadLeft, DPadRight];
        for x in all {
            if g.just_pressed(x) {
                let accs: Vec<&str> = ACCIONES.iter().filter(|(a, ..)| m.entradas(*a).contains(&Entrada::Boton(x))).map(|x| x.1).collect();
                let txt = format!("MANDO: {:?} ({}) → {}", x, m.etiqueta(x), if accs.is_empty() { "sin asignar".into() } else { accs.join(", ") });
                game.message(txt, 2.5);
            }
        }
        for b in g.get_just_pressed() {
            if let GamepadButton::Other(n) = b {
                game.message(format!("MANDO: botón no estándar Otro{n}"), 2.5);
            }
        }
        if let Some((e, d)) = m.desviaciones(g).into_iter().filter(|(e, _)| matches!(e, Entrada::Eje(..))).max_by(|a, b| a.1.total_cmp(&b.1)) {
            if d > 0.5 {
                game.message(format!("MANDO: eje {} ({:.0}%)", nombre_entrada(&e), d * 100.0), 0.5);
            }
        }
    }

    // ---- vibración
    let k = m.vibracion;
    if k > 0.0 {
        let mut add = |strong: f32, weak: f32, ms: u64| {
            rumble.write(GamepadRumbleRequest::Add {
                gamepad: ent,
                duration: Duration::from_millis(ms),
                intensity: GamepadRumbleIntensity { strong_motor: (strong * k).clamp(0.0, 1.0), weak_motor: (weak * k).clamp(0.0, 1.0) },
            });
        };
        if game.shake > m.ult_shake + 0.05 {
            add((game.shake * 0.8).min(1.0), 0.5, 260);
        }
        if game.hit_flash > m.ult_flash + 0.05 {
            add(0.45, 0.9, 160);
        }
        if game.missiles < m.ult_misiles {
            add(0.0, 0.45, 140);
        }
        if game.cluster < m.ult_racimo {
            add(0.25, 0.7, 600);
        }
    }
    m.ult_shake = game.shake;
    m.ult_flash = game.hit_flash;
    m.ult_misiles = game.missiles;
    m.ult_racimo = game.cluster;
}

fn release_all(m: &mut Mando, keys: &mut ButtonInput<KeyCode>) {
    for k in m.teclas.drain() {
        keys.release(k);
    }
    m.estado.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::gamepad::{GamepadConnection, GamepadConnectionEvent, RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent, RawGamepadEvent};
    use bevy::input::{InputPlugin, InputSystems};

    #[test]
    fn cfg_por_defecto() {
        let m = Mando::desde_texto(CFG_DEFECTO);
        assert_eq!(m.estilo_cfg, Estilo::Auto);
        assert!(!m.intercambiar_ab);
        assert_eq!(m.entradas(Accion::Disparar), &[Entrada::Boton(GamepadButton::South)]);
        assert_eq!(m.entradas(Accion::CabecearArriba), &[Entrada::Eje(GamepadAxis::LeftStickY, 1.0)]);
        assert_eq!(m.entradas(Accion::Camara).len(), 2);
        assert!((m.umbral - 0.5).abs() < 1e-6);
    }

    #[test]
    fn etiquetas_por_diseno() {
        let mut m = Mando::desde_texto(CFG_DEFECTO);
        m.estilo = Estilo::PlayStation;
        assert_eq!(m.etiquetas(Accion::Disparar), "✕");
        assert_eq!(m.etiquetas(Accion::Acelerar), "R2");
        m.estilo = Estilo::Nintendo;
        assert_eq!(m.etiquetas(Accion::Disparar), "B");
        assert_eq!(m.etiquetas(Accion::Misil), "A");
        assert_eq!(m.etiquetas(Accion::Acelerar), "ZR");
        assert_eq!(m.etiquetas(Accion::Pausa), "+");
    }

    fn app(name: &str, vendor: u16, cfg: &str) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin));
        let mut game = Game::default();
        game.started = true;
        app.insert_resource(game);
        app.insert_resource(Mando::desde_texto(cfg));
        app.add_systems(PreUpdate, mando_input.after(InputSystems));
        let g = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(GamepadConnectionEvent::new(
            g,
            GamepadConnection::Connected { name: name.into(), vendor_id: Some(vendor), product_id: Some(1) },
        ));
        app.update();
        app.update();
        {
            let mut m = app.world_mut().resource_mut::<Mando>();
            m.cal_t = 0.0;
            m.armado = true;
        }
        (app, g)
    }

    #[test]
    fn dualsense_dispara_y_vuela() {
        let (mut app, g) = app("DualSense Wireless Controller", 0x054c, CFG_DEFECTO);
        assert_eq!(app.world().resource::<Mando>().estilo, Estilo::PlayStation);
        app.world_mut().write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(g, GamepadButton::South, 1.0)));
        app.world_mut().write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(g, GamepadAxis::LeftStickY, 0.9)));
        app.world_mut().write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(g, GamepadButton::RightTrigger2, 0.8)));
        app.update();
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(keys.pressed(KeyCode::Space), "✕ debe disparar");
        let m = app.world().resource::<Mando>();
        assert!(m.pitch > 0.5, "stick arriba = cabeceo {}", m.pitch);
        assert!(m.acel > 0.4, "R2 = acelerar {}", m.acel);
        // soltar
        app.world_mut().write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(g, GamepadButton::South, 0.0)));
        app.update();
        assert!(!app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::Space));
    }

    #[test]
    fn nintendo_con_intercambio() {
        let cfg = CFG_DEFECTO.replace("intercambiar_ab   = no", "intercambiar_ab   = sí");
        let (mut app, g) = app("Nintendo Switch Pro Controller", 0x057e, &cfg);
        assert_eq!(app.world().resource::<Mando>().estilo, Estilo::Nintendo);
        // con intercambio, el botón que el driver llama East hace lo de South (disparar)
        app.world_mut().write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(g, GamepadButton::East, 1.0)));
        app.world_mut().write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(g, GamepadButton::West, 1.0)));
        app.update();
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(keys.pressed(KeyCode::Space));
        assert!(keys.pressed(KeyCode::KeyG), "West intercambiado = transformar");
        assert!(!keys.pressed(KeyCode::KeyF));
    }

    #[test]
    fn sensor_imu_ignorado_y_no_se_mueve_solo() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin));
        let mut game = Game::default();
        game.started = true;
        app.insert_resource(game);
        app.insert_resource(Mando::desde_texto(CFG_DEFECTO));
        app.add_systems(PreUpdate, mando_input.after(InputSystems));
        // el sensor de movimiento del Switch Pro aparece como otro "mando"
        let imu = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(GamepadConnectionEvent::new(
            imu,
            GamepadConnection::Connected { name: "Nintendo Switch Pro Controller (IMU)".into(), vendor_id: Some(0x057e), product_id: Some(0x2009) },
        ));
        let g = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(GamepadConnectionEvent::new(
            g,
            GamepadConnection::Connected { name: "Nintendo Switch Pro Controller".into(), vendor_id: Some(0x057e), product_id: Some(0x2009) },
        ));
        app.update();
        // stick izquierdo descentrado en reposo (0.4) y gatillo que reposa en -1
        app.world_mut().write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(g, GamepadAxis::LeftStickY, 0.4)));
        app.world_mut().write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(g, GamepadAxis::RightZ, -1.0)));
        // el IMU no para de moverse
        app.world_mut().write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(imu, GamepadAxis::LeftStickX, 0.9)));
        app.update();
        {
            let m = app.world().resource::<Mando>();
            assert_eq!(m.activo, Some(g), "debe elegir el mando, no el sensor");
        }
        // terminar la calibración midiendo el reposo
        for _ in 0..3 {
            app.update();
        }
        {
            let mut m = app.world_mut().resource_mut::<Mando>();
            m.cal_t = 0.0;
        }
        app.update();
        let m = app.world().resource::<Mando>();
        assert!(!m.armado, "no se activa hasta pulsar un botón");
        assert_eq!(m.pitch, 0.0);
        // pulsar un botón lo activa y el reposo descentrado no mueve la nave
        app.world_mut().write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(g, GamepadButton::South, 1.0)));
        app.update();
        app.update();
        let m = app.world().resource::<Mando>();
        assert!(m.armado);
        assert!(m.pitch.abs() < 0.05, "stick en reposo no debe cabecear: {}", m.pitch);
    }

    #[test]
    fn asistente_aprende_botones_no_estandar() {
        let (mut app, g) = app("Generic USB Joystick", 0x0079, CFG_DEFECTO);
        let dir = std::env::temp_dir().join("mandos_prueba");
        let _ = std::fs::remove_dir_all(&dir);
        {
            let mut m = app.world_mut().resource_mut::<Mando>();
            m.dir_perfiles = dir.clone();
            m.asistente = Some(Asistente { paso: 0, espera_suelto: true, res: Vec::new() });
        }
        app.update();
        // paso 1 (disparar): un botón no estándar
        app.world_mut().write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(g, GamepadButton::Other(5), 1.0)));
        app.update();
        app.world_mut().write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(g, GamepadButton::Other(5), 0.0)));
        app.update();
        app.update();
        // paso 2 (misil): un eje no estándar hacia abajo
        app.world_mut().write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(g, GamepadAxis::Other(3), -0.9)));
        app.update();
        app.world_mut().write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(g, GamepadAxis::Other(3), 0.0)));
        app.update();
        let m = app.world().resource::<Mando>();
        let a = m.asistente.as_ref().unwrap();
        assert_eq!(a.res[0], (Accion::Disparar, Entrada::Boton(GamepadButton::Other(5))));
        assert_eq!(a.res[1], (Accion::Misil, Entrada::Eje(GamepadAxis::Other(3), -1.0)));
        // saltar el resto y guardar
        {
            let mut m = app.world_mut().resource_mut::<Mando>();
            let mut a = m.asistente.clone().unwrap();
            a.paso = PASOS.len();
            m.asistente = Some(a);
        }
        app.update();
        let m = app.world().resource::<Mando>();
        assert!(m.asistente.is_none());
        assert_eq!(m.entradas(Accion::Disparar), &[Entrada::Boton(GamepadButton::Other(5))]);
        let txt = std::fs::read_to_string(dir.join("generic_usb_joystick.cfg")).unwrap();
        assert!(txt.contains("disparar          = Otro5"), "{txt}");
        assert!(txt.contains("Eje3-"));
        // y al releerlo se interpreta igual
        let mut m2 = Mando::desde_texto(CFG_DEFECTO);
        m2.aplicar_perfil(&txt);
        assert_eq!(m2.entradas(Accion::Misil), &[Entrada::Eje(GamepadAxis::Other(3), -1.0)]);
    }

    #[test]
    fn eje_que_se_mueve_solo_se_ignora() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin));
        let mut game = Game::default();
        game.started = true;
        app.insert_resource(game);
        app.insert_resource(Mando::desde_texto(CFG_DEFECTO));
        app.add_systems(PreUpdate, mando_input.after(InputSystems));
        let g = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(GamepadConnectionEvent::new(
            g,
            GamepadConnection::Connected { name: "Pro Controller".into(), vendor_id: Some(0x057e), product_id: Some(0x2009) },
        ));
        app.update();
        // el giroscopio llega como el stick derecho y no para de moverse
        for k in 0..6 {
            let v = if k % 2 == 0 { 0.5 } else { -0.4 };
            app.world_mut().write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(g, GamepadAxis::RightStickX, v)));
            app.update();
        }
        app.world_mut().resource_mut::<Mando>().cal_t = 0.0001;
        std::thread::sleep(std::time::Duration::from_millis(2));
        app.update();
        app.world_mut().write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(g, GamepadButton::South, 1.0)));
        app.update();
        app.world_mut().write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(g, GamepadAxis::RightStickX, 0.9)));
        app.update();
        let m = app.world().resource::<Mando>();
        assert!(m.ruido.contains(&GamepadInput::Axis(GamepadAxis::RightStickX)));
        assert!(m.armado);
        assert_eq!(m.yaw, 0.0, "el eje inestable no debe girar la nave");
    }

    #[test]
    fn titulo_start_empieza() {
        let (mut app, g) = app("Xbox Wireless Controller", 0x045e, CFG_DEFECTO);
        app.world_mut().resource_mut::<Game>().started = false;
        app.world_mut().write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(g, GamepadButton::Start, 1.0)));
        app.update();
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(keys.just_pressed(KeyCode::Enter));
        assert!(!keys.pressed(KeyCode::Escape));
    }
}

fn asistente(m: &mut Mando, g: &Gamepad, nombre: &str, keys: &mut ButtonInput<KeyCode>, game: &mut Game) {
    let Some(mut a) = m.asistente.clone() else { return };
    m.pitch = 0.0;
    m.yaw = 0.0;
    m.roll = 0.0;
    m.acel = 0.0;
    m.freno = 0.0;
    let dev = m.desviaciones(g);
    if keys.just_pressed(KeyCode::Backspace) {
        a.paso += 1;
        a.espera_suelto = true;
    }
    if a.paso < PASOS.len() {
        let (acc, txt) = PASOS[a.paso];
        if a.espera_suelto {
            // esperar a que se suelte todo
            if dev.iter().all(|(_, d)| *d < 0.3) {
                a.espera_suelto = false;
            }
            game.message(format!("ASISTENTE DEL MANDO ({}/{}): suelta todo…", a.paso + 1, PASOS.len()), 0.3);
        } else {
            game.message(format!("ASISTENTE DEL MANDO ({}/{}): {txt}   (Retroceso salta · F7 cancela)", a.paso + 1, PASOS.len()), 0.3);
            let usados: Vec<Entrada> = a.res.iter().map(|r| r.1).collect();
            if let Some((e, _)) = dev.iter().filter(|(e, d)| *d > 0.6 && !usados.contains(e)).max_by(|x, y| x.1.total_cmp(&y.1)) {
                a.res.push((acc, *e));
                a.paso += 1;
                a.espera_suelto = true;
            }
        }
        m.asistente = Some(a);
        return;
    }
    // fin: guardar el perfil y aplicarlo
    let mut txt = format!("# Perfil del mando \"{nombre}\" creado con el asistente (F7).\n# Se carga solo cuando se conecta este mando. Bórralo para volver a mando.cfg.\n\n");
    for (acc, e) in &a.res {
        if let Some((_, k, _, _)) = ACCIONES.iter().find(|x| x.0 == *acc) {
            txt.push_str(&format!("{k:<18}= {}\n", nombre_entrada(e)));
        }
    }
    txt.push_str("intercambiar_ab   = no\n");
    let ruta = m.dir_perfiles.join(format!("{}.cfg", slug(nombre)));
    let ok = std::fs::create_dir_all(&m.dir_perfiles).and_then(|_| std::fs::write(&ruta, &txt)).is_ok();
    m.aplicar_perfil(&txt);
    m.intercambiar_ab = false;
    m.perfil = Some(ruta.display().to_string());
    m.asistente = None;
    m.armado = true;
    let msg = if ok { format!("Perfil del mando guardado ({} controles) en {}", a.res.len(), ruta.display()) } else { "Perfil aplicado (no se pudo guardar el archivo)".to_string() };
    game.message(msg, 4.0);
}

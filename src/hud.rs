//! Capa de pantalla cinematográfica: franjas negras (letterbox), subtítulos,
//! tarjeta de título, destello de daño y fundidos. El resto de la información
//! vive dentro de la cabina (pantallas y HUD vectorial).

use bevy::prelude::*;

use crate::components::*;
use crate::world::*;

const SUB: Color = Color::srgb(1.0, 0.96, 0.75);
const WARN: Color = Color::srgb(1.0, 0.30, 0.25);
const DIMW: Color = Color::srgba(1.0, 1.0, 1.0, 0.65);

/// Ayuda de controles de teclado y ratón (la del mando se añade debajo).
pub const AYUDA_TECLADO: &str = "RATÓN  cabeceo / guiñada     A / D  alabeo\n\
     W / S  acelerador     SHIFT  turbo\n\
     CLIC IZQ / ESPACIO  láseres\n\
     CLIC DER / F  misil (mantén al enemigo\n   en la mira para fijarlo)\n\
     V / BOTÓN CENTRAL  misiles en racimo (12)\n\
     G  transformar: CAZA / GERWALK / BATTROID\n\
     (cerca del suelo o en el espacio)\n\
     C  siguiente cámara   1-6  elegir cámara\n\
     X  cortes cinematográficos sí/no\n\
     I  invertir eje Y     H  ocultar ayuda\n\
     ESC  pausa     R  reiniciar     M  música\n\
     F7  configurar mando   F6  probarlo   F8  diseño\n\
     Sube sobre 3400 m → ÓRBITA\n\
     Baja hacia la Tierra → REENTRADA";

#[derive(Component, Clone, Copy, PartialEq)]
pub enum HudText {
    View,
    Score,
    Msg,
    Warning,
    RadioWho,
    RadioText,
    Objective,
    BannerTitle,
    BannerSub,
    Title,
    Help,
}

#[derive(Component)]
pub struct FadeOverlay;
#[derive(Component)]
pub struct DamageOverlay;
#[derive(Component)]
pub struct TitleOverlay;
#[derive(Component)]
pub struct HelpPanel;
#[derive(Component)]
pub struct ObjectivePanel;
#[derive(Component)]
pub struct BannerNode;

fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

pub fn setup_hud(mut commands: Commands) {
    // destello de daño (debajo de las franjas)
    commands.spawn((
        Node { width: percent(100), height: percent(100), position_type: PositionType::Absolute, ..default() },
        BackgroundColor(Color::srgba(1.0, 0.0, 0.0, 0.0)),
        DamageOverlay,
    ));
    // franja superior
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(0),
                width: percent(100),
                height: percent(9),
                padding: UiRect::horizontal(px(28)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK),
        ))
        .with_children(|p| {
            p.spawn((Text::new(""), font(15.0), TextColor(DIMW), HudText::View));
            p.spawn((Text::new(""), font(15.0), TextColor(DIMW), HudText::Score));
        });
    // franja inferior con subtítulos
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: px(0),
                width: percent(100),
                height: percent(9),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(2),
                ..default()
            },
            BackgroundColor(Color::BLACK),
        ))
        .with_children(|p| {
            p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(10), ..default() }).with_children(|r| {
                r.spawn((Text::new(""), font(19.0), TextColor(SUB), HudText::RadioWho));
                r.spawn((Text::new(""), font(19.0), TextColor(Color::WHITE), HudText::RadioText));
            });
            p.spawn((Text::new(""), font(16.0), TextColor(SUB), HudText::Msg));
            p.spawn((Text::new(""), font(15.0), TextColor(WARN), HudText::Warning));
        });
    // ayuda de controles
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(24),
                top: percent(11),
                padding: UiRect::all(px(12)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            HelpPanel,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(AYUDA_TECLADO),
                font(14.0),
                TextColor(Color::srgb(0.6, 1.0, 0.7)),
                HudText::Help,
            ));
        });
    // panel de objetivo de la misión (arriba a la derecha)
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(24),
                top: percent(11),
                max_width: px(560),
                padding: UiRect::all(px(10)),
                border: UiRect::left(px(3)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            BorderColor::all(Color::srgb(1.0, 0.75, 0.3)),
            ObjectivePanel,
        ))
        .with_children(|p| {
            p.spawn((Text::new(""), font(15.0), TextColor(Color::srgb(1.0, 0.92, 0.75)), HudText::Objective));
        });
    // cartel de misión (centro)
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: percent(24),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(6),
                ..default()
            },
            BannerNode,
        ))
        .with_children(|p| {
            p.spawn((Text::new(""), font(40.0), TextColor(Color::srgb(1.0, 0.85, 0.4)), HudText::BannerTitle));
            p.spawn((Text::new(""), font(20.0), TextColor(Color::WHITE), HudText::BannerSub));
        });
    // fundido de transición
    commands.spawn((
        Node { width: percent(100), height: percent(100), position_type: PositionType::Absolute, ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 1.0)),
        FadeOverlay,
    ));
    // tarjeta de título
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                padding: UiRect::bottom(percent(11)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.02, 0.10, 0.12)),
            TitleOverlay,
        ))
        .with_children(|p| {
            p.spawn((Text::new(""), font(46.0), TextColor(SUB), HudText::Title, TextLayout::justify(Justify::Center)));
        });
}

pub fn hud_text(
    mando: Res<crate::mando::Mando>,
    demo: Option<Res<crate::demo::Demo>>,
    mecha: Res<crate::mecha::Mecha>,
    radio: Res<crate::missions::Radio>,
    mission: Res<crate::missions::Mission>,
    diff: Res<crate::missions::Difficulty>,
    intro: Option<Res<crate::models::Intro>>,
    game: Res<Game>,
    dir: Res<Director>,
    player: Single<&Transform, With<Player>>,
    time: Res<Time<Real>>,
    mut q: Query<(&mut Text, &HudText, &mut TextColor)>,
) {
    let p = player.translation;
    let f = player.forward();
    let blink = (time.elapsed_secs() * 4.0).sin() > 0.0;
    let in_intro = intro.as_ref().is_some_and(|i| i.phase != crate::models::IntroPhase::Off);
    let scramble = intro.as_ref().is_some_and(|i| i.phase == crate::models::IntroPhase::Scramble);
    for (mut t, kind, mut tc) in &mut q {
        if in_intro && matches!(kind, HudText::View | HudText::Score | HudText::Msg | HudText::Warning | HudText::RadioWho | HudText::RadioText | HudText::Objective) {
            if !t.0.is_empty() {
                t.0.clear();
            }
            continue;
        }
        let s: String = match kind {
            HudText::View => {
                let auto = if dir.auto { "  ·  CORTES AUTO" } else { "" };
                let zone = match game.zone {
                    Zone::Atmosphere => "ATMÓSFERA",
                    Zone::Space => "ÓRBITA",
                };
                let p = player.translation;
                let can = crate::mecha::allowed(game.zone, p).len() > 1;
                let form = if mecha.morph > 0.0 { "TRANSFORMANDO…".to_string() } else { mecha.mode.name().to_string() };
                format!("CÁMARA: {}{}   ·   {}   ·   FORMA: {}{}", dir.current().name(), auto, zone, form, if can { "  (G)" } else { "" })
            }
            HudText::Score => format!(
                "{}   ·   NODRIZA {:.0}%   ·   DERRIBOS {}   ·   PUNTOS {:06}",
                diff.name(),
                (game.carrier_hp / CARRIER_HP * 100.0).floor().max(0.0),
                game.kills,
                game.score
            ),
            HudText::Msg => {
                if game.msg_timer > 0.0 {
                    game.msg.clone()
                } else {
                    String::new()
                }
            }
            HudText::Warning => {
                let mut w = String::new();
                if !game.dead {
                    if game.zone == Zone::Atmosphere {
                        let agl = p.y - ground_height(p.x, p.z);
                        if agl < 250.0 && f.y < -0.1 {
                            w = "¡ARRIBA! ¡SUBE!".into();
                        } else if p.y > CEILING - 600.0 {
                            w = "Aproximándose al límite de la atmósfera".into();
                        }
                    }
                    if game.hull < 30.0 {
                        w = "¡DAÑO CRÍTICO EN EL CASCO!".into();
                    }
                    if game.overheated {
                        w = "LÁSERES SOBRECALENTADOS".into();
                    }
                    if game.jam > 0.0 {
                        w = "INTERFERENCIA: RADAR INOPERATIVO".into();
                    }
                    if game.plasma_warn > 0.0 {
                        w = format!("¡PLASMA EN COLA! {:.0} m — TURBO + VIRAJE", game.plasma_warn);
                    }
                }
                if blink { w } else { String::new() }
            }
            HudText::Title => {
                if scramble {
                    String::new()
                } else if !game.started && demo.is_some() {
                    "ESCUADRÓN ORBITAL".to_string()
                } else if !game.started {
                    let pad = if mando.nombre.is_some() { format!("\n(mando: {} para despegar · cruceta ← → dificultad)", mando.etiqueta(bevy::input::gamepad::GamepadButton::Start)) } else { String::new() };
                    format!(
                        "ESCUADRÓN ORBITAL\n\
                         Dificultad:  ◄ {} ►   (flechas ← →)\n\
                         Haz clic o pulsa ENTER para despegar{pad}",
                        diff.name()
                    )
                } else if game.paused {
                    let pad = if mando.nombre.is_some() { format!("\n(mando: {} o {})", mando.etiqueta(bevy::input::gamepad::GamepadButton::Start), mando.etiqueta(bevy::input::gamepad::GamepadButton::South)) } else { String::new() };
                    format!("PAUSA\n\nHaz clic o pulsa ENTER para continuar{pad}")
                } else {
                    String::new()
                }
            }
            HudText::RadioWho => match &radio.current {
                Some((who, _, _)) => {
                    tc.0 = who.color();
                    format!("{}:", who.label())
                }
                None => String::new(),
            },
            HudText::RadioText => radio.current.as_ref().map(|c| c.1.clone()).unwrap_or_default(),
            HudText::Objective => {
                if mission.objective.is_empty() {
                    String::new()
                } else {
                    let head = if mission.phase == crate::missions::Phase::Free {
                        "PATRULLA LIBRE".to_string()
                    } else {
                        format!("MISIÓN {}/5 · {}", mission.idx + 1, crate::missions::MISSIONS[mission.idx.min(4)])
                    };
                    format!("{head}\n▸ {}", mission.objective)
                }
            }
            HudText::BannerTitle => {
                if mission.banner > 0.0 { mission.banner_title.clone() } else { String::new() }
            }
            HudText::BannerSub => {
                if mission.banner > 0.0 { mission.banner_sub.clone() } else { String::new() }
            }
            HudText::Help => format!("{AYUDA_TECLADO}{}", mando.ayuda()),
        };
        if t.0 != s {
            t.0 = s;
        }
    }
}

pub fn hud_overlays(
    game: Res<Game>,
    mut fade: Single<&mut BackgroundColor, (With<FadeOverlay>, Without<DamageOverlay>)>,
    mut dmg: Single<&mut BackgroundColor, (With<DamageOverlay>, Without<FadeOverlay>)>,
    mut title: Single<&mut Visibility, (With<TitleOverlay>, Without<HelpPanel>)>,
    mut help: Single<&mut Visibility, (With<HelpPanel>, Without<TitleOverlay>)>,
    mission: Res<crate::missions::Mission>,
    mut obj: Single<&mut Visibility, (With<ObjectivePanel>, Without<HelpPanel>, Without<TitleOverlay>)>,
) {
    **obj = if mission.objective.is_empty() || !game.started { Visibility::Hidden } else { Visibility::Inherited };
    let white = game.pending_zone.is_some() && !game.reset_on_switch;
    fade.0 = if white { Color::srgba(1.0, 0.92, 0.95, game.fade) } else { Color::srgba(0.0, 0.0, 0.0, game.fade) };
    dmg.0 = Color::srgba(0.9, 0.05, 0.0, game.hit_flash * 0.45);
    **title = if !game.started || game.paused { Visibility::Inherited } else { Visibility::Hidden };
    // la ayuda se oculta sola cuando empieza la primera misión activa
    **help = if game.show_help && (!game.started || mission.idx == 0 && mission.phase != crate::missions::Phase::Active) { Visibility::Inherited } else { Visibility::Hidden };
}

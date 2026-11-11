//! Mandos a través de SDL2 (se compila con `--features sdl`; start.sh lo activa en macOS).
//!
//! En macOS el lector de mandos que trae Bevy (gilrs) no sabe hablar con el mando Pro de
//! Switch ni con los mandos chinos "compatibles con Nintendo": por Bluetooth no reaccionan y
//! por cable mandan datos sin sentido. SDL2 sí tiene controladores para ellos (Switch Pro,
//! Joy-Con, Xbox, DualSense...), separa el giroscopio y entrega los botones por posición.
//! Aquí se traducen sus eventos a los eventos de mando de Bevy, así el resto del juego no
//! cambia.

use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    #[cfg(feature = "sdl")]
    app.add_systems(PreUpdate, imp::sdl_poll.before(bevy::input::InputSystems));
    let _ = app;
}

/// ¿Se usan los mandos por SDL2 en lugar de gilrs?
pub fn activo() -> bool {
    cfg!(feature = "sdl") && std::env::var("NAVE_GILRS").is_err()
}

#[cfg(feature = "sdl")]
pub mod imp {
    use std::collections::HashMap;
    use std::time::Duration;

    use bevy::ecs::message::{MessageCursor, Messages};
    use bevy::input::gamepad::{
        GamepadAxis, GamepadButton, GamepadConnection, GamepadConnectionEvent, GamepadRumbleRequest, RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent,
        RawGamepadEvent,
    };
    use bevy::prelude::*;
    use sdl2::controller::{Axis, Button, GameController};
    use sdl2::event::Event;

    pub struct SdlPad {
        _sdl: sdl2::Sdl,
        gc: sdl2::GameControllerSubsystem,
        pump: sdl2::EventPump,
        open: HashMap<u32, (GameController, Entity)>,
        cursor: MessageCursor<GamepadRumbleRequest>,
    }

    fn iniciar() -> Result<SdlPad, String> {
        // botones por POSICIÓN (A = abajo) también en los mandos de Nintendo
        sdl2::hint::set("SDL_GAMECONTROLLER_USE_BUTTON_LABELS", "0");
        // la ventana es de Bevy, no de SDL: sin esto SDL cree que el juego está en segundo
        // plano y no entrega eventos
        sdl2::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
        sdl2::hint::set("SDL_JOYSTICK_HIDAPI", "1");
        sdl2::hint::set("SDL_JOYSTICK_HIDAPI_SWITCH", "1");
        sdl2::hint::set("SDL_JOYSTICK_HIDAPI_PS5", "1");
        sdl2::hint::set("SDL_JOYSTICK_HIDAPI_SWITCH_HOME_LED", "0");
        let sdl = sdl2::init()?;
        let gc = sdl.game_controller()?;
        let pump = sdl.event_pump()?;
        Ok(SdlPad { _sdl: sdl, gc, pump, open: HashMap::new(), cursor: MessageCursor::default() })
    }

    fn boton(b: Button) -> GamepadButton {
        use GamepadButton as G;
        match b {
            Button::A => G::South,
            Button::B => G::East,
            Button::X => G::West,
            Button::Y => G::North,
            Button::Back => G::Select,
            Button::Guide => G::Mode,
            Button::Start => G::Start,
            Button::LeftStick => G::LeftThumb,
            Button::RightStick => G::RightThumb,
            Button::LeftShoulder => G::LeftTrigger,
            Button::RightShoulder => G::RightTrigger,
            Button::DPadUp => G::DPadUp,
            Button::DPadDown => G::DPadDown,
            Button::DPadLeft => G::DPadLeft,
            Button::DPadRight => G::DPadRight,
            Button::Misc1 => G::Other(15),
            Button::Paddle1 => G::Other(16),
            Button::Paddle2 => G::Other(17),
            Button::Paddle3 => G::Other(18),
            Button::Paddle4 => G::Other(19),
            Button::Touchpad => G::Other(20),
        }
    }

    /// Sondea SDL (en el hilo principal) y escribe los eventos de mando de Bevy.
    pub fn sdl_poll(world: &mut World, mut estado: Local<u8>) {
        if *estado == 2 {
            return;
        }
        let mut pad = match world.remove_non_send::<SdlPad>() {
            Some(p) => p,
            None if *estado == 0 => match iniciar() {
                Ok(p) => {
                    info!("Mandos: usando SDL2 {}", sdl2::version::version());
                    *estado = 1;
                    p
                }
                Err(e) => {
                    warn!("Mandos: no se pudo iniciar SDL2 ({e}); sin mandos");
                    *estado = 2;
                    return;
                }
            },
            None => return,
        };
        let eventos: Vec<Event> = pad.pump.poll_iter().collect();
        for ev in eventos {
            match ev {
                Event::ControllerDeviceAdded { which, .. } => {
                    if let Ok(c) = pad.gc.open(which) {
                        let id = c.instance_id();
                        if pad.open.contains_key(&id) {
                            continue;
                        }
                        let name = c.name();
                        let (v, p) = (c.vendor_id(), c.product_id());
                        info!("Mando conectado por SDL2: {name} (fabricante {v:04x?}, producto {p:04x?})");
                        let e = world.spawn_empty().id();
                        world.write_message(GamepadConnectionEvent::new(e, GamepadConnection::Connected { name, vendor_id: v, product_id: p }));
                        pad.open.insert(id, (c, e));
                    }
                }
                Event::ControllerDeviceRemoved { which, .. } => {
                    if let Some((_, e)) = pad.open.remove(&which) {
                        world.write_message(GamepadConnectionEvent::new(e, GamepadConnection::Disconnected));
                    }
                }
                Event::ControllerButtonDown { which, button, .. } | Event::ControllerButtonUp { which, button, .. } => {
                    if let Some((_, e)) = pad.open.get(&which) {
                        let down = matches!(ev, Event::ControllerButtonDown { .. });
                        world.write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(*e, boton(button), if down { 1.0 } else { 0.0 })));
                    }
                }
                Event::ControllerAxisMotion { which, axis, value, .. } => {
                    if let Some((_, e)) = pad.open.get(&which) {
                        let v = (value as f32 / 32767.0).clamp(-1.0, 1.0);
                        let ev = match axis {
                            Axis::LeftX => RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(*e, GamepadAxis::LeftStickX, v)),
                            // SDL: Y positivo hacia abajo; Bevy: hacia arriba
                            Axis::LeftY => RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(*e, GamepadAxis::LeftStickY, -v)),
                            Axis::RightX => RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(*e, GamepadAxis::RightStickX, v)),
                            Axis::RightY => RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(*e, GamepadAxis::RightStickY, -v)),
                            Axis::TriggerLeft => RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(*e, GamepadButton::LeftTrigger2, v.max(0.0))),
                            Axis::TriggerRight => RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(*e, GamepadButton::RightTrigger2, v.max(0.0))),
                        };
                        world.write_message(ev);
                    }
                }
                _ => {}
            }
        }
        // vibración pedida por el juego
        let pedidas: Vec<GamepadRumbleRequest> = {
            let msgs = world.resource::<Messages<GamepadRumbleRequest>>();
            pad.cursor.read(msgs).cloned().collect()
        };
        for r in pedidas {
            if let GamepadRumbleRequest::Add { gamepad, duration, intensity } = r {
                if let Some((c, _)) = pad.open.values_mut().find(|(_, e)| *e == gamepad) {
                    let lo = (intensity.strong_motor.clamp(0.0, 1.0) * 65535.0) as u16;
                    let hi = (intensity.weak_motor.clamp(0.0, 1.0) * 65535.0) as u16;
                    let _ = c.set_rumble(lo, hi, duration.min(Duration::from_secs(5)).as_millis() as u32);
                }
            }
        }
        world.insert_non_send(pad);
    }
}

#[cfg(all(test, feature = "sdl"))]
mod tests {
    use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton};
    use bevy::input::{InputPlugin, InputSystems};
    use bevy::prelude::*;

    /// Un mando virtual de SDL llega al juego como un mando de Bevy, con los botones por
    /// posición y el eje Y del stick hacia arriba positivo.
    #[test]
    fn mando_virtual_sdl() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin));
        app.add_systems(PreUpdate, super::imp::sdl_poll.before(InputSystems));
        app.update(); // inicia SDL
        let idx = unsafe {
            sdl2::sys::SDL_JoystickAttachVirtual(sdl2::sys::SDL_JoystickType::SDL_JOYSTICK_TYPE_GAMECONTROLLER, 6, 15, 0)
        };
        assert!(idx >= 0, "no se pudo crear el mando virtual");
        for _ in 0..3 {
            app.update();
        }
        let joy = unsafe { sdl2::sys::SDL_JoystickOpen(idx) };
        assert!(!joy.is_null());
        unsafe {
            sdl2::sys::SDL_JoystickSetVirtualButton(joy, 0, 1); // A
            sdl2::sys::SDL_JoystickSetVirtualAxis(joy, 1, -30000); // stick izq. hacia arriba
        }
        for _ in 0..3 {
            app.update();
        }
        let mut q = app.world_mut().query::<&Gamepad>();
        let pads: Vec<&Gamepad> = q.iter(app.world()).collect();
        assert_eq!(pads.len(), 1, "debe haber un mando conectado");
        let g = pads[0];
        assert!(g.pressed(GamepadButton::South), "A de SDL = South");
        assert!(g.get(GamepadAxis::LeftStickY).unwrap_or(0.0) > 0.8, "arriba = positivo: {:?}", g.get(GamepadAxis::LeftStickY));
    }
}

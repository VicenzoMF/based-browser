//! Tradução de eventos de input do Slint (já decodificados para primitivos no `.slint`) para os
//! tipos de input do Servo (`InputEvent` / `Scroll`). Porte fino do exemplo oficial
//! `slint-ui/slint/examples/servo` (`events_utils`), confirmado contra a fonte do `servo 0.2.0`.
//!
//! Coordenadas chegam em **device pixels** (o `.slint` passa `self.mouse-x/y` como `physical-length`,
//! então o Slint já aplicou o scale factor). Como o contexto do Servo tem o mesmo tamanho da área
//! web e a `Image` usa `image-fit: fill`, o mapeamento é identidade (ver doc do módulo em `main.rs`).

use servo::{
    Code, DevicePoint, DeviceVector2D, InputEvent, Key, KeyState, KeyboardEvent, Location,
    Modifiers, MouseButton, MouseButtonAction, MouseButtonEvent, MouseMoveEvent, NamedKey, Scroll,
    WebViewPoint,
};
use slint::platform::Key as SlintKey;
use slint::SharedString;

/// Constrói o `WebViewPoint` (em device pixels) usado como âncora de pointer/scroll.
pub fn device_point(x: f32, y: f32) -> WebViewPoint {
    DevicePoint::new(x, y).into()
}

/// Traduz um evento de pointer do Slint para `InputEvent` do Servo.
///
/// `kind`: 0 = down, 1 = up, qualquer outro = move (notificação de movimento, sem botão).
/// `button`: 0 = left, 1 = right, 2 = middle, qualquer outro = left (fallback).
#[must_use]
pub fn pointer_input_event(x: f32, y: f32, kind: i32, button: i32) -> InputEvent {
    let point = device_point(x, y);
    match kind {
        0 => InputEvent::MouseButton(MouseButtonEvent::new(
            MouseButtonAction::Down,
            mouse_button(button),
            point,
        )),
        1 => InputEvent::MouseButton(MouseButtonEvent::new(
            MouseButtonAction::Up,
            mouse_button(button),
            point,
        )),
        _ => InputEvent::MouseMove(MouseMoveEvent::new(point)),
    }
}

fn mouse_button(button: i32) -> MouseButton {
    match button {
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        _ => MouseButton::Left,
    }
}

/// Traduz o delta de scroll (logical px do Slint) para o `Scroll` do Servo. O sentido é **invertido**
/// (como no exemplo oficial): no Servo um delta positivo revela conteúdo acima/à esquerda.
#[must_use]
pub fn scroll_delta(delta_x: f32, delta_y: f32) -> Scroll {
    Scroll::Delta(DeviceVector2D::new(-delta_x, -delta_y).into())
}

/// Traduz um evento de teclado do Slint para `InputEvent::Keyboard` do Servo. `text` é o campo do
/// `KeyEvent` do Slint (teclas especiais vêm como chars de uso-privado; ver [`key_from_text`]).
/// O `Code` físico não é exposto pelo Slint, então usamos `Code::Unidentified` (como o exemplo).
#[must_use]
#[expect(
    clippy::fn_params_excessive_bools,
    reason = "espelho 1:1 do KeyEvent do Slint"
)]
pub fn key_input_event(
    text: &str,
    pressed: bool,
    ctrl: bool,
    shift: bool,
    alt: bool,
    meta: bool,
    repeat: bool,
) -> InputEvent {
    let state = if pressed {
        KeyState::Down
    } else {
        KeyState::Up
    };
    let key = key_from_text(text);
    let mut modifiers = Modifiers::empty();
    modifiers.set(Modifiers::CONTROL, ctrl);
    modifiers.set(Modifiers::SHIFT, shift);
    modifiers.set(Modifiers::ALT, alt);
    modifiers.set(Modifiers::META, meta);
    InputEvent::Keyboard(KeyboardEvent::new_without_event(
        state,
        key,
        Code::Unidentified,
        Location::Standard,
        modifiers,
        repeat,
        false,
    ))
}

/// Mapeia o `text` do `KeyEvent` do Slint para uma `Key` do Servo (`keyboard_types`). Teclas
/// especiais do Slint (`slint::platform::Key`) convertem para um char de uso-privado em `text`;
/// comparamos contra ele e devolvemos o `NamedKey` correspondente. Texto de 1 char vira
/// `Key::Character`. Porte do `key_event_util` do exemplo oficial.
fn key_from_text(text: &str) -> Key {
    macro_rules! named {
        ($slint_key:expr, $named:expr) => {
            if text == SharedString::from($slint_key).as_str() {
                return Key::Named($named);
            }
        };
    }

    named!(SlintKey::Backspace, NamedKey::Backspace);
    named!(SlintKey::Tab, NamedKey::Tab);
    named!(SlintKey::Return, NamedKey::Enter);
    named!(SlintKey::Escape, NamedKey::Escape);
    named!(SlintKey::Delete, NamedKey::Delete);
    named!(SlintKey::Shift, NamedKey::Shift);
    named!(SlintKey::ShiftR, NamedKey::Shift);
    named!(SlintKey::Control, NamedKey::Control);
    named!(SlintKey::ControlR, NamedKey::Control);
    named!(SlintKey::Alt, NamedKey::Alt);
    named!(SlintKey::AltGr, NamedKey::AltGraph);
    named!(SlintKey::Meta, NamedKey::Meta);
    named!(SlintKey::MetaR, NamedKey::Meta);
    named!(SlintKey::UpArrow, NamedKey::ArrowUp);
    named!(SlintKey::DownArrow, NamedKey::ArrowDown);
    named!(SlintKey::LeftArrow, NamedKey::ArrowLeft);
    named!(SlintKey::RightArrow, NamedKey::ArrowRight);
    named!(SlintKey::Home, NamedKey::Home);
    named!(SlintKey::End, NamedKey::End);
    named!(SlintKey::Insert, NamedKey::Insert);
    named!(SlintKey::PageUp, NamedKey::PageUp);
    named!(SlintKey::PageDown, NamedKey::PageDown);
    named!(SlintKey::Pause, NamedKey::Pause);
    named!(SlintKey::ScrollLock, NamedKey::ScrollLock);
    named!(SlintKey::F1, NamedKey::F1);
    named!(SlintKey::F2, NamedKey::F2);
    named!(SlintKey::F3, NamedKey::F3);
    named!(SlintKey::F4, NamedKey::F4);
    named!(SlintKey::F5, NamedKey::F5);
    named!(SlintKey::F6, NamedKey::F6);
    named!(SlintKey::F7, NamedKey::F7);
    named!(SlintKey::F8, NamedKey::F8);
    named!(SlintKey::F9, NamedKey::F9);
    named!(SlintKey::F10, NamedKey::F10);
    named!(SlintKey::F11, NamedKey::F11);
    named!(SlintKey::F12, NamedKey::F12);

    if text.chars().count() == 1 {
        return Key::Character(text.to_string());
    }
    Key::Named(NamedKey::Unidentified)
}

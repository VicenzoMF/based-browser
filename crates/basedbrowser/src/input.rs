//! Tradução de eventos de input do Slint (já decodificados para primitivos no `.slint`) para os
//! tipos de input do Servo (`InputEvent` / `Scroll`). Porte fino do exemplo oficial
//! `slint-ui/slint/examples/servo` (`events_utils`), confirmado contra a fonte do `servo 0.2.0`.
//!
//! Coordenadas chegam em **device pixels** (o `.slint` passa `self.mouse-x/y` como `physical-length`,
//! então o Slint já aplicou o scale factor). Como o contexto do Servo tem o mesmo tamanho da área
//! web e a `Image` usa `image-fit: fill`, o mapeamento é identidade (ver doc do módulo em `main.rs`).

use servo::{
    DevicePoint, DeviceVector2D, InputEvent, MouseButton, MouseButtonAction, MouseButtonEvent,
    MouseMoveEvent, Scroll, WebViewPoint,
};

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

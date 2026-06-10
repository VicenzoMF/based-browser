//! BasedBrowser — janela do produto (Slint). **Marco M1**: o Slint hospeda o motor Servo
//! exibindo, via cópia-CPU, uma página renderizada (URL fixa). Ver `.specs/project/ROADMAP.md`
//! (M1), `docs/adr/0003-*` (arquitetura da integração) e `crates/servo-poc` (prova de conceito do M0).
//!
//! T2 (este passo) — **spike de de-risking (AD-004):** prova se um contexto de render do Servo
//! (`WindowRenderingContext` → `OffscreenRenderingContext`) pode coexistir com o renderer do Slint
//! na MESMA janela. Inicializa o contexto no hook `set_rendering_notifier(RenderingSetup)` (o mesmo
//! ponto que o caminho GPU do M3 usará) e registra OK/FALHA. Sem `WebView`/pixels ainda (T3).

use std::cell::RefCell;
use std::rc::Rc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{OffscreenRenderingContext, RenderingContext, WindowRenderingContext};
use slint::{ComponentHandle, Image, RenderingState, Rgba8Pixel, SharedPixelBuffer};

slint::slint! {
    export component MainWindow inherits Window {
        in property <image> frame;
        title: "BasedBrowser";
        // Tamanho inicial da viewport web (casará com o tamanho do buffer do Servo no T3).
        preferred-width: 1024px;
        preferred-height: 768px;
        Image {
            source: root.frame;
            width: 100%;
            height: 100%;
            // O conteúdo já vem no tamanho do frame; não esticar/borrar.
            image-fit: contain;
        }
    }
}

/// Contexto de render do Servo derivado da janela do Slint. Mantido vivo (parent + offscreen) para
/// o spike testar a coexistência ao longo da sessão, não só na criação.
struct ServoRenderContext {
    _parent: Rc<WindowRenderingContext>,
    _offscreen: Rc<OffscreenRenderingContext>,
}

/// Tenta criar o contexto offscreen do Servo SOBRE a janela do Slint (caminho future-proof:
/// mesmo tipo que o M3 exportará p/ GPU). É a função única e trocável da "escada de fallback"
/// do plano — se a coexistência GL falhar, troca-se o corpo por `SoftwareRenderingContext`.
fn create_servo_offscreen(window: &slint::Window) -> Result<ServoRenderContext, String> {
    let provider = window.window_handle();
    let display_handle = provider
        .display_handle()
        .map_err(|e| format!("display_handle: {e}"))?;
    let window_handle = provider
        .window_handle()
        .map_err(|e| format!("window_handle: {e}"))?;

    let size = window.size();
    let physical = dpi::PhysicalSize::new(size.width.max(1), size.height.max(1));

    let parent = Rc::new(
        WindowRenderingContext::new(display_handle, window_handle, physical)
            .map_err(|e| format!("WindowRenderingContext::new: {e:?}"))?,
    );
    let offscreen = Rc::new(parent.offscreen_context(physical));
    offscreen
        .make_current()
        .map_err(|e| format!("offscreen.make_current: {e:?}"))?;

    Ok(ServoRenderContext {
        _parent: parent,
        _offscreen: offscreen,
    })
}

/// Placeholder do frame: buffer RGBA8 de cor sólida, no mesmo caminho de dados que o Servo usará no
/// T3 (`SharedPixelBuffer<Rgba8Pixel>` → `Image::from_rgba8`). Serve de "controle" visual do spike:
/// se a janela seguir exibindo esta cor após o Servo pegar um contexto GL, a coexistência funciona.
fn placeholder_frame(width: u32, height: u32) -> Image {
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    let fill = Rgba8Pixel {
        r: 28,
        g: 30,
        b: 38,
        a: 255,
    };
    for pixel in buffer.make_mut_slice() {
        *pixel = fill;
    }
    Image::from_rgba8(buffer)
}

fn main() -> Result<(), slint::PlatformError> {
    let app = MainWindow::new()?;
    app.set_frame(placeholder_frame(1024, 768));

    // Inicializa o contexto do Servo quando o Slint terminou de montar seu contexto gráfico
    // (RenderingSetup) — momento em que a janela já existe e o handle é válido. O `held` (co-possuído
    // dentro e fora da closure) mantém o contexto vivo pela sessão (teste de coexistência) e serve
    // de guarda de init única (`is_some`).
    let weak = app.as_weak();
    let held: Rc<RefCell<Option<ServoRenderContext>>> = Rc::new(RefCell::new(None));
    let held_in_cb = held.clone();
    let notifier = app
        .window()
        .set_rendering_notifier(move |state, _graphics_api| {
            if !matches!(state, RenderingState::RenderingSetup) || held_in_cb.borrow().is_some() {
                return;
            }
            let Some(app) = weak.upgrade() else { return };
            match create_servo_offscreen(app.window()) {
                Ok(ctx) => {
                    eprintln!(
                        "[spike] OK — Servo WindowRenderingContext + offscreen + make_current \
                         criados sobre a janela do Slint."
                    );
                    *held_in_cb.borrow_mut() = Some(ctx);
                }
                Err(e) => eprintln!("[spike] FALHA ao criar o contexto do Servo: {e}"),
            }
        });
    if let Err(e) = notifier {
        eprintln!("[spike] set_rendering_notifier indisponivel (renderer sem GL?): {e}");
    }

    let result = app.run();
    drop(held); // libera o contexto do Servo só depois do loop encerrar.
    result
}

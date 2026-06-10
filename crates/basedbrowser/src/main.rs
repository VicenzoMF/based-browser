//! BasedBrowser — janela do produto (Slint). **Marco M1**: o Slint hospeda o motor Servo
//! exibindo, via cópia-CPU, uma página renderizada (URL fixa). Ver `.specs/project/ROADMAP.md`
//! (M1), `docs/adr/0003-*` (arquitetura) e `crates/servo-poc` (prova de conceito do M0).
//!
//! Arquitetura (future-proof rumo ao M3, ver ADR-0003):
//! - O **Slint é dono do event loop** e da janela (renderer femtovg/GL por default).
//! - O **Servo** renderiza num `OffscreenRenderingContext` (FBO de GL de hardware) derivado de um
//!   `WindowRenderingContext` criado a partir do handle da janela do Slint.
//! - Um `slint::Timer` dirige o Servo (`spin_event_loop`) e, a cada frame novo, lê o FBO
//!   (`read_to_image` -> `SharedPixelBuffer` -> `Image::from_rgba8`) e entrega à UI. Cópia-CPU
//!   (AD-003): no M3 troca-se só o readback por compartilhamento de textura GPU.
//!
//! **Lição do M1 (ADR-0003):** o contexto GL do Servo é montado de forma LAZY, alguns ticks após
//! o loop subir — NÃO dentro do `set_rendering_notifier(RenderingSetup)`. Inicializar durante o
//! setup do renderer do Slint corrompia o estado de GL compartilhado (erros do femtovg + frames em
//! branco). Adiar o init faz os dois renderers de hardware coexistirem na mesma janela.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use euclid::Scale;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{
    DeviceIntRect, EventLoopWaker, OffscreenRenderingContext, RenderingContext, Servo,
    ServoBuilder, WebView, WebViewBuilder, WebViewDelegate, WindowRenderingContext,
};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode};
use url::Url;

slint::slint! {
    export component MainWindow inherits Window {
        in property <image> frame;
        title: "BasedBrowser";
        // Tamanho inicial da viewport web (casa com o tamanho do contexto do Servo).
        preferred-width: 1024px;
        preferred-height: 768px;
        Image {
            source: root.frame;
            width: 100%;
            height: 100%;
            image-fit: contain;
        }
    }
}

/// Página de demonstração do M1 (HTML/CSS auto-contido). Carregada via `file://` para um render
/// determinístico e offline (sem rede/TLS).
const M1_PAGE_HTML: &str = r#"<!doctype html>
<html lang="pt-br"><head><meta charset="utf-8"><style>
  * { margin: 0; box-sizing: border-box; }
  html, body { height: 100%; }
  body {
    font-family: system-ui, sans-serif; color: #f5f7ff;
    background: linear-gradient(135deg, #1e2030 0%, #3a2d5c 50%, #5c2d4d 100%);
    display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 28px;
  }
  h1 { font-size: 56px; letter-spacing: -1px; }
  h1 span { color: #ff7eb6; }
  p { font-size: 20px; opacity: .85; }
  .row { display: flex; gap: 18px; }
  .card {
    width: 150px; height: 110px; border-radius: 16px; padding: 18px;
    background: rgba(255,255,255,.08); border: 1px solid rgba(255,255,255,.18);
    display: flex; align-items: flex-end; font-weight: 600;
  }
  .a { background: rgba(255,126,182,.22); }
  .b { background: rgba(126,203,255,.22); }
  .c { background: rgba(140,255,180,.22); }
</style></head><body>
  <h1>Based<span>Browser</span></h1>
  <p>Servo renderizando dentro de uma janela Slint — Marco M1 (cópia-CPU)</p>
  <div class="row">
    <div class="card a">flexbox</div>
    <div class="card b">gradiente</div>
    <div class="card c">CSS</div>
  </div>
</body></html>
"#;

/// Atraso (em ticks de ~16 ms) antes de montar o contexto GL do Servo, para o renderer do Slint
/// estabilizar e evitar a colisão de GL no init (ver lição no doc do módulo / ADR-0003).
const INIT_DELAY_TICKS: u32 = 8;

/// `EventLoopWaker` mínimo: o `Timer` de ~16 ms já dirige o `spin_event_loop` continuamente, então
/// o M1 não precisa que o `wake()` agende trabalho (otimização do waker fica para o M2).
#[derive(Clone)]
struct PeriodicWaker;

impl EventLoopWaker for PeriodicWaker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }
    fn wake(&self) {}
}

/// Sink de frames: o `WebViewDelegate` marca "sujo" quando o Servo tem um frame novo; o `Timer` lê
/// e limpa a flag antes de ler o FBO.
struct FrameSink {
    dirty: Cell<bool>,
}

impl WebViewDelegate for FrameSink {
    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.dirty.set(true);
    }
}

/// Estado vivo do motor. `_parent` é mantido vivo porque o `OffscreenRenderingContext` empresta o
/// contexto GL dele.
struct Runtime {
    webview: WebView,
    servo: Servo,
    context: Rc<OffscreenRenderingContext>,
    _parent: Rc<WindowRenderingContext>,
}

/// Cria o `Servo` + `WebView` renderizando num `OffscreenRenderingContext` (FBO de GL de hardware)
/// derivado da janela do Slint. `show()` + `focus()` são necessários: sem `show()` a pipeline fica
/// "fechada" e renderiza em branco.
fn init_runtime(window: &slint::Window, sink: Rc<FrameSink>) -> Result<Runtime, String> {
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
    let context = Rc::new(parent.offscreen_context(physical));

    let servo = ServoBuilder::default()
        .event_loop_waker(Box::new(PeriodicWaker))
        .build();
    servo.setup_logging();

    let webview = WebViewBuilder::new(&servo, context.clone())
        .url(fixed_page_url()?)
        .hidpi_scale_factor(Scale::new(window.scale_factor()))
        .delegate(sink)
        .build();
    webview.focus();
    webview.show();

    Ok(Runtime {
        webview,
        servo,
        context,
        _parent: parent,
    })
}

/// Escreve a página de demo num arquivo temporário e devolve a URL `file://` correspondente.
fn fixed_page_url() -> Result<Url, String> {
    let path = std::env::temp_dir().join("basedbrowser-m1.html");
    std::fs::write(&path, M1_PAGE_HTML).map_err(|e| format!("escrever HTML de demo: {e}"))?;
    Url::from_file_path(&path).map_err(|()| "Url::from_file_path falhou".to_string())
}

/// Frame inicial (cor sólida) antes do Servo produzir o primeiro frame.
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

/// Sequência canônica de leitura (servo-paint/screenshot.rs): `paint()` renderiza no FBO offscreen
/// (o compositor faz `make_current` + `prepare_for_rendering` internamente) -> `make_current` ->
/// `read_to_image` lê o FBO. Copia para um `SharedPixelBuffer` e atualiza a `Image` da UI.
/// Define `BASEDBROWSER_DUMP_FRAME=<path>` p/ salvar o frame em PNG (evidência/depuração).
fn pump_frame(runtime: &Runtime, weak: &slint::Weak<MainWindow>, logged: &Cell<bool>) {
    runtime.webview.paint();
    if runtime.context.make_current().is_err() {
        return;
    }
    let rect = DeviceIntRect::from_size(runtime.context.size2d().to_i32());
    let Some(frame) = runtime.context.read_to_image(rect) else {
        return;
    };

    if let Ok(path) = std::env::var("BASEDBROWSER_DUMP_FRAME") {
        match frame.save(&path) {
            Ok(()) => {
                if !logged.replace(true) {
                    eprintln!(
                        "[m1] frame do Servo salvo em {path} ({}x{})",
                        frame.width(),
                        frame.height()
                    );
                }
            }
            Err(e) => eprintln!("[m1] falha ao salvar dump do frame: {e}"),
        }
    }

    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(frame.width(), frame.height());
    buffer.make_mut_bytes().copy_from_slice(frame.as_raw());
    if let Some(app) = weak.upgrade() {
        app.set_frame(Image::from_rgba8(buffer));
    }
}

fn main() -> Result<(), slint::PlatformError> {
    // Provedor de cripto process-wide p/ TLS. `install_default` falha se já houver um — ignorável.
    if rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .is_err()
    {
        eprintln!("[m1] provedor de cripto rustls ja instalado (ok)");
    }

    let app = MainWindow::new()?;
    app.set_frame(placeholder_frame(1024, 768));

    let weak = app.as_weak();
    let runtime: Rc<RefCell<Option<Runtime>>> = Rc::new(RefCell::new(None));
    let sink = Rc::new(FrameSink {
        dirty: Cell::new(false),
    });
    let logged = Rc::new(Cell::new(false));

    // Dirige o Servo e bombeia frames. O runtime é montado LAZY aqui (e não no
    // `set_rendering_notifier`) para criar o contexto GL do Servo FORA do setup do renderer do
    // Slint, evitando a colisão de `make_current` que corrompia o GL (ver doc do módulo).
    let timer = Timer::default();
    let tick_runtime = runtime;
    let tick_sink = sink;
    let tick_weak = weak;
    let tick_logged = logged;
    let init_ticks = Cell::new(0u32);
    timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
        if tick_runtime.borrow().is_none() {
            let n = init_ticks.get();
            init_ticks.set(n + 1);
            if n < INIT_DELAY_TICKS {
                return;
            }
            let Some(app) = tick_weak.upgrade() else {
                return;
            };
            match init_runtime(app.window(), tick_sink.clone()) {
                Ok(rt) => {
                    eprintln!(
                        "[m1] runtime do Servo iniciado (offscreen GL sobre a janela do Slint)"
                    );
                    *tick_runtime.borrow_mut() = Some(rt);
                }
                Err(e) => eprintln!("[m1] FALHA ao iniciar o runtime do Servo: {e}"),
            }
            return;
        }

        let guard = tick_runtime.borrow();
        let Some(rt) = guard.as_ref() else {
            return;
        };
        if rt.context.make_current().is_err() {
            return;
        }
        rt.servo.spin_event_loop();
        if tick_sink.dirty.replace(false) {
            pump_frame(rt, &tick_weak, &tick_logged);
        }
    });

    app.run()
}

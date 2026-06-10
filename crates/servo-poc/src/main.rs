/* Derivado de servo/servo `components/servo/examples/winit_minimal.rs` (tag v0.2.0),
 * licenciado sob a Mozilla Public License 2.0 (https://mozilla.org/MPL/2.0/).
 * Adaptado para o BasedBrowser: usa os re-exports `servo::` (embedding fino, STATE L-001),
 * remove `unwrap`/`expect` (lints do projeto) e enxuga o exemplo (sem scroll/código morto). */

//! `servo-poc` — PoC isolada do **Marco M0**: embute o motor Servo numa janela `winit`
//! pura (SEM Slint) só para provar build + render de uma página nesta máquina.
//! Ver `.specs/project/ROADMAP.md` (M0) e `docs/adr/0002-pin-servo-0.2.0.md`.

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use euclid::Scale;
use servo::{
    EventLoopWaker, RenderingContext, Servo, ServoBuilder, WebView, WebViewBuilder,
    WebViewDelegate, WindowRenderingContext,
};
use url::Url;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Window, WindowId};

/// Página aberta por padrão; sobrescreva com `cargo run -p servo-poc -- <url>`.
const DEFAULT_URL: &str = "https://servo.org";

fn main() -> Result<(), Box<dyn Error>> {
    // Provedor de cripto process-wide p/ TLS (HTTPS). Sem isto, o `net` do Servo entra em pânico.
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| "falha ao instalar o provedor de cripto rustls (aws-lc-rs)")?;

    let event_loop = EventLoop::with_user_event().build()?;
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_URL.to_owned());
    let mut app = App::new(&event_loop, url);
    event_loop.run_app(&mut app)?;
    Ok(())
}

/// Estado vivo da aplicação depois que o event loop do winit sobe (`resumed`).
struct AppState {
    window: Window,
    servo: Servo,
    rendering_context: Rc<WindowRenderingContext>,
    webviews: RefCell<Vec<WebView>>,
}

impl WebViewDelegate for AppState {
    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.window.request_redraw();
    }
}

/// Janela/Servo só existem após `resumed`, então começamos em `Initial` (só o necessário
/// p/ subir) e migramos para `Running`.
enum App {
    Initial { waker: Waker, url: String },
    Running(Rc<AppState>),
}

impl App {
    fn new(event_loop: &EventLoop<WakerEvent>, url: String) -> Self {
        Self::Initial {
            waker: Waker::new(event_loop),
            url,
        }
    }

    /// Cria janela + contexto GL + Servo + WebView. Falível: o chamador trata o erro.
    fn start(
        event_loop: &ActiveEventLoop,
        waker: &Waker,
        url: &str,
    ) -> Result<Rc<AppState>, Box<dyn Error>> {
        let display_handle = event_loop.display_handle()?;
        let window = event_loop.create_window(Window::default_attributes())?;
        let window_handle = window.window_handle()?;

        let rendering_context = Rc::new(
            WindowRenderingContext::new(display_handle, window_handle, window.inner_size())
                .map_err(|e| format!("WindowRenderingContext::new falhou: {e:?}"))?,
        );
        // Ativa o contexto GL na thread atual (mesmo padrão do exemplo do Servo).
        let _ = rendering_context.make_current();

        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(waker.clone()))
            .build();
        servo.setup_logging();

        let app_state = Rc::new(AppState {
            window,
            servo,
            rendering_context,
            webviews: RefCell::new(Vec::new()),
        });

        let webview = WebViewBuilder::new(&app_state.servo, app_state.rendering_context.clone())
            .url(Url::parse(url)?)
            .hidpi_scale_factor(Scale::new(app_state.window.scale_factor() as f32))
            .delegate(app_state.clone())
            .build();
        app_state.webviews.borrow_mut().push(webview);

        Ok(app_state)
    }
}

impl ApplicationHandler<WakerEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Clona o pouco que precisamos p/ não conflitar com a reatribuição de `self`.
        let (waker, url) = match self {
            Self::Initial { waker, url } => (waker.clone(), url.clone()),
            Self::Running(_) => return,
        };
        match App::start(event_loop, &waker, &url) {
            Ok(state) => *self = Self::Running(state),
            Err(e) => {
                eprintln!("servo-poc: falha ao iniciar: {e}");
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: WakerEvent) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.webviews.borrow().last() {
                        webview.paint();
                        state.rendering_context.present();
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.webviews.borrow().last() {
                        webview.resize(new_size);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Ponte winit↔Servo: o Servo chama `wake()` (de qualquer thread) p/ pedir mais uma volta do
/// event loop do winit — onde rodamos `spin_event_loop`.
#[derive(Clone)]
struct Waker(winit::event_loop::EventLoopProxy<WakerEvent>);

#[derive(Debug)]
struct WakerEvent;

impl Waker {
    fn new(event_loop: &EventLoop<WakerEvent>) -> Self {
        Self(event_loop.create_proxy())
    }
}

impl EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }

    fn wake(&self) {
        // Se o loop já fechou não há o que acordar; ignorar o erro é seguro.
        let _ = self.0.send_event(WakerEvent);
    }
}

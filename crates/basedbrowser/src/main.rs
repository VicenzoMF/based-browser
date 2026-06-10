//! BasedBrowser — janela do produto (Slint). **Marco M2** (browser navegável): evolui o pipeline
//! de cópia-CPU do M1 com input (pointer/scroll/teclado encaminhados ao Servo), chrome mínimo
//! (barra de URL + voltar/avançar/recarregar + indicador de carregamento) e resize dinâmico. Ver
//! `.specs/project/ROADMAP.md` (M2), `docs/adr/0003-*` (arquitetura M1) e `crates/servo-poc` (M0).
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

mod input;

use euclid::Scale;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{
    DeviceIntRect, EventLoopWaker, LoadStatus, OffscreenRenderingContext, RenderingContext, Servo,
    ServoBuilder, WebView, WebViewBuilder, WebViewDelegate, WindowRenderingContext,
};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode};
use url::Url;

slint::slint! {
    import { Button, LineEdit } from "std-widgets.slint";

    // Chrome do M2: barra de navegação + área web. O `.slint` apenas DECODIFICA cada evento
    // para primitivos e chama um callback; a tradução primitivo -> `InputEvent` do Servo mora no
    // Rust (`input.rs`). Coordenadas: declarar os params como `physical-length` faz o Slint
    // converter logical -> físico pelo scale factor da janela, e como a Image usa `image-fit: fill`
    // sobre um contexto do Servo do MESMO tamanho da área web, o mapeamento é identidade
    // (physical-px == device-px do Servo). Sem matemática de letterbox.
    export component MainWindow inherits Window {
        // Dirigidas pelo Rust (pipeline de frame + WebViewDelegate).
        in property <image> frame;
        in property <string> page-url;
        in property <bool> loading;
        in property <bool> can-go-back;
        in property <bool> can-go-forward;
        in property <string> window-title: "BasedBrowser";

        // Chrome -> Rust.
        callback load-url(string);
        callback go-back();
        callback go-forward();
        callback reload();
        // Input -> Rust. pointer: (x, y, kind, button); kind 0=down 1=up 2=move; button 0=left
        // 1=right 2=middle 3=outro. scroll: (x, y, delta-x, delta-y). key: (text, pressed, ctrl,
        // shift, alt, meta, repeat). web-resized: novo tamanho FÍSICO da área web.
        callback forward-pointer(physical-length, physical-length, int, int);
        callback forward-scroll(physical-length, physical-length, length, length);
        callback forward-key(string, bool, bool, bool, bool, bool, bool);
        callback web-resized(physical-length, physical-length);

        title: root.window-title;
        preferred-width: 1024px;
        preferred-height: 768px;

        VerticalLayout {
            padding: 0;
            spacing: 0;

            // Barra de navegação (altura natural; não estica).
            HorizontalLayout {
                vertical-stretch: 0;
                padding: 6px;
                spacing: 6px;
                Button {
                    text: "<";
                    enabled: root.can-go-back;
                    clicked => { root.go-back(); }
                }
                Button {
                    text: ">";
                    enabled: root.can-go-forward;
                    clicked => { root.go-forward(); }
                }
                Button {
                    text: "Recarregar";
                    clicked => { root.reload(); }
                }
                LineEdit {
                    placeholder-text: "Digite uma URL e tecle Enter";
                    text: root.page-url;
                    accepted(t) => { root.load-url(t); }
                }
                if root.loading : Text {
                    vertical-alignment: center;
                    color: #aaaaaa;
                    text: "carregando...";
                }
            }

            // Area web: a Image exibe o frame do Servo; a TouchArea captura pointer/scroll e o
            // FocusScope captura teclado. `web-resized` dispara quando o layout muda o tamanho.
            web := Rectangle {
                vertical-stretch: 1;
                background: #1e1e26;
                changed width => { root.web-resized(self.width, self.height); }
                changed height => { root.web-resized(self.width, self.height); }

                Image {
                    width: 100%;
                    height: 100%;
                    image-fit: fill;
                    source: root.frame;
                }
                fs := FocusScope {
                    width: 100%;
                    height: 100%;
                    key-pressed(e) => {
                        root.forward-key(e.text, true, e.modifiers.control, e.modifiers.shift,
                            e.modifiers.alt, e.modifiers.meta, e.repeat);
                        accept
                    }
                    key-released(e) => {
                        root.forward-key(e.text, false, e.modifiers.control, e.modifiers.shift,
                            e.modifiers.alt, e.modifiers.meta, false);
                        accept
                    }
                    TouchArea {
                        width: 100%;
                        height: 100%;
                        pointer-event(e) => {
                            root.forward-pointer(
                                self.mouse-x,
                                self.mouse-y,
                                e.kind == PointerEventKind.down ? 0
                                    : e.kind == PointerEventKind.up ? 1 : 2,
                                e.button == PointerEventButton.left ? 0
                                    : e.button == PointerEventButton.right ? 1
                                    : e.button == PointerEventButton.middle ? 2 : 3);
                            fs.focus();
                        }
                        scroll-event(e) => {
                            root.forward-scroll(self.mouse-x, self.mouse-y, e.delta-x, e.delta-y);
                            accept
                        }
                    }
                }
            }
        }
    }
}

/// Página inicial/de-teste do M2 (HTML/CSS auto-contido). Carregada via `file://` para um render
/// determinístico e offline (sem rede/TLS). É **rolável** (testa scroll), tem um `<input>` (testa
/// teclado) e um link para a 2ª página (testa clique -> navegação -> voltar). O token
/// `__PAGE2_URL__` é trocado pela URL real da 2ª página em [`home_page_url`].
const START_HTML: &str = r#"<!doctype html>
<html lang="pt-br"><head><meta charset="utf-8"><style>
  * { margin: 0; box-sizing: border-box; }
  body {
    font-family: system-ui, sans-serif; color: #f5f7ff; padding: 48px;
    background: linear-gradient(135deg, #1e2030 0%, #3a2d5c 50%, #5c2d4d 100%);
  }
  h1 { font-size: 52px; letter-spacing: -1px; }
  h1 span { color: #ff7eb6; }
  p { font-size: 19px; opacity: .85; margin-top: 12px; max-width: 760px; }
  .panel {
    margin-top: 28px; padding: 22px; border-radius: 16px; max-width: 760px;
    background: rgba(255,255,255,.08); border: 1px solid rgba(255,255,255,.18);
  }
  label { display: block; font-weight: 600; margin-bottom: 8px; }
  input {
    width: 100%; padding: 12px 14px; font-size: 16px; border-radius: 10px;
    border: 1px solid rgba(255,255,255,.3); background: rgba(0,0,0,.25); color: #fff;
  }
  a.nav {
    display: inline-block; margin-top: 22px; padding: 12px 18px; border-radius: 10px;
    background: #ff7eb6; color: #1e2030; font-weight: 700; text-decoration: none;
  }
  .spacer { height: 900px; }
  .end { font-size: 22px; font-weight: 700; color: #8cffb4; }
</style></head><body>
  <h1>Based<span>Browser</span> — M2</h1>
  <p>Browser navegável: digite uma URL na barra e tecle Enter; clique/role/digite nesta
     página; use voltar/avancar/recarregar. Esta pagina e rolavel (role ate o fim).</p>
  <div class="panel">
    <label for="t">Teste de teclado — clique e digite aqui:</label>
    <input id="t" type="text" placeholder="o texto digitado deve aparecer">
  </div>
  <a class="nav" href="__PAGE2_URL__">Ir para a Pagina 2 (testar clique + navegacao)</a>
  <div class="spacer"></div>
  <p class="end">Fim da pagina — se voce leu isto rolando, o scroll funciona.</p>
</body></html>
"#;

/// 2ª página do harness de teste (alvo do link da inicial). `__START_URL__` -> URL da inicial.
const PAGE2_HTML: &str = r#"<!doctype html>
<html lang="pt-br"><head><meta charset="utf-8"><style>
  * { margin: 0; box-sizing: border-box; }
  body {
    font-family: system-ui, sans-serif; color: #1e2030; padding: 48px;
    background: linear-gradient(135deg, #8cffb4 0%, #7ecbff 100%);
  }
  h1 { font-size: 52px; }
  p { font-size: 19px; margin-top: 12px; max-width: 760px; }
  a.nav {
    display: inline-block; margin-top: 22px; padding: 12px 18px; border-radius: 10px;
    background: #1e2030; color: #fff; font-weight: 700; text-decoration: none;
  }
</style></head><body>
  <h1>Pagina 2</h1>
  <p>Voce navegou via clique num link. Agora teste o botao voltar do chrome
     (deve ficar habilitado) e o recarregar.</p>
  <a class="nav" href="__START_URL__">Voltar para a inicial (ou use o botao voltar)</a>
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

/// `WebViewDelegate`: ponte do Servo para a UI. Marca "sujo" quando há frame novo (lido pelo `Timer`)
/// e dirige o chrome (URL, carregamento, voltar/avançar, título) via um handle fraco da janela. Roda
/// na main thread durante `spin_event_loop`; só toca o `app` e a `WebView` recebida (nunca o
/// `RefCell` do runtime), então não há risco de borrow reentrante.
struct Embedder {
    dirty: Cell<bool>,
    app: slint::Weak<MainWindow>,
}

impl Embedder {
    /// Reflete `can_go_back`/`can_go_forward` da `WebView` nas propriedades do chrome.
    fn sync_history(&self, webview: &WebView) {
        if let Some(app) = self.app.upgrade() {
            app.set_can_go_back(webview.can_go_back());
            app.set_can_go_forward(webview.can_go_forward());
        }
    }
}

impl WebViewDelegate for Embedder {
    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.dirty.set(true);
    }

    fn notify_load_status_changed(&self, webview: WebView, status: LoadStatus) {
        if let Some(app) = self.app.upgrade() {
            app.set_loading(status != LoadStatus::Complete);
        }
        self.sync_history(&webview);
    }

    fn notify_url_changed(&self, webview: WebView, url: Url) {
        if let Some(app) = self.app.upgrade() {
            app.set_page_url(url.to_string().into());
        }
        self.sync_history(&webview);
    }

    fn notify_history_changed(&self, webview: WebView, _entries: Vec<Url>, _current: usize) {
        self.sync_history(&webview);
    }

    fn notify_page_title_changed(&self, _webview: WebView, title: Option<String>) {
        if let Some(app) = self.app.upgrade() {
            let title = title.unwrap_or_default();
            let shown = if title.is_empty() {
                "BasedBrowser".to_string()
            } else {
                format!("{title} — BasedBrowser")
            };
            app.set_window_title(shown.into());
        }
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
fn init_runtime(
    window: &slint::Window,
    sink: Rc<Embedder>,
    web_size: dpi::PhysicalSize<u32>,
) -> Result<Runtime, String> {
    let provider = window.window_handle();
    let display_handle = provider
        .display_handle()
        .map_err(|e| format!("display_handle: {e}"))?;
    let window_handle = provider
        .window_handle()
        .map_err(|e| format!("window_handle: {e}"))?;

    // O contexto PAI cobre a janela inteira (é a superfície GL da janela); o offscreen do Servo é
    // dimensionado pela ÁREA WEB (exclui a toolbar), o que torna o mapeamento de coordenadas
    // identidade (ver doc do módulo). O resize dinâmico só mexe no offscreen (ver `wire_chrome`).
    let size = window.size();
    let window_physical = dpi::PhysicalSize::new(size.width.max(1), size.height.max(1));
    let web_physical = dpi::PhysicalSize::new(web_size.width.max(1), web_size.height.max(1));

    let parent = Rc::new(
        WindowRenderingContext::new(display_handle, window_handle, window_physical)
            .map_err(|e| format!("WindowRenderingContext::new: {e:?}"))?,
    );
    let context = Rc::new(parent.offscreen_context(web_physical));

    let servo = ServoBuilder::default()
        .event_loop_waker(Box::new(PeriodicWaker))
        .build();
    servo.setup_logging();

    let webview = WebViewBuilder::new(&servo, context.clone())
        .url(home_page_url()?)
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

/// Converte um comprimento físico (`f32`, vindo do Slint como `physical-length`) para pixels,
/// arredondando e garantindo o mínimo de 1 (o `OffscreenRenderingContext` exige dimensões >= 1x1).
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "physical-length é finito e >= 0; arredondado e clampado a [1, u16::MAX]"
)]
fn physical_px(value: f32) -> u32 {
    value.round().clamp(1.0, f32::from(u16::MAX)) as u32
}

/// Interpreta o texto digitado na barra como `Url`. Se já é uma URL absoluta (tem esquema), usa
/// como está; senão tenta prefixar `https://` (atalho de digitação tipo `exemplo.com`). Devolve
/// `None` se nem assim virar uma URL válida.
fn parse_user_url(input: &str) -> Option<Url> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(url) = Url::parse(trimmed) {
        return Some(url);
    }
    Url::parse(&format!("https://{trimmed}")).ok()
}

/// Escreve as duas páginas de teste (cruzadas por link) em arquivos temporários e devolve a URL
/// `file://` da inicial. Offline/determinístico (sem rede/TLS).
fn home_page_url() -> Result<Url, String> {
    let dir = std::env::temp_dir();
    let start_path = dir.join("basedbrowser-start.html");
    let page2_path = dir.join("basedbrowser-page2.html");
    let start_url = Url::from_file_path(&start_path)
        .map_err(|()| "Url::from_file_path (start) falhou".to_string())?;
    let page2_url = Url::from_file_path(&page2_path)
        .map_err(|()| "Url::from_file_path (page2) falhou".to_string())?;

    std::fs::write(
        &start_path,
        START_HTML.replace("__PAGE2_URL__", page2_url.as_str()),
    )
    .map_err(|e| format!("escrever HTML inicial: {e}"))?;
    std::fs::write(
        &page2_path,
        PAGE2_HTML.replace("__START_URL__", start_url.as_str()),
    )
    .map_err(|e| format!("escrever HTML pagina 2: {e}"))?;
    Ok(start_url)
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

/// Registra os callbacks do chrome (Slint) que encaminham input e navegação à `WebView`. Cada
/// handler captura um clone do runtime compartilhado; se o evento chegar antes do init lazy, o
/// `if let Some` ignora com segurança. Borrows são curtos e os callbacks do Slint rodam
/// serializados com o `Timer` na main thread (sem reentrância no `RefCell`).
fn wire_chrome(
    app: &MainWindow,
    runtime: &Rc<RefCell<Option<Runtime>>>,
    web_size: &Rc<Cell<dpi::PhysicalSize<u32>>>,
) {
    // Resize: o `.slint` dispara `web-resized` com o tamanho FÍSICO da área web quando o layout
    // muda. Guardamos para o init lazy e, se o runtime já existe, redimensionamos só o contexto
    // OFFSCREEN via `webview.resize` (recria o FBO + reflui o viewport; ver painter.rs). NÃO
    // tocamos o `WindowRenderingContext` pai — resize concorrente das duas superfícies GL é o que
    // corrompia o estado compartilhado no M1 (L-004 / ADR-0003).
    let rt = runtime.clone();
    let ws = web_size.clone();
    app.on_web_resized(move |w, h| {
        let size = dpi::PhysicalSize::new(physical_px(w), physical_px(h));
        ws.set(size);
        if let Some(r) = rt.borrow().as_ref() {
            r.webview.resize(size);
        }
    });

    let rt = runtime.clone();
    app.on_forward_pointer(move |x, y, kind, button| {
        if let Some(r) = rt.borrow().as_ref() {
            r.webview
                .notify_input_event(input::pointer_input_event(x, y, kind, button));
        }
    });

    let rt = runtime.clone();
    app.on_forward_scroll(move |x, y, dx, dy| {
        if let Some(r) = rt.borrow().as_ref() {
            r.webview
                .notify_scroll_event(input::scroll_delta(dx, dy), input::device_point(x, y));
        }
    });

    let rt = runtime.clone();
    app.on_forward_key(move |text, pressed, ctrl, shift, alt, meta, repeat| {
        if let Some(r) = rt.borrow().as_ref() {
            r.webview.notify_input_event(input::key_input_event(
                text.as_str(),
                pressed,
                ctrl,
                shift,
                alt,
                meta,
                repeat,
            ));
        }
    });

    let rt = runtime.clone();
    app.on_load_url(move |text| {
        if let Some(r) = rt.borrow().as_ref() {
            match parse_user_url(text.as_str()) {
                Some(url) => r.webview.load(url),
                None => eprintln!("[m2] URL invalida ignorada: {text:?}"),
            }
        }
    });

    let rt = runtime.clone();
    app.on_go_back(move || {
        if let Some(r) = rt.borrow().as_ref() {
            if r.webview.can_go_back() {
                r.webview.go_back(1);
            }
        }
    });

    let rt = runtime.clone();
    app.on_go_forward(move || {
        if let Some(r) = rt.borrow().as_ref() {
            if r.webview.can_go_forward() {
                r.webview.go_forward(1);
            }
        }
    });

    let rt = runtime.clone();
    app.on_reload(move || {
        if let Some(r) = rt.borrow().as_ref() {
            r.webview.reload();
        }
    });
}

fn main() -> Result<(), slint::PlatformError> {
    // Provedor de cripto process-wide p/ TLS. `install_default` falha se já houver um — ignorável.
    if rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .is_err()
    {
        eprintln!("[m1] provedor de cripto rustls ja instalado (ok)");
    }

    // M3 (ADR-0005): força o renderer femtovg sobre wgpu (Vulkan no Linux) ANTES de criar qualquer
    // janela/componente. `Automatic` deixa o Slint criar instance/adapter/device wgpu; o caminho
    // zero-copy do M3 extrai o VkDevice cru desse device via `as_hal`. Por ora (T0) o transporte de
    // frame continua por cópia-CPU (`read_to_image`) — esta tarefa só de-risca a troca de renderer e
    // a coexistência surfman/GL (Servo) + wgpu/Vulkan (Slint) na mesma janela (classe do L-004).
    slint::BackendSelector::new()
        .require_wgpu_28(slint::wgpu_28::WGPUConfiguration::default())
        .select()?;
    eprintln!("[m3] backend Slint: femtovg sobre wgpu/Vulkan (ADR-0005)");

    let app = MainWindow::new()?;
    app.set_frame(placeholder_frame(1024, 768));
    if let Ok(url) = home_page_url() {
        app.set_page_url(url.to_string().into());
    }

    let weak = app.as_weak();
    let runtime: Rc<RefCell<Option<Runtime>>> = Rc::new(RefCell::new(None));
    let sink = Rc::new(Embedder {
        dirty: Cell::new(false),
        app: app.as_weak(),
    });
    let logged = Rc::new(Cell::new(false));
    // Tamanho físico da área web. Fallback inicial; o `changed width/height` do `.slint` corrige
    // para o valor real durante o layout (antes do init lazy do Servo).
    let web_size = Rc::new(Cell::new(dpi::PhysicalSize::new(1024_u32, 744_u32)));

    wire_chrome(&app, &runtime, &web_size);

    // Dirige o Servo e bombeia frames. O runtime é montado LAZY aqui (e não no
    // `set_rendering_notifier`) para criar o contexto GL do Servo FORA do setup do renderer do
    // Slint, evitando a colisão de `make_current` que corrompia o GL (ver doc do módulo).
    let timer = Timer::default();
    let tick_runtime = runtime;
    let tick_sink = sink;
    let tick_weak = weak;
    let tick_logged = logged;
    let tick_web_size = web_size;
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
            match init_runtime(app.window(), tick_sink.clone(), tick_web_size.get()) {
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

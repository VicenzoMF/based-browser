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
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod gpu_bridge;
mod input;
mod persist;

use euclid::Scale;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{
    CookieSource, CreateNewWebViewRequest, DeviceIntRect, EventLoopWaker, LoadStatus,
    OffscreenRenderingContext, Opts, RenderingContext, Servo, ServoBuilder, WebView,
    WebViewBuilder, WebViewDelegate, WebViewId, WindowRenderingContext,
};
use slint::wgpu_28::wgpu;
use slint::{
    ComponentHandle, Image, Model, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode, VecModel,
};
use url::Url;

// M4 (ADR-0007): o chrome saiu da macro inline grande para `ui/app.slint` (LSP/preview, e p/ crescer
// no M4: abas + favoritos + histórico), MAS continua entrando pela macro `slint::slint!` — agora só
// re-exportando o componente do arquivo. Caminho relativo ao `.rs` (toolchain ≥1.88; temos 1.92).
// NÃO usamos `build.rs`/`include_modules!()` de propósito: aquele caminho injeta o `app.rs` gerado
// como FONTE do crate, e o código gerado usa `.unwrap()`/`.expect()` à vontade → trombaria com os
// lints `deny` do projeto; a expansão da macro inline (crate externo) é isenta do clippy. Ver ADR-0007.
slint::slint!(export { MainWindow, TabInfo, BookmarkInfo, HistoryItem } from "../ui/app.slint";);

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

/// Quantos ticks (~16 ms) sem atividade antes de o loop entrar em baixa frequência (≈0,5 s).
const IDLE_ACTIVE_TICKS: u32 = 30;
/// Em baixa frequência (ocioso), spina o Servo 1 a cada N ticks (~10 Hz em vez de ~60 Hz).
const IDLE_SPIN_EVERY: u32 = 6;

/// `EventLoopWaker` real (M3/T6): o Servo chama `wake()` — de qualquer thread — quando tem trabalho
/// a fazer (frame novo, rede, timers da página). Em vez de o `Timer` spinar `spin_event_loop`
/// incondicionalmente a ~60 Hz (CPU ocioso), o waker marca `pending`; o loop spina a 60 Hz enquanto
/// há atividade e cai p/ ~10 Hz quando ocioso, voltando a 60 Hz IMEDIATAMENTE ao ser acordado (ou ao
/// receber input). `Send`+`Sync` via `Arc<AtomicBool>` (o Servo o usa em múltiplas threads).
#[derive(Clone)]
struct ServoWaker {
    pending: Arc<AtomicBool>,
}

impl EventLoopWaker for ServoWaker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }
    fn wake(&self) {
        self.pending.store(true, Ordering::Release);
    }
}

/// `WebViewDelegate`: ponte do Servo → estado POR-ABA (M4, ADR-0007). Cada callback do Servo carrega
/// a `WebView` que o disparou; roteamos por `webview.id()` para o [`TabState`] da aba certa e
/// atualizamos seus campos (interior-mutáveis). NÃO escrevemos no chrome aqui nem tocamos o `app`:
/// marcamos `chrome_dirty` e o LOOP re-sincroniza a aba ATIVA → propriedades do Slint (centraliza as
/// escritas de UI no loop, fora do `spin_event_loop`).
///
/// **Invariante de borrow:** durante `spin_event_loop` o loop segura um borrow IMUTÁVEL do `manager`;
/// aqui só fazemos borrow IMUTÁVEL (achar a aba). `data` é um `RefCell` separado → `borrow_mut`
/// (`record_visit`) não colide. O `manager` é `Weak` p/ não formar ciclo Rc
/// (webview → delegate → Embedder → manager → webview).
struct Embedder {
    /// Estado de persistência (favoritos/histórico). `RefCell` separado do `manager`.
    data: Rc<RefCell<persist::AppData>>,
    /// Acesso (fraco) ao `TabManager` p/ achar a aba que disparou o callback. Só borrow imutável.
    manager: Weak<RefCell<Option<TabManager>>>,
    /// Sinaliza ao loop que o chrome (props da aba ativa) precisa ser re-sincronizado.
    chrome_dirty: Rc<Cell<bool>>,
    /// Abas pedidas por `window.open`/`target=_blank` (`request_create_new`), construídas no delegate
    /// mas AINDA não registradas no `TabManager` — adicioná-las exigiria `borrow_mut` durante o
    /// `spin_event_loop` (reentrante). O loop drena esta fila pós-spin ([`integrate_pending_tabs`]).
    pending_new: Rc<RefCell<Vec<PendingTab>>>,
}

/// Uma aba construída por `request_create_new` (window.open), aguardando integração no `TabManager`.
struct PendingTab {
    webview: WebView,
    context: Rc<OffscreenRenderingContext>,
}

impl Embedder {
    /// Acha o [`TabState`] da aba cujo id é `id` (borrow imutável do manager; clona o `Rc`). `None` se
    /// o manager ainda não subiu ou a aba já foi fechada.
    fn state_for(&self, id: WebViewId) -> Option<Rc<TabState>> {
        let cell = self.manager.upgrade()?;
        let guard = cell.borrow();
        let manager = guard.as_ref()?;
        manager
            .tabs
            .iter()
            .find(|tab| tab.webview.id() == id)
            .map(|tab| Rc::clone(&tab.state))
    }
}

impl WebViewDelegate for Embedder {
    fn notify_new_frame_ready(&self, webview: WebView) {
        // Marca a aba que produziu o frame. O loop só bombeia a ATIVA — frames de abas de fundo ficam
        // marcados e são pintados quando a aba vira ativa (set_active força um pump).
        if let Some(state) = self.state_for(webview.id()) {
            state.dirty.set(true);
        }
    }

    fn notify_load_status_changed(&self, webview: WebView, status: LoadStatus) {
        if let Some(state) = self.state_for(webview.id()) {
            state.loading.set(status != LoadStatus::Complete);
            state.can_go_back.set(webview.can_go_back());
            state.can_go_forward.set(webview.can_go_forward());
        }
        self.chrome_dirty.set(true);
    }

    fn notify_url_changed(&self, webview: WebView, url: Url) {
        if let Some(state) = self.state_for(webview.id()) {
            *state.url.borrow_mut() = url.to_string();
            state.can_go_back.set(webview.can_go_back());
            state.can_go_forward.set(webview.can_go_forward());
        }
        // M4: registra a visita no histórico (persistido). O título pode não ter chegado ainda (vem
        // depois via `notify_page_title_changed`); o dedup consecutivo de `record_visit` atualiza a
        // entrada quando a mesma URL reaparecer com título.
        let title = webview.page_title().unwrap_or_default();
        self.data.borrow_mut().record_visit(url.as_str(), &title);
        self.chrome_dirty.set(true);
    }

    fn notify_history_changed(&self, webview: WebView, _entries: Vec<Url>, _current: usize) {
        if let Some(state) = self.state_for(webview.id()) {
            state.can_go_back.set(webview.can_go_back());
            state.can_go_forward.set(webview.can_go_forward());
        }
        self.chrome_dirty.set(true);
    }

    fn notify_page_title_changed(&self, webview: WebView, title: Option<String>) {
        if let Some(state) = self.state_for(webview.id()) {
            *state.title.borrow_mut() = title.unwrap_or_default();
        }
        self.chrome_dirty.set(true);
    }

    /// `window.open`/`target=_blank`: o conteúdo pediu uma `WebView` nova. Construímos AQUI (com um
    /// offscreen próprio derivado do pai, que está corrente durante o spin) — guardar o handle vivo é
    /// obrigatório, senão a `WebView` é destruída na hora. Mas adiamos REGISTRÁ-la como aba: empilhamos
    /// na fila `pending_new` (`RefCell` separado) e o loop a integra pós-spin (evita `borrow_mut` do
    /// `TabManager` reentrante durante `spin_event_loop`).
    fn request_create_new(&self, parent: WebView, request: CreateNewWebViewRequest) {
        let Some(cell) = self.manager.upgrade() else {
            return;
        };
        // Pai + tamanho da aba ativa (borrow imutável, liberado antes de mexer no GL).
        let (parent_ctx, size) = {
            let guard = cell.borrow();
            let Some(manager) = guard.as_ref() else {
                return;
            };
            let size = manager.active_tab().map_or_else(
                || dpi::PhysicalSize::new(1024, 700),
                |tab| {
                    let s = tab.context.size2d();
                    dpi::PhysicalSize::new(s.width, s.height)
                },
            );
            (manager.parent.clone(), size)
        };
        if let Err(e) = parent_ctx.make_current() {
            eprintln!("[m4] window.open: make_current falhou: {e:?}");
        }
        let context = Rc::new(parent_ctx.offscreen_context(size));
        let webview = request
            .builder(context.clone())
            .delegate(parent.delegate())
            .hidpi_scale_factor(parent.hidpi_scale_factor())
            .build();
        self.pending_new
            .borrow_mut()
            .push(PendingTab { webview, context });
        eprintln!("[m4] window.open: nova aba enfileirada (integra no próximo tick)");
        self.chrome_dirty.set(true);
    }
}

/// Handles wgpu/Vulkan do Slint, capturados via `set_rendering_notifier` (`GraphicsAPI::WGPU28`).
/// Clonáveis (Arc internamente); usados pela ponte GPU (`gpu_bridge`) p/ extrair os handles Vulkan
/// crus e embrulhar a imagem compartilhada como `wgpu::Texture`.
#[derive(Clone)]
struct WgpuCtx {
    instance: wgpu::Instance,
    device: wgpu::Device,
}

/// Estado observável POR-ABA (M4, ADR-0007): escrito pelo `Embedder` (roteado por id) durante o
/// `spin_event_loop` e lido pelo loop p/ refletir no chrome quando a aba é a ATIVA. Tudo
/// interior-mutável p/ o delegate atualizar sem `borrow_mut` do `TabManager` — preserva o invariante
/// anti-reentrância do M2/M3.
#[derive(Default)]
struct TabState {
    /// Há frame novo desta aba (lido pelo loop só p/ a aba ativa — abas de fundo não são pintadas).
    dirty: Cell<bool>,
    url: RefCell<String>,
    title: RefCell<String>,
    loading: Cell<bool>,
    can_go_back: Cell<bool>,
    can_go_forward: Cell<bool>,
}

/// Uma aba: a `WebView` do Servo + seu `OffscreenRenderingContext` (FBO PRÓPRIO, derivado do mesmo
/// `WindowRenderingContext` pai) + o estado observável. Cada aba retém o último frame no próprio FBO →
/// trocar de aba é instantâneo (a ponte GPU só muda a ORIGEM do blit).
struct Tab {
    webview: WebView,
    context: Rc<OffscreenRenderingContext>,
    state: Rc<TabState>,
}

/// Motor multi-aba (M4, ADR-0007): UM `Servo`, N `Tab`s, índice da ativa. O `parent` é o único
/// `WindowRenderingContext` (deriva o offscreen de cada aba — todas compartilham o contexto GL do
/// surfman — e provê o `get_proc_address` das entry-points `*EXT`). A ponte GPU (`bridge`) é ÚNICA e
/// compartilhada: só a aba ATIVA é pintada e blitada (trocar de aba = trocar o FBO de origem do blit).
/// Abas de fundo ficam `set_throttled(true)` e NÃO entram no `pump`.
struct TabManager {
    tabs: Vec<Tab>,
    active: usize,
    servo: Servo,
    parent: Rc<WindowRenderingContext>,
    /// Device wgpu do Slint (compartilhado com o `set_rendering_notifier`); `None` até ser capturado.
    wgpu: Rc<RefCell<Option<WgpuCtx>>>,
    /// Textura GPU compartilhada (M3), única p/ todas as abas. `None` até ser criada sob demanda.
    bridge: RefCell<Option<gpu_bridge::SharedFrameTexture>>,
    /// Entry-points GL `*EXT` já carregadas (uma vez).
    gl_loaded: Cell<bool>,
    /// Interop GPU desabilitado p/ esta sessão (fallback permanente p/ cópia-CPU).
    gpu_disabled: Cell<bool>,
    /// Flag do waker real (T6): o `ServoWaker` marca `true` quando o Servo tem trabalho; o loop o lê
    /// p/ spinar sob demanda (e os handlers de input o marcam p/ responsividade imediata).
    pending: Arc<AtomicBool>,
}

impl TabManager {
    /// A aba ativa (sempre válida por construção; `None` só num estado degenerado sem abas).
    fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    /// Abre uma aba nova carregando `url`, com seu próprio `OffscreenRenderingContext` do tamanho
    /// `web_size`, ligando o delegate `sink`. Se `activate`, torna-a a ativa (show+focus, throttle das
    /// outras). Devolve o índice da nova aba. Criar o offscreen faz chamadas GL → garante o contexto
    /// pai corrente antes (território do L-004 ao abrir abas em runtime).
    fn open_tab(
        &mut self,
        web_size: dpi::PhysicalSize<u32>,
        scale: f32,
        url: Url,
        sink: &Rc<Embedder>,
        activate: bool,
    ) -> usize {
        if let Err(e) = self.parent.make_current() {
            eprintln!("[m4] make_current antes de abrir aba falhou: {e:?}");
        }
        let web_physical = dpi::PhysicalSize::new(web_size.width.max(1), web_size.height.max(1));
        let context = Rc::new(self.parent.offscreen_context(web_physical));
        let url_string = url.to_string();
        let webview = WebViewBuilder::new(&self.servo, context.clone())
            .url(url)
            .hidpi_scale_factor(Scale::new(scale))
            .delegate(sink.clone())
            .build();
        let state = Rc::new(TabState::default());
        *state.url.borrow_mut() = url_string;
        let index = self.tabs.len();
        self.tabs.push(Tab {
            webview,
            context,
            state,
        });
        if activate {
            self.set_active(index);
        }
        index
    }

    /// Torna a aba `index` a ativa: mostra/foca ela e esconde/throttla as demais (economia de CPU/GPU
    /// nas abas de fundo). Marca a nova ativa como suja p/ forçar um pump. No-op se `index` fora de faixa.
    /// `show()`+`focus()` são obrigatórios na ativa (sem `show()` a pipeline fica em branco — L-004).
    fn set_active(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.active = index;
        for (i, tab) in self.tabs.iter().enumerate() {
            if i == index {
                tab.webview.set_throttled(false);
                tab.webview.show();
                tab.webview.focus();
                tab.state.dirty.set(true);
            } else {
                tab.webview.hide();
                tab.webview.set_throttled(true);
            }
        }
    }

    /// Fecha a aba `index` (dropar o `Tab` → `Drop` envia `CloseWebView`). RECUSA fechar a última aba
    /// (mantém o browser sempre usável e a sessão não-vazia). Recalcula o índice ativo e reativa a
    /// aba resultante. Devolve `true` se fechou.
    fn close_tab(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return false;
        }
        self.tabs.remove(index); // drop → CloseWebView + remove_webview no painter
        let new_active = if self.active > index {
            self.active - 1
        } else {
            self.active.min(self.tabs.len() - 1)
        };
        self.set_active(new_active);
        true
    }
}

/// Cria o `Servo` + a PRIMEIRA aba (uma `WebView` com seu `OffscreenRenderingContext`, derivado de um
/// único `WindowRenderingContext` pai = a superfície GL da janela do Slint). Ver [`TabManager`].
fn init_manager(
    window: &slint::Window,
    sink: &Rc<Embedder>,
    web_size: dpi::PhysicalSize<u32>,
    wgpu: Rc<RefCell<Option<WgpuCtx>>>,
) -> Result<TabManager, String> {
    let provider = window.window_handle();
    let display_handle = provider
        .display_handle()
        .map_err(|e| format!("display_handle: {e}"))?;
    let window_handle = provider
        .window_handle()
        .map_err(|e| format!("window_handle: {e}"))?;

    // O contexto PAI cobre a janela inteira (superfície GL da janela); cada aba deriva dele um offscreen
    // dimensionado pela ÁREA WEB (exclui a toolbar), o que torna o mapeamento de coordenadas identidade.
    let size = window.size();
    let window_physical = dpi::PhysicalSize::new(size.width.max(1), size.height.max(1));

    let parent = Rc::new(
        WindowRenderingContext::new(display_handle, window_handle, window_physical)
            .map_err(|e| format!("WindowRenderingContext::new: {e:?}"))?,
    );

    // Waker real (T6): começa `true` p/ o 1º tick pós-init já spinar e carregar a página.
    let pending = Arc::new(AtomicBool::new(true));
    let mut builder = ServoBuilder::default().event_loop_waker(Box::new(ServoWaker {
        pending: pending.clone(),
    }));
    // M6 (ADR-0009): LIGA a persistência de cookies + Web Storage. O Servo passa `opts.config_dir`
    // p/ `new_resource_threads` (cookies) E `new_storage_threads` (local/sessionStorage); com
    // `temporary_storage=false` (default, via `..Opts::default()`) os dados sobrevivem ao restart.
    // Mexida MÍNIMA na API do Servo (embedding fino, L-001): 1 ponto, aditivo ao builder — NÃO
    // reorganiza a ordem de init (o contexto GL segue lazy, L-004). Sem `config_dir` disponível,
    // cai no default (sem persistência) em vez de falhar.
    if let Some(dir) = persist::servo_config_dir() {
        builder = builder.opts(Opts {
            config_dir: Some(dir),
            ..Opts::default()
        });
    }
    let servo = builder.build();
    servo.setup_logging();

    let mut manager = TabManager {
        tabs: Vec::new(),
        active: 0,
        servo,
        parent,
        wgpu,
        bridge: RefCell::new(None),
        gl_loaded: Cell::new(false),
        gpu_disabled: Cell::new(false),
        pending,
    };
    // M4 (T7): restaura as abas da sessão salva; se não houver (ou nenhuma URL válida), abre a home.
    if !restore_session(&mut manager, web_size, window.scale_factor(), sink) {
        manager.open_tab(
            web_size,
            window.scale_factor(),
            home_page_url()?,
            sink,
            true,
        );
    }
    // M5: hook do harness de medição de footprint — garante N abas da MESMA URL p/ medir o custo
    // marginal por-aba. No-op sem a env (uma aba). Só `scripts/m5` usa (ver ADR-0008).
    open_extra_measurement_tabs(&mut manager, web_size, window.scale_factor(), sink)?;
    Ok(manager)
}

/// M5 (harness de footprint): se `BASEDBROWSER_OPEN_TABS=N` (N > nº de abas atual) estiver setado,
/// abre abas extras (de fundo) da home/URL resolvida até totalizar N — para medir o custo marginal
/// por-aba (`scripts/m5/measure.sh`). As abas extras carregam via o `spin_event_loop` global (o
/// `throttle` só pausa a PINTURA, não a carga/layout). No-op sem a env, com N <= nº de abas, ou URL
/// inválida. Reusa `BASEDBROWSER_URL`; combine com `BASEDBROWSER_EXIT_AFTER_MS` p/ um run limpo.
fn open_extra_measurement_tabs(
    manager: &mut TabManager,
    web_size: dpi::PhysicalSize<u32>,
    scale: f32,
    sink: &Rc<Embedder>,
) -> Result<(), String> {
    let Some(target) = std::env::var("BASEDBROWSER_OPEN_TABS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > manager.tabs.len())
    else {
        return Ok(());
    };
    let url = home_page_url()?;
    let extra = target - manager.tabs.len();
    for _ in 0..extra {
        manager.open_tab(web_size, scale, url.clone(), sink, false);
    }
    eprintln!(
        "[m5] BASEDBROWSER_OPEN_TABS={target}: abriu {extra} aba(s) extra(s) (total {})",
        manager.tabs.len()
    );
    Ok(())
}

/// Restaura as abas da sessão salva (`session.json`): abre cada URL como uma aba (de fundo) e ativa a
/// que estava ativa. Devolve `true` se abriu ao menos uma aba (senão o chamador abre a home). Cada aba
/// nasce escondida/throttled; o `set_active` final mostra/foca a ativa.
fn restore_session(
    manager: &mut TabManager,
    web_size: dpi::PhysicalSize<u32>,
    scale: f32,
    sink: &Rc<Embedder>,
) -> bool {
    let Some(session) = persist::load_session().filter(|s| !s.tabs.is_empty()) else {
        return false;
    };
    let mut opened = 0usize;
    for raw in &session.tabs {
        if let Some(url) = parse_user_url(raw) {
            manager.open_tab(web_size, scale, url, sink, false);
            opened += 1;
        }
    }
    if opened == 0 {
        return false;
    }
    let active = session.active.min(opened - 1);
    manager.set_active(active);
    eprintln!("[m4] sessão restaurada: {opened} aba(s), ativa={active}");
    true
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
    // Override opcional da URL inicial (benchmark/teste reproduzível): aceita URL absoluta ou
    // `file://...` (mesma normalização da barra). Ex.: `BASEDBROWSER_URL=file:///tmp/m3-bench.html`.
    if let Some(raw) = std::env::var_os("BASEDBROWSER_URL") {
        if let Some(url) = raw.to_str().and_then(parse_user_url) {
            return Ok(url);
        }
        eprintln!(
            "[m3] BASEDBROWSER_URL invalida, ignorando: {}",
            raw.display()
        );
    }

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

/// Bombeia um frame. `paint()` renderiza no FBO offscreen do Servo. Depois, **M3 (ADR-0005):** tenta
/// o caminho GPU zero-copy (blit do FBO offscreen para a textura compartilhada, daí um `slint::Image`
/// que a referencia); se o interop não estiver disponível, cai no fallback de **cópia-CPU** (M1/M2,
/// via `read_to_image`). `BASEDBROWSER_DUMP_FRAME=<path>` salva o frame em PNG (evidência, uma vez).
fn pump_frame(manager: &TabManager, weak: &slint::Weak<MainWindow>, logged: &Cell<bool>) {
    let Some(tab) = manager.active_tab() else {
        return;
    };
    tab.webview.paint();
    if tab.context.make_current().is_err() {
        return;
    }
    let handled = !manager.gpu_disabled.get() && pump_frame_gpu(manager, tab, weak);
    if !handled {
        pump_frame_cpu(tab, weak);
    }
    // Evidência (opt-in por env): dump a cada frame sobrescrevendo — o arquivo final é um frame já
    // renderizado (a 1ª rajada de frames de uma página pode estar em branco antes de pintar). Loga só
    // uma vez. Fora do caminho quente normal (só roda quando `BASEDBROWSER_DUMP_FRAME` está setado).
    if let Ok(path) = std::env::var("BASEDBROWSER_DUMP_FRAME") {
        let first = !logged.replace(true);
        dump_source(tab, &path, first);
        if let Some(bridge) = manager.bridge.borrow().as_ref() {
            bridge.dump_shared(&path, first);
        }
    }
}

/// Caminho GPU zero-copy (M3). Cria a ponte sob demanda assim que o device wgpu do Slint é capturado.
/// Faz o blit do FBO offscreen do Servo para a textura compartilhada (flip + sync dentro de
/// `blit_from`) e entrega um `slint::Image` que a referencia. Devolve `true` se entregou o frame;
/// `false` p/ cair no fallback CPU. Falha ao criar a ponte desabilita o GPU para a sessão (loga uma vez).
fn pump_frame_gpu(manager: &TabManager, tab: &Tab, weak: &slint::Weak<MainWindow>) -> bool {
    let size = tab.context.size2d();
    // (Re)cria a ponte se ainda não existe ou se o offscreen da ABA ATIVA mudou de tamanho (resize). O
    // contexto do Servo já está corrente (`pump_frame` chamou `make_current`), então é seguro mexer no
    // GL aqui. A ponte é ÚNICA p/ todas as abas (mesmo tamanho = área web) — trocar de aba não a recria.
    let needs_new = match manager.bridge.borrow().as_ref() {
        None => true,
        Some(bridge) => bridge.size() != (size.width, size.height),
    };
    if needs_new {
        // Precisa do device wgpu já capturado pelo `set_rendering_notifier`.
        let captured = manager.wgpu.borrow().clone();
        let Some(ctx) = captured else {
            return false; // ainda não capturado; usa CPU neste frame
        };
        if !manager.gl_loaded.replace(true) {
            // Carrega as entry-points GL `*EXT` via o `get_proc_address` do surfman do Servo.
            let (device, context) = manager.parent.surfman_details();
            gpu_bridge::load_gl_with(|symbol| device.get_proc_address(&context, symbol));
        }
        // Libera a ponte antiga (tamanho velho) antes de criar a nova.
        if let Some(old) = manager.bridge.borrow_mut().take() {
            old.destroy();
        }
        match gpu_bridge::SharedFrameTexture::new(
            &ctx.device,
            &ctx.instance,
            size.width,
            size.height,
        ) {
            Ok(bridge) => {
                eprintln!(
                    "[m3] textura GPU compartilhada criada ({}x{}) — zero-copy ativo",
                    size.width, size.height
                );
                *manager.bridge.borrow_mut() = Some(bridge);
            }
            Err(e) => {
                eprintln!("[m3] interop GPU indisponível, fallback p/ cópia-CPU: {e}");
                manager.gpu_disabled.set(true);
                return false;
            }
        }
    }

    let bridge_ref = manager.bridge.borrow();
    let Some(bridge) = bridge_ref.as_ref() else {
        return false;
    };
    // Origem do blit = FBO do offscreen da ABA ATIVA. `prepare_for_rendering` o liga; lemos o binding.
    tab.context.prepare_for_rendering();
    let src_fbo = gpu_bridge::bound_framebuffer();
    bridge.blit_from(src_fbo);
    match bridge.slint_image() {
        Ok(image) => {
            if let Some(app) = weak.upgrade() {
                app.set_frame(image);
            }
            true
        }
        Err(e) => {
            eprintln!("[m3] falha ao derivar slint::Image da textura GPU: {e}");
            false
        }
    }
}

/// Fallback de cópia-CPU (M1/M2): `read_to_image` (readback GL) → `SharedPixelBuffer` →
/// `Image::from_rgba8`. Usado até o device wgpu ser capturado, ou se o interop GPU falhar.
fn pump_frame_cpu(tab: &Tab, weak: &slint::Weak<MainWindow>) {
    let rect = DeviceIntRect::from_size(tab.context.size2d().to_i32());
    let Some(frame) = tab.context.read_to_image(rect) else {
        return;
    };
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(frame.width(), frame.height());
    buffer.make_mut_bytes().copy_from_slice(frame.as_raw());
    if let Some(app) = weak.upgrade() {
        app.set_frame(Image::from_rgba8(buffer));
    }
}

/// Salva o frame da FONTE (FBO offscreen da aba ativa, via `read_to_image`) em `path`, sobrescrevendo.
/// Evidência/depuração — comparável ao dump da textura GPU compartilhada (`.gpu.png`). `log` controla
/// se emite o eprintln (uma vez). Fora do caminho quente (só quando `BASEDBROWSER_DUMP_FRAME` setado).
fn dump_source(tab: &Tab, path: &str, log: bool) {
    let rect = DeviceIntRect::from_size(tab.context.size2d().to_i32());
    let Some(frame) = tab.context.read_to_image(rect) else {
        return;
    };
    match frame.save(path) {
        Ok(()) => {
            if log {
                eprintln!(
                    "[m3] frame da fonte salvo em {path} ({}x{})",
                    frame.width(),
                    frame.height()
                );
            }
        }
        Err(e) => eprintln!("[m3] falha ao salvar dump do frame: {e}"),
    }
}

/// Executa `f` com a `WebView` da ABA ATIVA, se o manager já subiu, e marca o waker (resposta imediata
/// à ação do usuário). Borrow IMUTÁVEL e curto; os callbacks do Slint rodam fora do `spin_event_loop`,
/// então nem competem com o borrow do loop. No-op se o manager ainda não existe / não há aba.
fn with_active_webview(manager: &Rc<RefCell<Option<TabManager>>>, f: impl FnOnce(&WebView)) {
    if let Some(m) = manager.borrow().as_ref() {
        if let Some(tab) = m.active_tab() {
            f(&tab.webview);
        }
        wake(m);
    }
}

/// Registra os callbacks do chrome (Slint) que encaminham input e navegação à ABA ATIVA. Cada handler
/// captura um clone do `manager` compartilhado; se o evento chegar antes do init lazy, é ignorado com
/// segurança. Input/navegação sempre vão para a aba ativa (M4).
fn wire_chrome(
    app: &MainWindow,
    manager: &Rc<RefCell<Option<TabManager>>>,
    web_size: &Rc<Cell<dpi::PhysicalSize<u32>>>,
) {
    // Resize: o `.slint` dispara `web-resized` com o tamanho FÍSICO da área web quando o layout muda.
    // Guardamos para o init lazy e, se o manager já existe, redimensionamos o offscreen de CADA aba
    // (cada uma tem seu FBO) via `webview.resize` — assim, ao trocar p/ uma aba de fundo após o
    // resize, ela já está no tamanho certo. NÃO tocamos o `WindowRenderingContext` pai (L-004). A
    // ponte GPU é recriada no novo tamanho por `pump_frame_gpu`.
    let mgr = manager.clone();
    let ws = web_size.clone();
    app.on_web_resized(move |w, h| {
        let size = dpi::PhysicalSize::new(physical_px(w), physical_px(h));
        ws.set(size);
        if let Some(m) = mgr.borrow().as_ref() {
            for tab in &m.tabs {
                tab.webview.resize(size);
            }
            wake(m);
        }
    });

    let mgr = manager.clone();
    app.on_forward_pointer(move |x, y, kind, button| {
        with_active_webview(&mgr, |wv| {
            wv.notify_input_event(input::pointer_input_event(x, y, kind, button));
        });
    });

    let mgr = manager.clone();
    app.on_forward_scroll(move |x, y, dx, dy| {
        with_active_webview(&mgr, |wv| {
            wv.notify_scroll_event(input::scroll_delta(dx, dy), input::device_point(x, y));
        });
    });

    let mgr = manager.clone();
    app.on_forward_key(move |text, pressed, ctrl, shift, alt, meta, repeat| {
        with_active_webview(&mgr, |wv| {
            wv.notify_input_event(input::key_input_event(
                text.as_str(),
                pressed,
                ctrl,
                shift,
                alt,
                meta,
                repeat,
            ));
        });
    });

    let mgr = manager.clone();
    app.on_load_url(move |text| match parse_user_url(text.as_str()) {
        Some(url) => with_active_webview(&mgr, |wv| wv.load(url.clone())),
        None => eprintln!("[m2] URL invalida ignorada: {text:?}"),
    });

    let mgr = manager.clone();
    app.on_go_back(move || {
        with_active_webview(&mgr, |wv| {
            if wv.can_go_back() {
                wv.go_back(1);
            }
        });
    });

    let mgr = manager.clone();
    app.on_go_forward(move || {
        with_active_webview(&mgr, |wv| {
            if wv.can_go_forward() {
                wv.go_forward(1);
            }
        });
    });

    let mgr = manager.clone();
    app.on_reload(move || {
        with_active_webview(&mgr, WebView::reload);
    });
}

/// Reflete o estado da ABA ATIVA nas propriedades do chrome (Slint). Chamado pelo loop quando o
/// `Embedder` marcou `chrome_dirty` — centraliza as escritas de UI no loop (fora do `spin_event_loop`).
fn sync_chrome(app: &MainWindow, manager: &TabManager) {
    let Some(tab) = manager.active_tab() else {
        return;
    };
    let state = &tab.state;
    app.set_page_url(state.url.borrow().as_str().into());
    app.set_loading(state.loading.get());
    app.set_can_go_back(state.can_go_back.get());
    app.set_can_go_forward(state.can_go_forward.get());
    let title = state.title.borrow();
    let shown = if title.is_empty() {
        "BasedBrowser".to_string()
    } else {
        format!("{title} — BasedBrowser")
    };
    app.set_window_title(shown.into());
}

/// Marca o `pending` do waker (T6): força o loop a spinar no próximo tick (60 Hz), p/ a ação do
/// usuário (input/navegação/resize) ser processada imediatamente mesmo se o loop estava ocioso.
fn wake(manager: &TabManager) {
    manager.pending.store(true, Ordering::Release);
}

/// Rótulo de uma aba na barra: o título da página, ou (enquanto não chega) o host da URL; vazio se
/// nem isso (ex.: `file://`) — aí o `.slint` exibe "Nova aba".
fn tab_label(title: &str, url: &str) -> String {
    if !title.is_empty() {
        return title.to_owned();
    }
    Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Reconstrói o `VecModel<TabInfo>` (a barra de abas do Slint) a partir do `TabManager` (fonte da
/// verdade). Chamado após mudanças estruturais (abrir/fechar/trocar) e quando títulos mudam.
fn rebuild_tabs_model(model: &VecModel<TabInfo>, manager: &TabManager) {
    let rows: Vec<TabInfo> = manager
        .tabs
        .iter()
        .enumerate()
        .map(|(i, tab)| TabInfo {
            title: tab_label(&tab.state.title.borrow(), &tab.state.url.borrow()).into(),
            active: i == manager.active,
        })
        .collect();
    model.set_vec(rows);
}

/// Registra os callbacks de ciclo de vida das abas (novo/selecionar/fechar). Mutam o `TabManager`
/// (`borrow_mut` — rodam FORA do `spin_event_loop`, sem reentrância), reconstroem a barra de abas e
/// marcam `chrome_dirty` p/ o loop re-sincronizar a aba ativa no chrome.
fn wire_tabs(
    app: &MainWindow,
    manager: &Rc<RefCell<Option<TabManager>>>,
    tabs_model: &Rc<VecModel<TabInfo>>,
    sink: &Rc<Embedder>,
    web_size: &Rc<Cell<dpi::PhysicalSize<u32>>>,
    chrome_dirty: &Rc<Cell<bool>>,
) {
    let (mgr, model, embedder, ws, cd) = (
        manager.clone(),
        tabs_model.clone(),
        sink.clone(),
        web_size.clone(),
        chrome_dirty.clone(),
    );
    app.on_new_tab(move || {
        if let Some(tm) = mgr.borrow_mut().as_mut() {
            let scale = tm
                .active_tab()
                .map_or(1.0, |tab| tab.webview.hidpi_scale_factor().get());
            match home_page_url() {
                Ok(url) => {
                    tm.open_tab(ws.get(), scale, url, &embedder, true);
                }
                Err(e) => eprintln!("[m4] nova aba: URL inicial inválida: {e}"),
            }
            rebuild_tabs_model(&model, tm);
            cd.set(true);
            wake(tm);
        }
    });

    let (mgr, model, cd) = (manager.clone(), tabs_model.clone(), chrome_dirty.clone());
    app.on_select_tab(move |index| {
        if let Some(tm) = mgr.borrow_mut().as_mut() {
            tm.set_active(usize::try_from(index).unwrap_or(0));
            rebuild_tabs_model(&model, tm);
            cd.set(true);
            wake(tm);
        }
    });

    let (mgr, model, cd) = (manager.clone(), tabs_model.clone(), chrome_dirty.clone());
    app.on_close_tab(move |index| {
        if let Some(tm) = mgr.borrow_mut().as_mut() {
            if tm.close_tab(usize::try_from(index).unwrap_or(0)) {
                rebuild_tabs_model(&model, tm);
                cd.set(true);
                wake(tm);
            }
        }
    });
}

/// Filtra o histórico por `query` (substring case-insensitive em url/título), mais recente primeiro,
/// dedup por url, até `limit` itens. Query vazia = histórico recente sem filtro.
fn filtered_history(data: &persist::AppData, query: &str, limit: usize) -> Vec<HistoryItem> {
    let q = query.to_lowercase();
    let mut seen = std::collections::HashSet::new();
    data.history
        .iter()
        .rev()
        .filter(|entry| {
            q.is_empty()
                || entry.url.to_lowercase().contains(&q)
                || entry.title.to_lowercase().contains(&q)
        })
        .filter(|entry| seen.insert(entry.url.clone()))
        .take(limit)
        .map(|entry| HistoryItem {
            title: entry.title.as_str().into(),
            url: entry.url.as_str().into(),
        })
        .collect()
}

/// Configura o painel de histórico + o autocomplete da barra e registra seus callbacks. Os dois
/// `VecModel` (painel e sugestões) são preenchidos sob demanda (abrir painel / digitar / buscar).
fn setup_history(
    app: &MainWindow,
    manager: &Rc<RefCell<Option<TabManager>>>,
    data: &Rc<RefCell<persist::AppData>>,
) {
    let history_model: Rc<VecModel<HistoryItem>> = Rc::new(VecModel::default());
    let suggestions_model: Rc<VecModel<HistoryItem>> = Rc::new(VecModel::default());
    app.set_history(history_model.clone().into());
    app.set_suggestions(suggestions_model.clone().into());

    // Abrir/atualizar o painel = histórico recente completo (até 200).
    let (dat, mdl) = (data.clone(), history_model.clone());
    app.on_toggle_history(move || {
        mdl.set_vec(filtered_history(&dat.borrow(), "", 200));
    });

    // Buscar no painel.
    let (dat, mdl) = (data.clone(), history_model.clone());
    app.on_search_history(move |query| {
        mdl.set_vec(filtered_history(&dat.borrow(), query.as_str(), 200));
    });

    // Autocomplete: até 6 sugestões; texto vazio limpa.
    let (dat, mdl) = (data.clone(), suggestions_model.clone());
    app.on_url_edited(move |text| {
        let rows = if text.is_empty() {
            Vec::new()
        } else {
            filtered_history(&dat.borrow(), text.as_str(), 6)
        };
        mdl.set_vec(rows);
    });

    // Abrir uma entrada do painel = carregar a URL na aba ativa + fechar o painel.
    let (mgr, weak) = (manager.clone(), app.as_weak());
    app.on_open_history(move |index| {
        if let Some(app) = weak.upgrade() {
            if let Some(item) = row_url(&app.get_history(), index) {
                if let Some(parsed) = parse_user_url(&item) {
                    with_active_webview(&mgr, |wv| wv.load(parsed.clone()));
                }
            }
            app.set_history_open(false);
        }
    });

    // Abrir uma sugestão = carregar a URL na aba ativa + limpar as sugestões.
    let (mgr, sug) = (manager.clone(), suggestions_model);
    let weak = app.as_weak();
    app.on_open_suggestion(move |index| {
        if let Some(app) = weak.upgrade() {
            if let Some(item) = row_url(&app.get_suggestions(), index) {
                if let Some(parsed) = parse_user_url(&item) {
                    with_active_webview(&mgr, |wv| wv.load(parsed.clone()));
                }
            }
        }
        sug.set_vec(Vec::new());
    });
}

/// Lê a URL da linha `index` de um model de `HistoryItem` (painel ou sugestões).
fn row_url(model: &slint::ModelRc<HistoryItem>, index: i32) -> Option<String> {
    let idx = usize::try_from(index).ok()?;
    model.row_data(idx).map(|item| item.url.to_string())
}

/// Cria a barra de favoritos: model populado dos favoritos carregados + callbacks.
fn setup_bookmarks(
    app: &MainWindow,
    manager: &Rc<RefCell<Option<TabManager>>>,
    data: &Rc<RefCell<persist::AppData>>,
) {
    let model: Rc<VecModel<BookmarkInfo>> = Rc::new(VecModel::default());
    rebuild_bookmarks_model(&model, &data.borrow());
    app.set_bookmarks(model.clone().into());
    wire_bookmarks(app, manager, data, &model);
}

/// Reconstrói o `VecModel<BookmarkInfo>` (a barra de favoritos do Slint) a partir do `AppData`.
fn rebuild_bookmarks_model(model: &VecModel<BookmarkInfo>, data: &persist::AppData) {
    let rows: Vec<BookmarkInfo> = data
        .bookmarks
        .iter()
        .map(|bm| BookmarkInfo {
            title: bm.title.as_str().into(),
            url: bm.url.as_str().into(),
        })
        .collect();
    model.set_vec(rows);
}

/// Registra os callbacks dos favoritos (★ adiciona a página atual; abrir/remover por índice). Mutam o
/// `AppData` (`borrow_mut` FORA do spin), persistem em disco e reconstroem a barra. Abrir carrega a URL
/// na aba ativa.
fn wire_bookmarks(
    app: &MainWindow,
    manager: &Rc<RefCell<Option<TabManager>>>,
    data: &Rc<RefCell<persist::AppData>>,
    model: &Rc<VecModel<BookmarkInfo>>,
) {
    // ★ adiciona a página da aba ativa (url + título), sem duplicar a mesma URL.
    let (mgr, dat, mdl) = (manager.clone(), data.clone(), model.clone());
    app.on_add_bookmark(move || {
        let Some((url, title)) = active_url_title(&mgr) else {
            return;
        };
        if url.is_empty() {
            return;
        }
        {
            let mut d = dat.borrow_mut();
            if d.bookmarks.iter().any(|bm| bm.url == url) {
                eprintln!("[m4] favorito já existe: {url}");
                return;
            }
            let title = if title.is_empty() { url.clone() } else { title };
            d.bookmarks.push(persist::Bookmark { title, url });
            persist::save_bookmarks(&d.bookmarks);
        }
        rebuild_bookmarks_model(&mdl, &dat.borrow());
    });

    // Abrir um favorito = carregar sua URL na aba ativa.
    let (mgr, dat) = (manager.clone(), data.clone());
    app.on_open_bookmark(move |index| {
        let url = dat
            .borrow()
            .bookmarks
            .get(usize::try_from(index).unwrap_or(usize::MAX))
            .map(|bm| bm.url.clone());
        if let Some(parsed) = url.as_deref().and_then(parse_user_url) {
            with_active_webview(&mgr, |wv| wv.load(parsed.clone()));
        }
    });

    // Remover um favorito (persistido).
    let (dat, mdl) = (data.clone(), model.clone());
    app.on_remove_bookmark(move |index| {
        let idx = usize::try_from(index).unwrap_or(usize::MAX);
        {
            let mut d = dat.borrow_mut();
            if idx < d.bookmarks.len() {
                d.bookmarks.remove(idx);
                persist::save_bookmarks(&d.bookmarks);
            }
        }
        rebuild_bookmarks_model(&mdl, &dat.borrow());
    });
}

/// URL + título da aba ativa (clones), se o manager subiu. Usado p/ adicionar favorito.
fn active_url_title(manager: &Rc<RefCell<Option<TabManager>>>) -> Option<(String, String)> {
    let guard = manager.borrow();
    let tab = guard.as_ref()?.active_tab()?;
    // Vincula os clones a locais p/ os temporários `Ref` dropparem ANTES de `guard` (ordem de drop).
    let url = tab.state.url.borrow().clone();
    let title = tab.state.title.borrow().clone();
    Some((url, title))
}

/// Integra (pós-spin) as abas que `window.open` enfileirou em `pending`: registra cada uma no
/// `TabManager` (com seu `TabState`) e ativa a última. `borrow_mut` do manager é seguro aqui — roda no
/// começo do tick, FORA do `spin_event_loop`. No-op se a fila está vazia.
fn integrate_pending_tabs(
    manager: &Rc<RefCell<Option<TabManager>>>,
    pending: &Rc<RefCell<Vec<PendingTab>>>,
    tabs_model: &Rc<VecModel<TabInfo>>,
    chrome_dirty: &Rc<Cell<bool>>,
) {
    let drained: Vec<PendingTab> = pending.borrow_mut().drain(..).collect();
    if drained.is_empty() {
        return;
    }
    if let Some(tm) = manager.borrow_mut().as_mut() {
        for tab in drained {
            let state = Rc::new(TabState::default());
            if let Some(url) = tab.webview.url() {
                *state.url.borrow_mut() = url.to_string();
            }
            let index = tm.tabs.len();
            tm.tabs.push(Tab {
                webview: tab.webview,
                context: tab.context,
                state,
            });
            tm.set_active(index);
        }
        rebuild_tabs_model(tabs_model, tm);
        chrome_dirty.set(true);
        wake(tm);
    }
}

/// Telemetria do hot path de frame, habilitada por `BASEDBROWSER_BENCH=1` (no-op quando ausente).
/// Mede o tempo de cada `pump_frame` — exatamente o custo que o M3 ataca — e reporta a cada ~1 s:
/// taxa de frames bombeados, média/p95/máx do tempo de pump em ms. A métrica é a MESMA na cópia-CPU
/// (M1/M2) e no caminho GPU (M3), então o "antes vs depois" é comparável (critério de sucesso do M3).
struct FrameBench {
    enabled: bool,
    /// Tempos de pump (ms) acumulados desde o último relatório.
    samples: Vec<f64>,
    total_frames: u64,
    last_report: Instant,
}

impl FrameBench {
    fn new() -> Self {
        let enabled = std::env::var_os("BASEDBROWSER_BENCH").is_some();
        if enabled {
            eprintln!("[bench] habilitado (BASEDBROWSER_BENCH) — medindo o tempo de pump_frame");
        }
        Self {
            enabled,
            samples: Vec::new(),
            total_frames: 0,
            last_report: Instant::now(),
        }
    }

    /// Registra a duração de um `pump_frame` e emite um relatório a cada ~1 s.
    fn record(&mut self, dur: Duration) {
        if !self.enabled {
            return;
        }
        self.total_frames += 1;
        self.samples.push(dur.as_secs_f64() * 1000.0);
        let window = self.last_report.elapsed();
        if window >= Duration::from_secs(1) {
            self.report(window);
            self.samples.clear();
            self.last_report = Instant::now();
        }
    }

    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "telemetria: contagem/índice de percentil; perda de precisão/truncamento irrelevante"
    )]
    fn report(&self, window: Duration) {
        let n = self.samples.len();
        if n == 0 {
            return;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mean = self.samples.iter().sum::<f64>() / n as f64;
        let p95 = sorted[(((n - 1) as f64) * 0.95).round() as usize];
        let max = sorted[n - 1];
        let fps = n as f64 / window.as_secs_f64();
        eprintln!(
            "[bench] pump={fps:.1}/s total={} pump_ms(mean={mean:.2} p95={p95:.2} max={max:.2})",
            self.total_frames
        );
    }
}

/// Captura o device wgpu/Vulkan que o Slint cria, via o rendering notifier (`GraphicsAPI::WGPU28`).
/// Só CLONA os handles (sem tocar GL aqui — evita a colisão do L-004); a ponte GPU é montada depois,
/// no `pump_frame`. Roda na main thread durante o render do Slint. Devolve a célula compartilhada que
/// o runtime lê; fica `None` se o notifier falhar (→ fallback de cópia-CPU).
fn capture_wgpu_device(app: &MainWindow) -> Rc<RefCell<Option<WgpuCtx>>> {
    let cell: Rc<RefCell<Option<WgpuCtx>>> = Rc::new(RefCell::new(None));
    let sink = cell.clone();
    let notifier = app
        .window()
        .set_rendering_notifier(move |state, graphics_api| {
            if matches!(state, slint::RenderingState::RenderingSetup) {
                if let slint::GraphicsAPI::WGPU28 {
                    instance, device, ..
                } = graphics_api
                {
                    let empty = sink.borrow().is_none();
                    if empty {
                        *sink.borrow_mut() = Some(WgpuCtx {
                            instance: instance.clone(),
                            device: device.clone(),
                        });
                        eprintln!("[m3] device wgpu/Vulkan capturado do Slint (RenderingSetup)");
                    }
                }
            }
        });
    if let Err(e) = notifier {
        eprintln!("[m3] set_rendering_notifier falhou ({e:?}); GPU desabilitado, fallback CPU");
    }
    cell
}

/// Monta o `TabManager` de forma LAZY (alguns ticks após o loop subir, FORA do setup do renderer do
/// Slint — ver L-004) quando ainda não existe. Devolve `true` enquanto ainda está inicializando (o
/// chamador deve retornar do tick). Espera `INIT_DELAY_TICKS` p/ o renderer do Slint estabilizar.
fn lazy_init_manager(
    manager: &Rc<RefCell<Option<TabManager>>>,
    sink: &Rc<Embedder>,
    weak: &slint::Weak<MainWindow>,
    web_size: dpi::PhysicalSize<u32>,
    wgpu: &Rc<RefCell<Option<WgpuCtx>>>,
    init_ticks: &Cell<u32>,
) -> bool {
    if manager.borrow().is_some() {
        return false;
    }
    let n = init_ticks.get();
    init_ticks.set(n + 1);
    if n < INIT_DELAY_TICKS {
        return true;
    }
    let Some(app) = weak.upgrade() else {
        return true;
    };
    match init_manager(app.window(), sink, web_size, wgpu.clone()) {
        Ok(m) => {
            eprintln!(
                "[m4] motor multi-aba iniciado ({} aba(s); offscreen GL sobre a janela)",
                m.tabs.len()
            );
            *manager.borrow_mut() = Some(m);
        }
        Err(e) => eprintln!("[m1] FALHA ao iniciar o runtime do Servo: {e}"),
    }
    true
}

/// Inicializa o provedor de cripto (TLS) e força o backend Slint femtovg sobre wgpu/Vulkan (M3,
/// ADR-0005) — ANTES de criar qualquer janela/componente. O caminho zero-copy extrai o `VkDevice` cru
/// do device que o Slint cria (`Automatic`) via `as_hal`.
fn init_backend() -> Result<(), slint::PlatformError> {
    // Provedor de cripto process-wide p/ TLS. `install_default` falha se já houver um — ignorável.
    if rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .is_err()
    {
        eprintln!("[m1] provedor de cripto rustls ja instalado (ok)");
    }
    slint::BackendSelector::new()
        .require_wgpu_28(slint::wgpu_28::WGPUConfiguration::default())
        .select()?;
    eprintln!("[m3] backend Slint: femtovg sobre wgpu/Vulkan (ADR-0005)");
    Ok(())
}

fn main() -> Result<(), slint::PlatformError> {
    init_backend()?;

    let app = MainWindow::new()?;
    app.set_frame(placeholder_frame(1024, 768));
    if let Ok(url) = home_page_url() {
        app.set_page_url(url.to_string().into());
    }

    // M3 (ADR-0005): captura o device wgpu/Vulkan que o Slint cria (ver `capture_wgpu_device`).
    let wgpu_ctx = capture_wgpu_device(&app);

    // M4 (ADR-0007): carrega o estado persistido (favoritos/histórico). A restauração de abas da
    // sessão salva é feita no `init_manager` (T7).
    let app_data = load_persisted_state();

    let weak = app.as_weak();
    let manager: Rc<RefCell<Option<TabManager>>> = Rc::new(RefCell::new(None));
    // Clone p/ salvar a sessão ao sair (o original é movido para o closure do timer).
    let exit_manager = manager.clone();
    // M4: o `Embedder` segura um `Weak` do manager (sem ciclo Rc) p/ rotear callbacks por id, um
    // `chrome_dirty` compartilhado com o loop (que re-sincroniza a aba ativa → props do Slint), e a
    // fila `pending_new` de abas pedidas por window.open (drenada pelo loop).
    let chrome_dirty = Rc::new(Cell::new(true));
    let pending_new: Rc<RefCell<Vec<PendingTab>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::new(Embedder {
        data: app_data.clone(),
        manager: Rc::downgrade(&manager),
        chrome_dirty: chrome_dirty.clone(),
        pending_new: pending_new.clone(),
    });
    let logged = Rc::new(Cell::new(false));
    // Tamanho físico da área web. Fallback inicial; o `changed width/height` do `.slint` corrige
    // para o valor real durante o layout (antes do init lazy do Servo).
    let web_size = Rc::new(Cell::new(dpi::PhysicalSize::new(1024_u32, 744_u32)));

    wire_chrome(&app, &manager, &web_size);

    // M4: barra de abas. `tabs_model` é a view derivada do `TabManager` (fonte da verdade); os
    // callbacks de ciclo de vida a reconstroem, e o loop a atualiza quando títulos mudam.
    let tabs_model: Rc<VecModel<TabInfo>> = Rc::new(VecModel::default());
    app.set_tabs(tabs_model.clone().into());
    wire_tabs(&app, &manager, &tabs_model, &sink, &web_size, &chrome_dirty);

    // M4: barra de favoritos (persistida), populada dos favoritos carregados no start.
    setup_bookmarks(&app, &manager, &app_data);

    // M4: painel de histórico (lista + busca) + autocomplete da barra de URL.
    setup_history(&app, &manager, &app_data);

    // Drivers de evidência (no-op sem as envs respectivas); manter os timers vivos.
    let _drivers = install_evidence_drivers(&app, &manager);

    // Dirige o Servo e bombeia frames. O manager é montado LAZY aqui (e não no
    // `set_rendering_notifier`) para criar o contexto GL do Servo FORA do setup do renderer do
    // Slint, evitando a colisão de `make_current` que corrompia o GL (ver doc do módulo).
    let timer = Timer::default();
    let tick_manager = manager;
    let tick_sink = sink;
    let tick_weak = weak;
    let tick_logged = logged;
    let tick_web_size = web_size;
    let tick_wgpu = wgpu_ctx;
    let tick_chrome_dirty = chrome_dirty;
    let tick_tabs_model = tabs_model;
    let tick_pending_new = pending_new;
    let init_ticks = Cell::new(0u32);
    let idle_ticks = Cell::new(0u32);
    let tick_bench = RefCell::new(FrameBench::new());
    timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
        if lazy_init_manager(
            &tick_manager,
            &tick_sink,
            &tick_weak,
            tick_web_size.get(),
            &tick_wgpu,
            &init_ticks,
        ) {
            return;
        }

        // M4: integra abas abertas por window.open (request_create_new) ANTES do borrow imutável.
        integrate_pending_tabs(
            &tick_manager,
            &tick_pending_new,
            &tick_tabs_model,
            &tick_chrome_dirty,
        );

        let guard = tick_manager.borrow();
        let Some(manager) = guard.as_ref() else {
            return;
        };
        // Waker real (T6): spina a ~60 Hz enquanto há atividade; após `IDLE_ACTIVE_TICKS` ocioso,
        // cai p/ ~10 Hz (spina 1 a cada `IDLE_SPIN_EVERY`), economizando o `spin_event_loop` ocioso.
        // O `wake()` do Servo e os handlers de input marcam `pending` → volta a 60 Hz no próximo tick.
        let woken = manager.pending.swap(false, Ordering::AcqRel);
        let idle = idle_ticks.get();
        if !woken && idle >= IDLE_ACTIVE_TICKS && !idle.is_multiple_of(IDLE_SPIN_EVERY) {
            idle_ticks.set(idle.saturating_add(1));
            return;
        }
        // Torna corrente o contexto da aba ativa (= o pai; todas as abas o compartilham) antes do spin.
        if manager
            .active_tab()
            .is_none_or(|tab| tab.context.make_current().is_err())
        {
            return;
        }
        manager.servo.spin_event_loop();
        // Só a ABA ATIVA é bombeada — frames de abas de fundo ficam marcados e são pintados ao virarem
        // ativas (set_active força um pump). Abas de fundo throttled também produzem menos.
        let produced = manager
            .active_tab()
            .is_some_and(|tab| tab.state.dirty.replace(false));
        if produced {
            let started = Instant::now();
            pump_frame(manager, &tick_weak, &tick_logged);
            tick_bench.borrow_mut().record(started.elapsed());
        }
        // M4: re-sincroniza o chrome (props da aba ativa) + a barra de abas (títulos) se o `Embedder`
        // marcou algo mudado.
        if tick_chrome_dirty.replace(false) {
            if let Some(app) = tick_weak.upgrade() {
                sync_chrome(&app, manager);
            }
            rebuild_tabs_model(&tick_tabs_model, manager);
        }
        idle_ticks.set(if woken || produced {
            0
        } else {
            idle.saturating_add(1)
        });
    });

    // Evidência/teste automatizado: como a captura/fechamento de JANELA é bloqueada no GNOME 46/
    // Wayland, `BASEDBROWSER_EXIT_AFTER_MS=<n>` encerra o loop de forma LIMPA após n ms — assim o
    // caminho de save-on-exit (sessão) roda de verdade num smoke test não-interativo.
    let _exit_timer = install_exit_timer();

    let result = app.run();
    // M4 (T7): ao sair, persiste a sessão (URLs das abas + índice da ativa), restaurada no próximo
    // start (`init_manager`). O histórico já é gravado a cada visita.
    save_session_on_exit(&exit_manager);
    result
}

/// URL `file://` da 2ª página de teste (escrita por [`home_page_url`] no init). Usada pelo driver de
/// evidência das abas.
fn page2_url() -> Option<Url> {
    Url::from_file_path(std::env::temp_dir().join("basedbrowser-page2.html")).ok()
}

/// Salva o frame da FONTE (FBO próprio) da aba `index` em `path` — evidência de que CADA aba renderiza
/// seu próprio conteúdo (FBOs independentes). Pinta e torna o contexto corrente antes de ler.
fn dump_tab_source(manager: &Rc<RefCell<Option<TabManager>>>, index: usize, path: &str) {
    if let Some(m) = manager.borrow().as_ref() {
        if let Some(tab) = m.tabs.get(index) {
            tab.webview.paint();
            if tab.context.make_current().is_ok() {
                dump_source(tab, path, true);
            }
        }
    }
}

/// Loga contagem de abas + índice ativo (resumo do driver de evidência).
fn log_tab_summary(manager: &Rc<RefCell<Option<TabManager>>>) {
    if let Some(m) = manager.borrow().as_ref() {
        eprintln!(
            "[tabtest] resumo: {} aba(s), ativa={}",
            m.tabs.len(),
            m.active
        );
    }
}

/// Driver de evidência das abas (`BASEDBROWSER_TAB_TEST=1`): como não há clique na UI num smoke test
/// headless (captura de janela bloqueada no Wayland), dispara os MESMOS callbacks da barra de abas via
/// `invoke_*` numa sequência temporizada (1 passo/s) e salva o source FBO da 2ª aba. Combinado com
/// `BASEDBROWSER_DUMP_FRAME` (textura da aba ATIVA) + `BASEDBROWSER_EXIT_AFTER_MS`, prova abrir/trocar/
/// fechar com conteúdo distinto por aba. Devolve o `Timer` (mantê-lo vivo). No-op sem a env.
fn install_tab_test(app: &MainWindow, manager: &Rc<RefCell<Option<TabManager>>>) -> Option<Timer> {
    std::env::var_os("BASEDBROWSER_TAB_TEST")?;
    let timer = Timer::default();
    let weak = app.as_weak();
    let mgr = manager.clone();
    let step = Cell::new(0u32);
    timer.start(
        TimerMode::Repeated,
        Duration::from_millis(1000),
        move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            let n = step.replace(step.get() + 1);
            match n {
                2 => {
                    app.invoke_new_tab();
                    eprintln!("[tabtest] passo: abrir 2ª aba");
                }
                3 => {
                    if let Some(url) = page2_url() {
                        app.invoke_load_url(url.to_string().into());
                    }
                    eprintln!("[tabtest] passo: carregar page2 na aba ativa (2ª)");
                }
                5 => dump_tab_source(&mgr, 1, "/tmp/m4-t4-tab1-source.png"),
                6 => {
                    app.invoke_select_tab(0);
                    eprintln!("[tabtest] passo: trocar p/ aba 0");
                }
                8 => {
                    app.invoke_close_tab(1);
                    eprintln!("[tabtest] passo: fechar aba 1");
                }
                9 => log_tab_summary(&mgr),
                _ => {}
            }
        },
    );
    Some(timer)
}

/// Instala todos os drivers de evidência (no-op sem as envs respectivas). Retorna os timers — o
/// chamador deve mantê-los vivos.
fn install_evidence_drivers(
    app: &MainWindow,
    manager: &Rc<RefCell<Option<TabManager>>>,
) -> Vec<Timer> {
    [
        install_tab_test(app, manager),
        install_bookmark_test(app),
        install_history_test(app),
        install_persist_test(manager),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Driver de evidência da persistência (`BASEDBROWSER_PERSIST_TEST=1`, M6/ADR-0009): sem captura de
/// janela (Wayland, L-008), prova em TEXTO que cookies + `localStorage` sobrevivem ao restart. A
/// página de teste (`scripts/m6/pages/persist.html`) reflete o cookie + `localStorage` lidos no
/// `document.title`; aqui lemos `webview.page_title()` (poll até a página carregar) e os cookies do
/// jar via `cookies_for_url`. Rodado 2× no MESMO perfil por `scripts/m6/verify-persist.sh` (RUN1 seta,
/// RUN2 lê de volta). No-op sem a env. Mantenha o `Timer`.
fn install_persist_test(manager: &Rc<RefCell<Option<TabManager>>>) -> Option<Timer> {
    std::env::var_os("BASEDBROWSER_PERSIST_TEST")?;
    let timer = Timer::default();
    let mgr = manager.clone();
    let done = Cell::new(false);
    timer.start(
        TimerMode::Repeated,
        Duration::from_millis(1000),
        move || {
            if done.get() {
                return;
            }
            let guard = mgr.borrow();
            let Some(m) = guard.as_ref() else {
                return;
            };
            let Some(tab) = m.active_tab() else {
                return;
            };
            let title = tab.webview.page_title().unwrap_or_default();
            // Espera a página rodar o JS (que escreve o título "BBPERSIST cookie=… local=…").
            if !title.starts_with("BBPERSIST cookie=") {
                return;
            }
            done.set(true);
            eprintln!("[persisttest] title={title}");
            let url_str = tab.webview.url().map(|u| u.to_string()).unwrap_or_default();
            if let Ok(url) = Url::parse(&url_str) {
                let cookies = m
                    .servo
                    .site_data_manager()
                    .cookies_for_url(url, CookieSource::NonHTTP);
                for c in &cookies {
                    eprintln!("[persisttest] cookie {}={}", c.name(), c.value());
                }
                eprintln!("[persisttest] {} cookie(s) lido(s) do jar", cookies.len());
            }
        },
    );
    Some(timer)
}

/// Driver de evidência do histórico (`BASEDBROWSER_HISTORY_TEST=1`): invoca os callbacks do painel +
/// autocomplete e loga as contagens dos models resultantes (provando popular/filtrar/sugerir/revisitar
/// num smoke test não-interativo). Requer histórico pré-existente (de execuções anteriores). Mantenha
/// o `Timer`.
fn install_history_test(app: &MainWindow) -> Option<Timer> {
    std::env::var_os("BASEDBROWSER_HISTORY_TEST")?;
    let timer = Timer::default();
    let weak = app.as_weak();
    let step = Cell::new(0u32);
    timer.start(
        TimerMode::Repeated,
        Duration::from_millis(1000),
        move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            match step.replace(step.get() + 1) {
                2 => {
                    app.invoke_toggle_history();
                    eprintln!(
                        "[histtest] painel aberto: {} entrada(s)",
                        app.get_history().row_count()
                    );
                }
                3 => {
                    app.invoke_search_history("basedbrowser".into());
                    eprintln!(
                        "[histtest] busca 'basedbrowser': {} resultado(s)",
                        app.get_history().row_count()
                    );
                }
                4 => {
                    app.invoke_url_edited("file".into());
                    eprintln!(
                        "[histtest] autocomplete 'file': {} sugestão(ões)",
                        app.get_suggestions().row_count()
                    );
                }
                5 => {
                    app.invoke_open_history(0);
                    eprintln!("[histtest] revisitou history[0] (carrega na aba ativa)");
                }
                _ => {}
            }
        },
    );
    Some(timer)
}

/// Driver de evidência dos favoritos (`BASEDBROWSER_BOOKMARK_TEST=1`): invoca `add-bookmark` da página
/// atual após o load (~3 s), p/ provar persistência num smoke test não-interativo. Mantenha o `Timer`.
fn install_bookmark_test(app: &MainWindow) -> Option<Timer> {
    std::env::var_os("BASEDBROWSER_BOOKMARK_TEST")?;
    let timer = Timer::default();
    let weak = app.as_weak();
    timer.start(
        TimerMode::SingleShot,
        Duration::from_millis(3000),
        move || {
            if let Some(app) = weak.upgrade() {
                app.invoke_add_bookmark();
                eprintln!("[bmtest] add-bookmark invocado (favorita a página atual)");
            }
        },
    );
    Some(timer)
}

/// Instala um timer single-shot que encerra o loop do Slint após `BASEDBROWSER_EXIT_AFTER_MS` ms, se
/// a env estiver setada (exit LIMPO → roda o save-on-exit). `None`/no-op quando ausente ou inválida.
/// O `Timer` retornado precisa ser mantido vivo pelo chamador. Fora do caminho normal de uso.
fn install_exit_timer() -> Option<Timer> {
    let ms: u64 = std::env::var("BASEDBROWSER_EXIT_AFTER_MS")
        .ok()?
        .parse()
        .ok()?;
    let timer = Timer::default();
    timer.start(TimerMode::SingleShot, Duration::from_millis(ms), || {
        eprintln!("[m4] BASEDBROWSER_EXIT_AFTER_MS atingido; encerrando o loop (exit limpo)");
        let _ = slint::quit_event_loop();
    });
    Some(timer)
}

/// Carrega o estado persistido (favoritos/histórico) e loga a contagem. A sessão de abas é carregada
/// e restaurada no `init_manager` (T7). Devolve o estado vivo compartilhado com o `Embedder`.
fn load_persisted_state() -> Rc<RefCell<persist::AppData>> {
    let data = Rc::new(RefCell::new(persist::AppData::load()));
    let d = data.borrow();
    eprintln!(
        "[m4] persistência: {} favorito(s), {} entrada(s) no histórico",
        d.bookmarks.len(),
        d.history.len()
    );
    drop(d);
    data
}

/// Salva a sessão de abas ao encerrar: as URLs de todas as abas + o índice da ativa. No-op se o
/// manager nunca subiu ou nenhuma aba tem URL. A restauração no start chega na T7.
fn save_session_on_exit(manager: &Rc<RefCell<Option<TabManager>>>) {
    if let Some(m) = manager.borrow().as_ref() {
        let tabs: Vec<String> = m
            .tabs
            .iter()
            .filter_map(|tab| tab.webview.url().map(|url| url.to_string()))
            .collect();
        if !tabs.is_empty() {
            let active = m.active.min(tabs.len() - 1);
            persist::save_session(&persist::Session { tabs, active });
            eprintln!("[m4] sessão salva ({} aba(s))", m.tabs.len());
        }
    }
}

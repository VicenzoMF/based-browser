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
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use devtools_client::NetRecord;

mod devtools_client;
mod gpu_bridge;
mod input;
mod persist;

use euclid::Scale;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{
    AllowOrDenyRequest, ConsoleLogLevel, CookieSource, CreateNewWebViewRequest, DeviceIntRect,
    EventLoopWaker, JSValue, LoadStatus, OffscreenRenderingContext, Opts, Preferences,
    RenderingContext, Servo, ServoBuilder, ServoDelegate, ServoError, StorageType, WebView,
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
slint::slint!(export { MainWindow, TabInfo, BookmarkInfo, HistoryItem, DevConsoleLine, DevNetRow } from "../ui/app.slint";);

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
    /// M7 (ADR-0010): estado de devtools compartilhado (console in-process + rede do cliente RDP +
    /// porta do servidor). Escrito aqui durante o spin (console/porta) e drenado/lido pelo LOOP e pela
    /// UI. Vazio/no-op quando o devtools está desligado (sem a env `BASEDBROWSER_DEVTOOLS`).
    devtools: Rc<DevtoolsState>,
}

/// M7 (ADR-0010): estado de inspeção compartilhado (Rc) entre o `Embedder` (escreve console/porta
/// durante o `spin_event_loop`), o LOOP (drena o canal de rede → models) e a UI (lê os buffers).
/// Criado SEMPRE, mas só populado quando o servidor de devtools está ligado (`BASEDBROWSER_DEVTOOLS`).
/// Tudo interior-mutável p/ o delegate escrever sem `borrow_mut` do `TabManager` (invariante ADR-0007).
struct DevtoolsState {
    /// Porta do servidor de devtools do Servo, capturada via `notify_devtools_server_started`. `None`
    /// até o servidor subir. Usada pelo cliente RDP (T4) e pela UI (T5). Ver [`devtools_port`].
    port: Cell<Option<u16>>,
    /// Console IN-PROCESS (M7): linhas de `console.log/warn/error/...` capturadas por
    /// `WebViewDelegate::show_console_message` — chega ao embedder INCONDICIONALMENTE (não depende do
    /// servidor de devtools). Teto FIFO [`DEVTOOLS_CONSOLE_CAP`]. Lido pela UI (T5) / driver (T6).
    console: RefCell<Vec<ConsoleLine>>,
    /// Eventos de rede (req+resp) capturados pelo cliente RDP (T4), por upsert de `id`. O LOOP/timer
    /// drena [`net_rx`] p/ cá; a UI (T5)/driver leem daqui.
    net: RefCell<Vec<NetRecord>>,
    /// Console/rede mudaram → o LOOP re-sincroniza os models da UI (T5). Análogo ao `chrome_dirty`.
    dirty: Cell<bool>,
    /// Lado-emissor do canal cliente RDP → UI. O delegate o CLONA para a thread do cliente.
    net_tx: Sender<NetRecord>,
    /// Lado-receptor (drenado pelo timer de devtools). `Option` p/ ser `take`-able caso necessário.
    net_rx: RefCell<Option<Receiver<NetRecord>>>,
    /// Garante 1 cliente RDP por sessão (o servidor pode emitir `notify_devtools_server_started` 1×).
    client_spawned: Cell<bool>,
}

impl DevtoolsState {
    fn new() -> Self {
        let (net_tx, net_rx) = mpsc::channel();
        Self {
            port: Cell::new(None),
            console: RefCell::new(Vec::new()),
            net: RefCell::new(Vec::new()),
            dirty: Cell::new(false),
            net_tx,
            net_rx: RefCell::new(Some(net_rx)),
            client_spawned: Cell::new(false),
        }
    }
}

/// Uma linha do console in-app (M7): nível (`log`/`warn`/`error`/…) + texto já formatado pelo Servo.
struct ConsoleLine {
    level: &'static str,
    text: String,
}

/// Teto FIFO do buffer de console (M7) — evita crescer sem limite numa página que loga muito.
const DEVTOOLS_CONSOLE_CAP: usize = 500;

/// M7 (ADR-0010): porta do servidor de devtools quando OPT-IN. `None` = desligado (sem a env
/// `BASEDBROWSER_DEVTOOLS`) → sem socket, caminho normal e números do M5 intactos. Com a env setada:
/// `1`/`true`/`on`/`yes`/vazio ligam na porta padrão do Servo (7000); um número (≥1024) escolhe a porta.
///
/// Porta **FIXA** (não efêmera) por necessidade: o Servo 0.2.0 reporta ao embedder a porta PEDIDA, não a
/// real do listener — `servo-devtools-0.2.0/lib.rs:202-203` faz `Ok(address.port())` e descarta o
/// `local_addr()` (l.196). Com `:0` o embedder receberia `0` e o cliente RDP não saberia onde conectar.
/// Loopback sempre (`127.0.0.1`). Ver Decisão de segurança do ADR-0010.
fn devtools_port() -> Option<u16> {
    let raw = std::env::var("BASEDBROWSER_DEVTOOLS").ok()?;
    let port = match raw.trim() {
        "" | "1" | "true" | "on" | "yes" => 7000,
        other => other
            .parse::<u16>()
            .ok()
            .filter(|p| *p >= 1024)
            .unwrap_or(7000),
    };
    Some(port)
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

    /// M9 (ADR-0012): o favicon mudou. Lê a imagem decodificada (`WebView::favicon`), converte p/
    /// `slint::Image` e guarda no `TabState` (roteado por id); o loop a reflete na barra de abas.
    /// Interior-mutável + `chrome_dirty` (invariante anti-reentrância do ADR-0007).
    fn notify_favicon_changed(&self, webview: WebView) {
        let icon = webview.favicon().and_then(|img| favicon_to_slint(&img));
        if let Some(state) = self.state_for(webview.id()) {
            *state.favicon.borrow_mut() = icon;
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

    /// M7 (ADR-0010): `console.log/warn/error/...` de uma página → console in-app. Esta callback chega
    /// ao embedder SEMPRE (não depende do servidor de devtools — `servo-script/dom/console.rs` emite
    /// `ShowConsoleApiMessage` incondicionalmente). Acumulamos num buffer interior-mutável (teto FIFO)
    /// e marcamos `devtools.dirty` p/ o LOOP refletir na UI — NUNCA escrevemos no Slint aqui (ADR-0007).
    /// Buffer GLOBAL (não por-aba): um dev-tool simples mostra o console de toda a sessão.
    fn show_console_message(&self, _webview: WebView, level: ConsoleLogLevel, message: String) {
        {
            let mut buf = self.devtools.console.borrow_mut();
            buf.push(ConsoleLine {
                level: console_level_str(&level),
                text: message,
            });
            let len = buf.len();
            if len > DEVTOOLS_CONSOLE_CAP {
                buf.drain(0..len - DEVTOOLS_CONSOLE_CAP);
            }
        }
        self.devtools.dirty.set(true);
    }
}

/// Nome curto e estável do nível de console (M7) p/ exibir/logar.
fn console_level_str(level: &ConsoleLogLevel) -> &'static str {
    match level {
        ConsoleLogLevel::Log => "log",
        ConsoleLogLevel::Debug => "debug",
        ConsoleLogLevel::Info => "info",
        ConsoleLogLevel::Warn => "warn",
        ConsoleLogLevel::Error => "error",
        ConsoleLogLevel::Trace => "trace",
    }
}

/// M7 (ADR-0010): formata o resultado de um `evaluate_javascript` (`JSValue`) p/ uma linha de texto do
/// console in-app. Referências de DOM (Element/Frame/Window/ShadowRoot) viram o id opaco que o Servo dá
/// (inspeção via `outerHTML` etc. no próprio eval). Recursivo p/ Array/Object.
fn format_jsvalue(value: &JSValue) -> String {
    match value {
        JSValue::Undefined => "undefined".to_string(),
        JSValue::Null => "null".to_string(),
        JSValue::Boolean(b) => b.to_string(),
        JSValue::Number(n) => n.to_string(),
        JSValue::String(s) => s.clone(),
        JSValue::Element(s) | JSValue::ShadowRoot(s) | JSValue::Frame(s) | JSValue::Window(s) => {
            s.clone()
        }
        JSValue::Array(items) => {
            let parts: Vec<String> = items.iter().map(format_jsvalue).collect();
            format!("[{}]", parts.join(", "))
        }
        JSValue::Object(map) => {
            let parts: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{k}: {}", format_jsvalue(v)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

/// M7 (ADR-0010): roda `script` na ABA ATIVA via `WebView::evaluate_javascript` (in-process) e empurra
/// a entrada + o resultado no buffer de console (níveis `eval`/`result`/`erro`). O callback é
/// ASSÍNCRONO (dispara num spin posterior, quando o JS termina): captura um clone do `DevtoolsState` e
/// só mexe nele + `dirty` — NUNCA escreve no Slint (invariante anti-reentrância do ADR-0007).
fn devtools_eval(
    manager: &Rc<RefCell<Option<TabManager>>>,
    devtools: &Rc<DevtoolsState>,
    script: String,
) {
    devtools.console.borrow_mut().push(ConsoleLine {
        level: "eval",
        text: script.clone(),
    });
    devtools.dirty.set(true);
    let dt = devtools.clone();
    with_active_webview(manager, move |wv| {
        wv.evaluate_javascript(script, move |res| {
            let (level, text) = match res {
                Ok(value) => ("result", format_jsvalue(&value)),
                Err(err) => ("erro", format!("{err:?}")),
            };
            {
                let mut buf = dt.console.borrow_mut();
                buf.push(ConsoleLine { level, text });
                let len = buf.len();
                if len > DEVTOOLS_CONSOLE_CAP {
                    buf.drain(0..len - DEVTOOLS_CONSOLE_CAP);
                }
            }
            dt.dirty.set(true);
        });
    });
}

/// M9 (ADR-0012): estado do find-in-page. `result` = (índice 1-based, total); (0,0) = nada/limpo.
/// O callback (assíncrono) do `evaluate_javascript` NÃO escreve no Slint (ADR-0007): guarda aqui e
/// marca `dirty`; o Timer de `setup_find` reflete nas props. `Cell<(i32,i32)>` (tupla Copy).
#[derive(Default)]
struct FindState {
    result: Cell<(i32, i32)>,
    dirty: Cell<bool>,
}

/// M9: script de find-in-page injetado na ABA ATIVA (`evaluate_javascript`). O Servo 0.2.0 não expõe
/// busca nativa → fazemos em JS (TreeWalker): destaca todas as ocorrências (`<mark class=bb-find>`),
/// navega entre elas (índice em `window.__bbi`, query em `window.__bbq`) e devolve "idx,count". Usa
/// `__Q__`/`__F__` (substituídos por `find_script`) p/ não brigar com as chaves no `format!`.
const FIND_JS_TEMPLATE: &str = r"(function(q, forward){
var P='bb-find';
var old=document.querySelectorAll('mark.'+P);
for(var k=0;k<old.length;k++){var m=old[k];var t=document.createTextNode(m.textContent);m.parentNode.replaceChild(t,m);}
if(q===''){window.__bbq='';window.__bbi=0;return '0,0';}
var lc=q.toLowerCase();
var prev=window.__bbq; window.__bbq=q;
var nodes=[];
var w=document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null);
var nd; while(nd=w.nextNode()){
  var v=nd.nodeValue; if(!v||!v.trim())continue;
  var p=nd.parentNode; if(!p)continue;
  var tag=p.nodeName; if(tag==='SCRIPT'||tag==='STYLE'||tag==='NOSCRIPT'||tag==='TEXTAREA')continue;
  if(v.toLowerCase().indexOf(lc)===-1)continue;
  nodes.push(nd);
}
var marks=[];
for(var i2=0;i2<nodes.length;i2++){
  var node=nodes[i2]; var text=node.nodeValue; var low=text.toLowerCase();
  var frag=document.createDocumentFragment(); var last=0; var pos;
  while((pos=low.indexOf(lc,last))!==-1){
    if(pos>last)frag.appendChild(document.createTextNode(text.slice(last,pos)));
    var mk=document.createElement('mark'); mk.className=P;
    mk.style.backgroundColor='#6c5ce7'; mk.style.color='#fff'; mk.style.borderRadius='2px';
    mk.textContent=text.slice(pos,pos+q.length);
    frag.appendChild(mk); marks.push(mk);
    last=pos+q.length;
  }
  if(last<text.length)frag.appendChild(document.createTextNode(text.slice(last)));
  node.parentNode.replaceChild(frag,node);
}
var count=marks.length;
if(count===0){window.__bbi=0;return '0,0';}
var idx;
if(prev!==q){idx=0;}else{idx=(window.__bbi||0)+(forward?1:-1);}
if(idx<0)idx=count-1; if(idx>=count)idx=0;
window.__bbi=idx;
for(var j=0;j<marks.length;j++){marks[j].style.backgroundColor=(j===idx)?'#ff8a3d':'#6c5ce7';}
try{marks[idx].scrollIntoView({block:'center'});}catch(e){marks[idx].scrollIntoView();}
return (idx+1)+','+count;
})(__Q__, __F__)";

/// M9: remove os destaques do find (ao fechar a barra / trocar de busca p/ vazio).
const FIND_CLEAR_JS: &str = r"(function(){var P='bb-find';var old=document.querySelectorAll('mark.'+P);for(var k=0;k<old.length;k++){var m=old[k];var t=document.createTextNode(m.textContent);m.parentNode.replaceChild(t,m);}window.__bbq='';window.__bbi=0;})()";

/// M9: instancia o script de find com a `query` (escapada via JSON → string literal JS segura) e o
/// sentido (`forward`).
fn find_script(query: &str, forward: bool) -> String {
    let q = serde_json::to_string(query).unwrap_or_else(|_| "\"\"".to_string());
    FIND_JS_TEMPLATE
        .replace("__Q__", &q)
        .replace("__F__", if forward { "true" } else { "false" })
}

/// M9: parseia o "idx,count" devolvido pelo script de find. `None` se o JS errou / formato inesperado.
fn parse_find_result(value: &JSValue) -> Option<(i32, i32)> {
    let s = match value {
        JSValue::String(s) => s.clone(),
        other => format_jsvalue(other),
    };
    let (idx, count) = s.split_once(',')?;
    Some((idx.trim().parse().ok()?, count.trim().parse().ok()?))
}

/// M9 (ADR-0012): liga o find-in-page (Ctrl+F / menu ⋯). `find-in-page(query, forward)` injeta o
/// script na aba ativa; o callback (assíncrono, NÃO toca o Slint — ADR-0007) guarda "idx,count" no
/// `FindState` + `dirty`, e o Timer reflete nas props `find-index`/`find-count`. `find-close` limpa os
/// destaques e zera o contador. Devolve o Timer (mantê-lo vivo).
fn setup_find(app: &MainWindow, manager: &Rc<RefCell<Option<TabManager>>>) -> Timer {
    let find = Rc::new(FindState::default());

    let mgr = manager.clone();
    let st = find.clone();
    app.on_find_in_page(move |query, forward| {
        let script = find_script(query.as_str(), forward);
        let st = st.clone();
        with_active_webview(&mgr, move |wv| {
            wv.evaluate_javascript(script, move |res| {
                if let Ok(value) = res {
                    if let Some(pair) = parse_find_result(&value) {
                        st.result.set(pair);
                        st.dirty.set(true);
                    }
                }
            });
        });
    });

    let mgr = manager.clone();
    let st = find.clone();
    app.on_find_close(move || {
        with_active_webview(&mgr, |wv| {
            wv.evaluate_javascript(FIND_CLEAR_JS.to_string(), |_res| {});
        });
        st.result.set((0, 0));
        st.dirty.set(true);
    });

    let timer = Timer::default();
    let weak = app.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_millis(80), move || {
        if find.dirty.replace(false) {
            if let Some(app) = weak.upgrade() {
                let (idx, count) = find.result.get();
                app.set_find_index(idx);
                app.set_find_count(count);
            }
        }
    });
    timer
}

/// M9: liga os painéis dirigidos por injeção de JS (devtools M7 + find M9) numa só chamada (mantém
/// `main` enxuto). Os dois Timers devem ser mantidos vivos pelo chamador.
fn setup_panels(
    app: &MainWindow,
    manager: &Rc<RefCell<Option<TabManager>>>,
    sink: &Rc<Embedder>,
) -> (Timer, Timer) {
    (setup_devtools(app, manager, sink), setup_find(app, manager))
}

/// `ServoDelegate` (M7, ADR-0010): hooks de NÍVEL-SERVO (não por-`WebView`). Usado SÓ para o servidor
/// de devtools — setado via `servo.set_delegate` apenas quando o devtools está ligado. Roda durante o
/// `spin_event_loop`; só mexe em estado interior-mutável (`devtools`), nunca escreve no Slint
/// (invariante anti-reentrância do ADR-0007), e spawnar a thread do cliente RDP (T4) é seguro aqui.
impl ServoDelegate for Embedder {
    /// O servidor de devtools subiu. Capturamos a `port` (efêmera — pedimos `:0`, o SO escolhe) para o
    /// cliente RDP in-app (T4) conectar e para a UI (T5) exibir. Logamos um sinal de TEXTO determinístico
    /// para a verificação sem captura de janela (L-008).
    fn notify_devtools_server_started(&self, port: u16, _token: String) {
        self.devtools.port.set(Some(port));
        eprintln!("[m7] devtools: server started on 127.0.0.1:{port}");
        // Sobe o cliente RDP de rede (1×) AGORA — cedo, p/ assinar os eventos de rede antes da página
        // disparar requisições (não há snapshot p/ network-event). Spawnar thread aqui é seguro
        // (ADR-0007): não toca o Slint nem faz borrow do TabManager.
        if !self.devtools.client_spawned.replace(true) {
            devtools_client::spawn(port, self.devtools.net_tx.clone());
        }
    }

    /// Pedido de conexão de um cliente de devtools. O servidor está em loopback (`127.0.0.1`) e é o NOSSO
    /// cliente RDP in-app → autorizamos. (Risco residual honesto, ADR-0010: enquanto o devtools está
    /// ligado, outro processo LOCAL também poderia conectar — aceito por ser opt-in/dev e loopback.)
    fn request_devtools_connection(&self, request: AllowOrDenyRequest) {
        eprintln!("[m7] devtools: conexão de cliente autorizada (loopback)");
        request.allow();
    }

    /// Erros de nível-Servo (inclui `DevtoolsFailedToStart`). Apenas loga — nunca paniqueia.
    fn notify_error(&self, error: ServoError) {
        eprintln!("[m7] devtools: erro do Servo: {error:?}");
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
    /// M9: zoom de página (1.0 = 100%); espelha `WebView::page_zoom`. Inicializado em `open_tab`
    /// (o `Default` daria 0.0). Mostrado no menu ⋯ e ajustado por Ctrl +/−/0.
    page_zoom: Cell<f32>,
    /// M9: favicon da página, já convertido p/ `slint::Image`; `None` até o Servo entregar.
    favicon: RefCell<Option<slint::Image>>,
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
        state.page_zoom.set(1.0); // M9: 100% (o Default de Cell<f32> seria 0.0).
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
    // M7 (ADR-0010): OPT-IN — habilita o servidor de devtools do Servo só com `BASEDBROWSER_DEVTOOLS`.
    // `devtools_server_enabled=true` faz o Servo subir o servidor (protocolo RDP do Firefox) em
    // `build()`; bind em `127.0.0.1:<port>` (loopback, porta fixa — ver `devtools_port`). Mexida
    // MÍNIMA/aditiva no builder (1 ponto, igual ao `opts` do M6; L-001), não reorganiza o init lazy do
    // GL (L-004).
    if let Some(port) = devtools_port() {
        builder = builder.preferences(Preferences {
            devtools_server_enabled: true,
            devtools_server_listen_address: format!("127.0.0.1:{port}"),
            ..Preferences::default()
        });
        eprintln!(
            "[m7] devtools: servidor habilitado (BASEDBROWSER_DEVTOOLS; loopback 127.0.0.1:{port})"
        );
    }
    let servo = builder.build();
    servo.setup_logging();
    // M7: registra o `ServoDelegate` (hooks de nível-servo p/ devtools) só quando ligado — caminho
    // normal fica byte-idêntico. O delegate é o próprio `Embedder` (Rc<Embedder> → Rc<dyn ServoDelegate>).
    if devtools_port().is_some() {
        servo.set_delegate(sink.clone());
    }

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
/// M9 (ADR-0012): converte o favicon do Servo (`servo::Image`, re-export de `embedder_traits`) p/
/// `slint::Image`. O Servo entrega RGBA8/BGRA8; convertemos p/ RGBA8 (swap R/B no caso BGRA). Sem dep
/// nova (a troca é à mão). O un-premultiply é dispensado (favicons ~opacos; diferença só em bordas
/// semitransparentes de um ícone de 15px — caveat aceito). `None` em formato não-RGBA/dimensão zero.
fn favicon_to_slint(img: &servo::Image) -> Option<slint::Image> {
    if img.width == 0 || img.height == 0 {
        return None;
    }
    let swap_rb = match img.format {
        servo::PixelFormat::RGBA8 => false,
        servo::PixelFormat::BGRA8 => true,
        _ => return None,
    };
    let src = img.data();
    let w = usize::try_from(img.width).ok()?;
    let h = usize::try_from(img.height).ok()?;
    let expected = w.checked_mul(h)?.checked_mul(4)?;
    if src.len() < expected {
        return None;
    }
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(img.width, img.height);
    for (d, s) in buffer
        .make_mut_bytes()
        .chunks_exact_mut(4)
        .zip(src.chunks_exact(4))
    {
        if swap_rb {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = s[3];
        } else {
            d.copy_from_slice(s);
        }
    }
    Some(Image::from_rgba8(buffer))
}

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

/// M9: limites e passo do zoom de página (aditivo, 10% por clique → percentuais limpos).
const ZOOM_STEP: f32 = 0.1;
const ZOOM_MIN: f32 = 0.3;
const ZOOM_MAX: f32 = 5.0;

/// M9: fator de zoom (f32) → percentual inteiro p/ exibição. O zoom é fixado em [0.3, 5.0]
/// (`apply_zoom`), então o percentual cabe em [30, 500] — sem truncação real no cast.
fn zoom_percent(zoom: f32) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "zoom ∈ [0.3, 5.0] → percentual ∈ [30, 500]; sem truncação"
    )]
    let pct = (zoom * 100.0).round() as i32;
    pct
}

/// M9: aplica um zoom à ABA ATIVA via `WebView::set_page_zoom` (clamp), espelha no `TabState` e
/// atualiza o chrome imediatamente. `target(atual) -> novo` define a operação (in/out/reset).
fn apply_zoom(
    manager: &Rc<RefCell<Option<TabManager>>>,
    app: &slint::Weak<MainWindow>,
    target: impl Fn(f32) -> f32,
) {
    if let Some(m) = manager.borrow().as_ref() {
        if let Some(tab) = m.active_tab() {
            let z = target(tab.webview.page_zoom()).clamp(ZOOM_MIN, ZOOM_MAX);
            tab.webview.set_page_zoom(z);
            tab.state.page_zoom.set(z);
            if let Some(app) = app.upgrade() {
                app.set_zoom_percent(zoom_percent(z));
            }
        }
        wake(m);
    }
}

/// M9: fecha a ABA ATIVA (Ctrl+W). Reusa o callback `close-tab(idx)` do Slint (que checa limites e
/// recusa fechar a última). `try_from` evita o cast truncante usize→i32 (lint do projeto).
fn close_active_tab(app: &MainWindow, manager: &Rc<RefCell<Option<TabManager>>>) {
    let idx = manager.borrow().as_ref().map(|m| m.active);
    if let Some(idx) = idx.and_then(|i| i32::try_from(i).ok()) {
        app.invoke_close_tab(idx);
    }
}

/// M9: vai p/ a PRÓXIMA aba, circular (Ctrl+Tab). Reusa `select-tab(idx)`.
fn cycle_tab(app: &MainWindow, manager: &Rc<RefCell<Option<TabManager>>>) {
    let next = manager
        .borrow()
        .as_ref()
        .and_then(|m| (!m.tabs.is_empty()).then(|| (m.active + 1) % m.tabs.len()));
    if let Some(next) = next.and_then(|n| i32::try_from(n).ok()) {
        app.invoke_select_tab(next);
    }
}

/// M9: intercepta atalhos de CHROME no caminho de teclado, ANTES de repassar ao Servo (a tecla é
/// "roubada" da página). Devolve `true` se a tecla é um atalho (não repassar — swallow no press E no
/// release). A AÇÃO roda só no press inicial (`pressed && !repeat`). Reusa os callbacks existentes
/// (`invoke_*`). Ctrl+L foca a omnibox (callback `focus-url-bar`, tratado no Slint); Ctrl+F abre a
/// find bar; Escape a fecha. Os atalhos usam `text`+modificadores (o code físico não é exposto, M2).
fn handle_chrome_key(
    app: &MainWindow,
    manager: &Rc<RefCell<Option<TabManager>>>,
    text: &str,
    pressed: bool,
    ctrl: bool,
    repeat: bool,
) -> bool {
    // Escape fecha a find bar (se aberta); senão segue p/ a página.
    if input::is_escape(text) {
        if app.get_find_open() {
            if pressed {
                app.set_find_open(false);
                app.invoke_find_close();
            }
            return true;
        }
        return false;
    }
    if !ctrl {
        return false;
    }
    let act = pressed && !repeat; // ação só no press inicial; release é só swallow
                                  // Ctrl+Tab: próxima aba.
    if input::is_tab(text) {
        if act {
            cycle_tab(app, manager);
        }
        return true;
    }
    match text {
        "t" | "T" => {
            if act {
                app.invoke_new_tab();
            }
            true
        }
        "w" | "W" => {
            if act {
                close_active_tab(app, manager);
            }
            true
        }
        "l" | "L" => {
            if act {
                app.invoke_focus_url_bar();
            }
            true
        }
        "r" | "R" => {
            if act {
                app.invoke_reload();
            }
            true
        }
        "f" | "F" => {
            if act {
                app.set_find_open(true);
            }
            true
        }
        "+" | "=" => {
            if act {
                app.invoke_zoom_in();
            }
            true
        }
        "-" | "_" => {
            if act {
                app.invoke_zoom_out();
            }
            true
        }
        "0" => {
            if act {
                app.invoke_zoom_reset();
            }
            true
        }
        _ => false,
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
    let weak = app.as_weak();
    app.on_forward_key(move |text, pressed, ctrl, shift, alt, meta, repeat| {
        // M9: atalhos de chrome (Ctrl+T/W/L/R/Tab/F, Ctrl +/−/0, Esc) são interceptados ANTES do
        // repasse — "roubam" a tecla da página. O resto segue normal p/ o Servo (digitação intacta).
        if let Some(app) = weak.upgrade() {
            if handle_chrome_key(&app, &mgr, text.as_str(), pressed, ctrl, repeat) {
                return;
            }
        }
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

    // M9: zoom de página (Ctrl +/−/0 e o menu ⋯). Aplica na aba ativa e reflete o % no chrome.
    let mgr = manager.clone();
    let weak = app.as_weak();
    app.on_zoom_in(move || apply_zoom(&mgr, &weak, |z| z + ZOOM_STEP));

    let mgr = manager.clone();
    let weak = app.as_weak();
    app.on_zoom_out(move || apply_zoom(&mgr, &weak, |z| z - ZOOM_STEP));

    let mgr = manager.clone();
    let weak = app.as_weak();
    app.on_zoom_reset(move || apply_zoom(&mgr, &weak, |_| 1.0));
}

/// Reflete o estado da ABA ATIVA nas propriedades do chrome (Slint). Chamado pelo loop quando o
/// `Embedder` marcou `chrome_dirty` — centraliza as escritas de UI no loop (fora do `spin_event_loop`).
fn sync_chrome(app: &MainWindow, manager: &TabManager) {
    let Some(tab) = manager.active_tab() else {
        return;
    };
    let state = &tab.state;
    {
        let url = state.url.borrow();
        app.set_page_url(url.as_str().into());
        // M9: cadeado fechado/ok p/ https (derivado aqui; o Slint não faz parsing de URL).
        app.set_page_secure(url.starts_with("https://"));
    }
    app.set_loading(state.loading.get());
    app.set_can_go_back(state.can_go_back.get());
    app.set_can_go_forward(state.can_go_forward.get());
    app.set_zoom_percent(zoom_percent(state.page_zoom.get())); // M9: zoom da aba ativa (menu ⋯).
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
            // M9: favicon da aba (vazio → o Slint mostra o dot placeholder).
            icon: tab.state.favicon.borrow().clone().unwrap_or_default(),
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

    // M6 (ADR-0009): limpar DADOS DE NAVEGAÇÃO — cookies + Web Storage do Servo + nosso histórico.
    // PRESERVA favoritos e a sessão de abas. Reflete na UI esvaziando o painel de histórico.
    let (mgr, dat, mdl) = (manager.clone(), data.clone(), history_model.clone());
    app.on_clear_browsing_data(move || {
        clear_browsing_data(&mgr, &dat);
        mdl.set_vec(filtered_history(&dat.borrow(), "", 200));
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

/// M6 (ADR-0009): limpa os DADOS DE NAVEGAÇÃO — cookies + `localStorage`/`sessionStorage` do Servo
/// (via [`SiteDataManager`]) e o nosso histórico. **PRESERVA** favoritos e a sessão de abas (curadoria
/// do usuário; convenção de browser).
///
/// Roda num callback de UI (FORA do `spin_event_loop`) → faz só borrow IMUTÁVEL do `manager` p/ pegar
/// `&servo` (respeita o invariante anti-reentrância do ADR-0007). Os métodos do `SiteDataManager` são
/// síncronos/bloqueantes — aceitável p/ uma ação pontual do usuário.
///
/// `clear_cookies()` apaga TODOS os cookies (domain-independent). Para Web Storage, `clear_site_data`
/// é escopado por SITE (eTLD+1): enumeramos `site_data()` e passamos os nomes. Sites sem domínio
/// registrado (`localhost`/IPs, uso de dev) não são listados nem limpos por aqui — caveat registrado
/// no ADR-0009; em domínios reais limpa normalmente.
fn clear_browsing_data(
    manager: &Rc<RefCell<Option<TabManager>>>,
    data: &Rc<RefCell<persist::AppData>>,
) {
    if let Some(m) = manager.borrow().as_ref() {
        let sdm = m.servo.site_data_manager();
        sdm.clear_cookies();
        let storage = StorageType::Local | StorageType::Session;
        let sites: Vec<String> = sdm
            .site_data(storage)
            .into_iter()
            .map(|s| s.name())
            .collect();
        let refs: Vec<&str> = sites.iter().map(String::as_str).collect();
        sdm.clear_site_data(&refs, storage);
        eprintln!(
            "[m6] limpou cookies + Web Storage de {} site(s)",
            refs.len()
        );
    }
    data.borrow_mut().clear_history();
    eprintln!("[m6] limpou o histórico (favoritos e sessão preservados)");
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
        devtools: Rc::new(DevtoolsState::new()), // M7 (ADR-0010): inspeção (console/rede/porta)
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
    let _drivers = install_evidence_drivers(&app, &manager, &app_data, &sink);

    // M7 (devtools) + M9 (find): painéis dirigidos por injeção de JS. Manter os timers vivos.
    let _panels = setup_panels(&app, &manager, &sink);

    // Dirige o Servo e bombeia frames. O manager é montado LAZY aqui (e não no
    // `set_rendering_notifier`) para criar o contexto GL do Servo FORA do setup do renderer do
    // Slint, evitando a colisão de `make_current` que corrompia o GL (ver doc do módulo).
    let timer = Timer::default();
    let tick_manager = manager;
    let tick_sink = sink;
    let tick_weak = weak;
    let (tick_logged, tick_web_size) = (logged, web_size);
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
    data: &Rc<RefCell<persist::AppData>>,
    sink: &Rc<Embedder>,
) -> Vec<Timer> {
    [
        install_tab_test(app, manager),
        install_bookmark_test(app),
        install_history_test(app),
        install_persist_test(manager),
        install_clear_test(app, manager, data),
        install_devtools_test(app, manager, sink),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Driver de evidência do M7 (`BASEDBROWSER_DEVTOOLS_TEST=1`, ADR-0010): sem captura de janela
/// (Wayland, L-008), prova em TEXTO a inspeção in-app. Após o load da página de teste (que faz
/// `console.log`), dumpa o buffer de console (T2); roda eval via `evaluate_javascript` (T3); e dumpa os
/// registros de rede capturados pelo cliente RDP (T4). Etapas escalonadas em ticks de 1 s. No-op sem a
/// env. Mantenha o `Timer`. A página/subrecurso vêm de `scripts/m7/` (T6).
fn install_devtools_test(
    app: &MainWindow,
    manager: &Rc<RefCell<Option<TabManager>>>,
    sink: &Rc<Embedder>,
) -> Option<Timer> {
    std::env::var_os("BASEDBROWSER_DEVTOOLS_TEST")?;
    let timer = Timer::default();
    let weak = app.as_weak();
    let mgr = manager.clone();
    let devtools = sink.devtools.clone();
    let step = Cell::new(0u32);
    timer.start(
        TimerMode::Repeated,
        Duration::from_millis(1000),
        move || {
            let n = step.get();
            step.set(n + 1);
            match n {
                5 => dump_devtools_console(&devtools, "console"),
                6 => {
                    devtools_eval(&mgr, &devtools, "2 + 2".to_string());
                    eprintln!("[devtoolstest] eval enviado: 2 + 2");
                }
                7 => {
                    devtools_eval(&mgr, &devtools, "document.title".to_string());
                    eprintln!("[devtoolstest] eval enviado: document.title");
                }
                9 => dump_devtools_console(&devtools, "console pós-eval"),
                11 => dump_devtools_net(&devtools),
                // Abre o painel (força refresh dos models do Slint) p/ provar o binding de UI.
                12 => {
                    if let Some(app) = weak.upgrade() {
                        app.invoke_toggle_devtools();
                    }
                }
                13 => {
                    if let Some(app) = weak.upgrade() {
                        eprintln!(
                            "[devtoolstest] models do painel: dev-console={} linha(s), dev-net={} linha(s)",
                            app.get_dev_console().row_count(),
                            app.get_dev_net().row_count()
                        );
                    }
                }
                _ => {}
            }
        },
    );
    Some(timer)
}

/// Dumpa (TEXTO, L-008) o buffer de console in-app p/ o driver do M7 — prova console.log capturado (T2)
/// e os resultados de eval (T3).
fn dump_devtools_console(devtools: &Rc<DevtoolsState>, label: &str) {
    let buf = devtools.console.borrow();
    eprintln!("[devtoolstest] {label}: {} linha(s)", buf.len());
    for line in buf.iter() {
        eprintln!("[devtoolstest] console[{}] {}", line.level, line.text);
    }
}

/// Dumpa (TEXTO, L-008) os eventos de rede capturados pelo cliente RDP (T4) — prova o lado da RESPOSTA
/// (método/URL/status + 1º response header + tamanho do corpo), o núcleo do M7.
fn dump_devtools_net(devtools: &Rc<DevtoolsState>) {
    let net = devtools.net.borrow();
    eprintln!("[devtoolstest] rede: {} requisição(ões)", net.len());
    for r in net.iter() {
        let status = if r.status.is_empty() {
            "(pendente)"
        } else {
            r.status.as_str()
        };
        eprintln!(
            "[devtoolstest] net {} {} status={status} req_headers={} resp_headers={} body_len={}",
            r.method,
            r.url,
            r.req_headers.len(),
            r.resp_headers.len(),
            r.resp_body.len()
        );
        if let Some((k, v)) = r.resp_headers.first() {
            eprintln!("[devtoolstest] net   resp_header[0] {k}: {v}");
        }
    }
}

/// M7 (ADR-0010): liga o painel de devtools. Cria os models (console/rede), conecta os callbacks
/// (eval/limpar/abrir) e instala um Timer que DRENA o canal do cliente RDP de rede para `devtools.net`
/// e, quando algo muda (`dirty`), reconstrói os models do Slint — tudo na thread de UI, fora do loop de
/// render e do delegate (invariante do ADR-0007). Devolve o `Timer` (mantê-lo vivo).
fn setup_devtools(
    app: &MainWindow,
    manager: &Rc<RefCell<Option<TabManager>>>,
    sink: &Rc<Embedder>,
) -> Timer {
    let devtools = sink.devtools.clone();
    let console_model: Rc<VecModel<DevConsoleLine>> = Rc::new(VecModel::default());
    let net_model: Rc<VecModel<DevNetRow>> = Rc::new(VecModel::default());
    app.set_dev_console(console_model.clone().into());
    app.set_dev_net(net_model.clone().into());

    // Avaliar JS na aba ativa (REPL).
    let (mgr, dt) = (manager.clone(), devtools.clone());
    app.on_dev_eval(move |script| devtools_eval(&mgr, &dt, script.to_string()));
    // Limpar os buffers de console + rede.
    let dt = devtools.clone();
    app.on_clear_devtools(move || {
        dt.console.borrow_mut().clear();
        dt.net.borrow_mut().clear();
        dt.dirty.set(true);
    });
    // Abrir o painel força um refresh dos models no próximo tick.
    let dt = devtools.clone();
    app.on_toggle_devtools(move || dt.dirty.set(true));

    let weak = app.as_weak();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(150), move || {
        drain_devtools_net(&devtools);
        if devtools.dirty.replace(false) {
            if let Some(app) = weak.upgrade() {
                rebuild_dev_models(&app, &devtools, &console_model, &net_model);
            }
        }
    });
    timer
}

/// Reconstrói os models do painel de devtools (console + rede) e o texto de status a partir dos buffers
/// interior-mutáveis. Roda na thread de UI (chamado pelo Timer de [`setup_devtools`]).
fn rebuild_dev_models(
    app: &MainWindow,
    devtools: &Rc<DevtoolsState>,
    console_model: &VecModel<DevConsoleLine>,
    net_model: &VecModel<DevNetRow>,
) {
    let console: Vec<DevConsoleLine> = devtools
        .console
        .borrow()
        .iter()
        .map(|l| DevConsoleLine {
            level: l.level.into(),
            text: l.text.as_str().into(),
        })
        .collect();
    console_model.set_vec(console);

    let net = devtools.net.borrow();
    let rows: Vec<DevNetRow> = net
        .iter()
        .map(|r| DevNetRow {
            method: r.method.as_str().into(),
            url: r.url.as_str().into(),
            status: r.status.as_str().into(),
            mime: r.mime.as_str().into(),
            req_headers: join_headers(&r.req_headers).into(),
            resp_headers: join_headers(&r.resp_headers).into(),
            body: r.resp_body.as_str().into(),
        })
        .collect();
    net_model.set_vec(rows);

    let status = match devtools.port.get() {
        Some(port) => format!("servidor 127.0.0.1:{port} · {} requisição(ões)", net.len()),
        None => "servidor desligado — rode com BASEDBROWSER_DEVTOOLS p/ ver a rede".to_string(),
    };
    app.set_dev_status(status.into());
}

/// Junta uma lista de headers em texto multi-linha `nome: valor` p/ o detalhe do painel de rede.
fn join_headers(headers: &[(String, String)]) -> String {
    headers
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Drena o `net_rx` (`try_recv`) → upsert por `id` em `devtools.net`; marca `dirty` se algo mudou.
fn drain_devtools_net(devtools: &Rc<DevtoolsState>) {
    let rx_guard = devtools.net_rx.borrow();
    let Some(rx) = rx_guard.as_ref() else {
        return;
    };
    let mut net = devtools.net.borrow_mut();
    let mut changed = false;
    while let Ok(rec) = rx.try_recv() {
        if let Some(existing) = net.iter_mut().find(|r| r.id == rec.id) {
            *existing = rec;
        } else {
            net.push(rec);
        }
        changed = true;
    }
    drop(net);
    if changed {
        devtools.dirty.set(true);
    }
}

/// Driver de evidência do "limpar dados" (`BASEDBROWSER_CLEAR_TEST=1`, M6/ADR-0009): após a página de
/// teste setar cookie+localStorage e registrar uma visita, loga o estado ANTES, invoca
/// `clear-browsing-data` e loga DEPOIS — provando que cookies zeram e o histórico esvazia, com os
/// favoritos preservados. Sem captura de janela (L-008). Mantenha o `Timer`.
fn install_clear_test(
    app: &MainWindow,
    manager: &Rc<RefCell<Option<TabManager>>>,
    data: &Rc<RefCell<persist::AppData>>,
) -> Option<Timer> {
    std::env::var_os("BASEDBROWSER_CLEAR_TEST")?;
    let timer = Timer::default();
    let weak = app.as_weak();
    let mgr = manager.clone();
    let dat = data.clone();
    let step = Cell::new(0u32);
    timer.start(
        TimerMode::Repeated,
        Duration::from_millis(1000),
        move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            match step.replace(step.get() + 1) {
                3 => {
                    // Favorita a página atual p/ provar que o "limpar" PRESERVA favoritos.
                    app.invoke_add_bookmark();
                    eprintln!("[cleartest] add-bookmark (favorito a preservar)");
                }
                4 => log_clear_state(&mgr, &dat, "antes"),
                5 => {
                    app.invoke_clear_browsing_data();
                    eprintln!("[cleartest] clear-browsing-data invocado");
                }
                6 => log_clear_state(&mgr, &dat, "depois"),
                _ => {}
            }
        },
    );
    Some(timer)
}

/// Loga (TEXTO) o estado de dados p/ o [`install_clear_test`]: nº de cookies da aba ativa + nº de
/// sites com Web Storage + nº de entradas de histórico + nº de favoritos (estes preservados).
fn log_clear_state(
    manager: &Rc<RefCell<Option<TabManager>>>,
    data: &Rc<RefCell<persist::AppData>>,
    phase: &str,
) {
    let guard = manager.borrow();
    let Some(m) = guard.as_ref() else {
        return;
    };
    let sdm = m.servo.site_data_manager();
    let storage_sites = sdm.site_data(StorageType::all()).len();
    let cookies = m
        .active_tab()
        .and_then(|tab| tab.webview.url().map(|u| u.to_string()))
        .and_then(|s| Url::parse(&s).ok())
        .map_or(0, |url| {
            sdm.cookies_for_url(url, CookieSource::NonHTTP).len()
        });
    let d = data.borrow();
    eprintln!(
        "[cleartest] {phase}: cookies(aba)={cookies} storage_sites={storage_sites} history={} bookmarks={}",
        d.history.len(),
        d.bookmarks.len()
    );
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

# ADR-0007: Arquitetura de recursos do M4 (multi-aba, histórico, favoritos, sessão)

- **Status:** Accepted
- **Data:** 2026-06-10
- **Relaciona-se com:** **estende** (não supersede) ADR-0003 (integração M1), ADR-0004 (input/resize
  M2) e ADR-0005/0006 (render GPU zero-copy M3). Mantém o pin do ADR-0002 (`servo =0.2.0`, toolchain
  1.92.0). **Supera o ponto "sem build.rs" da AD-008** (o chrome saiu da macro inline grande p/ um
  arquivo `.slint`, mas SEM `build.rs` — ver Decisão 5).

## Contexto

O M0–M3 entregaram um browser de UMA aba: barra de URL/voltar/avançar/recarregar, input
(pointer/scroll/teclado), resize dinâmico e o frame viajando Servo→Slint por compartilhamento de
textura GPU (zero-copy, Vulkan↔GL). O M4 ("recursos de navegador") torna o browser usável no dia a
dia: **multi-aba**, **histórico de sessão** e **favoritos**, dentro dos limites de compat do Servo.

A pesquisa do M4 (jun/2026) confirmou **na fonte** (cache do cargo do `servo 0.2.0`):

- **Múltiplas `WebView`s por `Servo`** (`examples/winit_minimal.rs`: `RefCell<Vec<WebView>>`). Ciclo
  de vida: `WebViewBuilder::new(&servo, ctx).delegate(..).url(..).build()`; `show`/`hide`/`focus`/
  `blur`; **fechar = dropar o handle** (`Drop for WebViewInner` → `CloseWebView` + `remove_webview`).
  `WebView` é `Rc<RefCell<..>>` (clone barato), `id() -> WebViewId` (`Copy`+`Eq`+`Hash`).
- **Pintura por-webview** (`webview.paint()`); `servo.spin_event_loop()` 1×/tick p/ o Servo inteiro;
  **todo callback do `WebViewDelegate` recebe a `WebView`** que o disparou → roteio por `id()`.
- **`OffscreenRenderingContext`** (`servo-paint-api`): cada offscreen tem **FBO próprio**
  (`framebuffer`), mas todos compartilham o **contexto surfman do pai** (`WindowRenderingContext`):
  `make_current()` → faz o pai corrente; `prepare_for_rendering()` → liga o FBO daquela aba. Vários
  offscreen podem ser derivados de um pai (`parent.offscreen_context(size)`).
- **`set_throttled(bool)`** por-webview (pausa render de abas de fundo).
- **`request_create_new(parent, CreateNewWebViewRequest)`** (`window.open`/`target=_blank`): o
  embedder constrói a webview via `request.builder(ctx).delegate(parent.delegate()).build()` e **deve
  manter o handle vivo** (senão é destruída na hora).

## Decisão

### 1. Multi-aba: UM `Servo`, N `Tab`s, ponte GPU reusada

`Runtime` (single-WebView do M1–M3) vira **`TabManager { tabs: Vec<Tab>, active, servo, parent, wgpu,
bridge, .. }`**. Cada **`Tab`** tem sua `WebView` + seu **`OffscreenRenderingContext` (FBO próprio)** +
um `Rc<TabState>` (estado observável interior-mutável: url/title/loading/can_go_*/dirty). Todos os
offscreen derivam do **único `WindowRenderingContext` pai** (compartilham o contexto GL do surfman).

- **A ponte GPU do M3 é REUSADA, não reescrita:** a `SharedFrameTexture` (memória externa Vulkan↔GL) é
  única, criada 1× (tamanho = área web) contra o contexto pai. **Só a aba ATIVA é pintada e blitada**;
  trocar de aba = mudar a ORIGEM do blit (o FBO da aba ativa, via `prepare_for_rendering` +
  `GL_FRAMEBUFFER_BINDING`). Sem recriar a ponte na troca (só no resize). Cada aba retém seu último
  frame no FBO próprio → troca de aba instantânea. Fallback de cópia-CPU intacto.
- **Abas de fundo:** `set_active` faz `show()`+`focus()`+`set_throttled(false)` na ativa e
  `hide()`+`set_throttled(true)` nas demais; o loop só bombeia a ativa (lê o `dirty` só dela) →
  economia de CPU/GPU (compõe com o waker do T6/M3).
- **Input/navegação** vão sempre para a **aba ativa** (`with_active_webview`).

### 2. `Embedder` roteia por `id()`; escrita de UI centralizada no loop (anti-reentrância)

O `WebViewDelegate` (`Embedder`) roda DURANTE `spin_event_loop`. Para preservar o invariante
anti-reentrância do M2/M3, o `Embedder`: (a) só faz **borrow IMUTÁVEL** do `manager` (que segura via
`Weak`, sem ciclo Rc) p/ achar o `TabState` da aba pelo `webview.id()`; (b) atualiza os `Cell`/`RefCell`
desse estado; (c) marca `chrome_dirty`. **Nenhuma escrita no Slint acontece no delegate** — o LOOP, ao
ver `chrome_dirty`, re-sincroniza a aba ativa → propriedades do chrome + a barra de abas (`sync_chrome`
+ `rebuild_tabs_model`). Mutações estruturais (abrir/fechar/trocar aba) rodam em callbacks de UI
(`borrow_mut`), que são serializados com o `Timer` mas FORA do `spin_event_loop`.

### 3. `window.open` via fila diferida

`request_create_new` constrói a `WebView` no delegate (com offscreen próprio do pai, corrente no spin)
e guarda o handle, mas **adia o REGISTRO** como aba: empilha em `Embedder.pending_new` (RefCell
separado); o loop drena a fila **pós-spin** (`integrate_pending_tabs`, `borrow_mut` seguro) e ativa a
nova aba. Isso evita o `borrow_mut` reentrante do `TabManager` durante o spin.

### 4. Persistência (favoritos / histórico / sessão) em JSON

Novo módulo `persist.rs`: armazenamento sob `~/.config/basedbrowser/` (via `dirs`), **escrita atômica**
(tmp+rename), leitura tolerante a falha (arquivo ausente/JSON inválido = vazio + log; nunca paniqueia).
- `bookmarks.json` (`Vec<Bookmark>`), `history.json` (`Vec<HistoryEntry>`, dedup consecutivo + teto
  FIFO 1000, alimentado por `notify_url_changed`), `session.json` (`{ tabs: [url], active }`).
- **Sessão restaurada no start** (`init_manager`/`restore_session`): tem precedência sobre
  `BASEDBROWSER_URL` (override só afeta o fallback da home).
- **UI:** barra de favoritos (★ adiciona a página atual; clique abre; × remove); painel de histórico
  (botão ☰; lista + busca) + autocomplete na barra de URL (sugestões de histórico). `VecModel<..>`
  derivados do estado (fonte da verdade = `TabManager`/`AppData`).
- **Deps novas (Cargo.toml do crate `basedbrowser` — não é config protegida):** `serde`,
  `serde_json`, `dirs 6` (todas extraem do cache offline). `servo =0.2.0`/toolchain/lints raiz
  **intocados**.

### 5. Chrome → arquivo `ui/app.slint` (re-export inline, NÃO `build.rs`)

O chrome cresce muito no M4 (barra de abas + favoritos + painel/autocomplete de histórico). Movemos a
UI da macro inline grande (em `main.rs`) para **`ui/app.slint`**, mantendo a entrada pela macro
`slint::slint!`:

```rust
slint::slint!(export { MainWindow, TabInfo, BookmarkInfo, HistoryItem } from "../ui/app.slint";);
```

**NÃO** usamos `build.rs` + `slint::include_modules!()` de propósito: aquele caminho injeta o `app.rs`
gerado pelo Slint como **código-fonte do crate**, e o gerado usa `.unwrap()`/`.expect()` à vontade →
**640 erros** nos lints `deny` do projeto (`unwrap_used`/`expect_used`), exigindo espalhar `#[allow]`
em código gerado (briga com `allow_attributes` e a filosofia do projeto). A **expansão da macro inline
(crate externo) é isenta do clippy**, então o gate fica verde sem `#[allow]`. Resultado: UI num arquivo
`.slint` (LSP/preview) com o gate intacto — entrega o objetivo da AD-008 sem o seu custo.

## Alternativas rejeitadas

- **`build.rs` + `include_modules!()`** (para o chrome): rejeitado — quebra o gate de lint do projeto
  (ver Decisão 5). O re-export inline dá o mesmo benefício (UI separada) sem o custo.
- **Contexto offscreen ÚNICO compartilhado por todas as abas** (em vez de um por aba): seria mais
  simples (a ponte GPU nem mudaria de origem), mas perde a retenção de frame por aba (flicker do frame
  antigo ao trocar de aba). Mantido como **fallback** se o offscreen-por-aba se mostrar instável (L-004).
- **Criar/registrar a aba de `window.open` direto no delegate:** `borrow_mut` reentrante do
  `TabManager` durante o spin → pânico. Por isso a fila diferida (Decisão 3).
- **Fechar a última aba encerra o app:** rejeitado no M4 — `close_tab` recusa fechar a última aba
  (mantém o browser sempre usável e a sessão não-vazia).

## Consequências

- (+) Browser usável no dia a dia: multi-aba (abrir/fechar/trocar; `window.open`), histórico
  persistido com painel + autocomplete, favoritos persistidos, restauração de sessão.
- (+) Reusa a ponte GPU do M3 (zero-copy) sem reescrever o interop; só a aba ativa renderiza; abas de
  fundo throttled (economia, compõe com o waker).
- (+) Gate de lint intacto (sem `#[allow]` em código gerado); persistência tolerante a falha.
- (−) Superfície de churn maior (múltiplas WebViews/contextos; classe do L-004 ao abrir abas em
  runtime — mitigado criando offscreen com o pai corrente). Embedding ainda fino (re-exports `servo::`).
- (−) Histórico grava a cada visita (arquivo pequeno; aceitável). Restauração de sessão precede o
  `BASEDBROWSER_URL` (intencional).

## Fontes (jun/2026)

- Servo 0.2.0 (cache): `webview.rs` (`WebView`/`WebViewBuilder`, `show`/`hide`/`focus`/`set_throttled`,
  `Drop` = `CloseWebView`), `webview_delegate.rs` (trait `WebViewDelegate`, `request_create_new`/
  `CreateNewWebViewRequest`), `servo.rs` (`spin_event_loop`), `servo-base/id.rs` (`WebViewId`
  `Copy`+`Eq`+`Hash`), `servo-paint-api/rendering_context.rs` (`OffscreenRenderingContext` FBO próprio
  + contexto pai compartilhado), `examples/winit_minimal.rs` + `tests/webview.rs` (padrão multi-WebView).
- Slint 1.16.1: `VecModel`/`ModelRc`/`Model::row_data`/`row_count`; macro `slint!` resolve paths
  relativos ao `.rs` (toolchain ≥1.88); `ListView`/`for`/`if` em `.slint`.

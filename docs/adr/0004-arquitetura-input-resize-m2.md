# ADR-0004: Arquitetura de input e resize do M2 (browser navegável)

- **Status:** Accepted
- **Data:** 2026-06-10
- **Relaciona-se com:** ADR-0003 (integração M1: Slint hospeda o Servo via `OffscreenRenderingContext`
  + cópia-CPU), AD-001 (stack), L-004 (init lazy do GL). **Não supersede** ADRs anteriores; estende
  a arquitetura do M1 para input/chrome/resize.

## Contexto

O M2 ("browser navegável") precisa tornar a imagem estática do M1 **interativa e dirigível**: input
(pointer/scroll/teclado) encaminhado ao Servo, chrome mínimo (barra de URL + voltar/avançar/recarregar
+ indicador de carregamento) e resize dinâmico da viewport web. A pesquisa (jun/2026) confirmou na
fonte do `servo 0.2.0` (cache do cargo) e no exemplo oficial `slint-ui/slint/examples/servo`:

- **Input da `WebView`:** `notify_input_event(InputEvent)` (`MouseButton`/`MouseMove`/`Wheel`/
  `Keyboard`/`Touch`), `notify_scroll_event(Scroll, WebViewPoint)`, e `load`/`reload`/`go_back`/
  `go_forward`/`can_go_*`. `servo::` re-exporta `embedder_traits::*` **e** `keyboard_types::{Key,
  KeyState, Code, Location, Modifiers, NamedKey}` → tradução de teclado **sem dependência nova**.
- **Resize:** `webview.resize(PhysicalSize)` → (compositor `painter.rs::resize_rendering_context`)
  `make_current` + `OffscreenRenderingContext::resize` (recria o FBO) + atualiza o viewport (reflow)
  + repaint. Faz **tudo** num só passo.
- **Slint 1.16.1:** `TouchArea` (`pointer-event`/`scroll-event`/`mouse-x`/`mouse-y`), `FocusScope`
  (`key-pressed`/`key-released`, `KeyEvent{text,modifiers,repeat}`), `LineEdit`/`Button`. Passar
  `self.mouse-x/y` (logical) a um parâmetro `physical-length` faz o Slint **converter pelo scale
  factor** automaticamente.

## Decisão

1. **Tradução de input no Rust, decodificação no `.slint`.** O `.slint` apenas decodifica cada
   evento para **primitivos** (x/y, kind/button como `int`, texto + flags de modificador) e chama um
   callback; `src/input.rs` traduz primitivo → `InputEvent`/`Scroll` do Servo. **Não** passamos os
   structs de evento do Slint ao Rust — isso desacopla o Rust dos tipos do Slint e funciona igual com
   a macro inline `slint::slint!{}` (sem `build.rs`).

2. **Mapeamento de coordenadas = identidade (sem letterbox).** Os callbacks declaram x/y como
   `physical-length` (Slint aplica o scale factor → device pixels). A `Image` da área web usa
   `image-fit: fill` e o `OffscreenRenderingContext` do Servo é dimensionado **igual à área web**;
   logo 1 px da Image = 1 device-px do Servo. Substitui o `image-fit: contain` do M1 (que
   letterboxava) — decisão deliberada para o mapeamento ser exato.

3. **Teclado:** porte do `key_event_util` do exemplo oficial — `slint::platform::Key` (teclas
   especiais via char de uso-privado em `KeyEvent.text`) → `NamedKey`; texto de 1 char →
   `Key::Character`. `Code::Unidentified` (o Slint não expõe o code físico). Variantes confirmadas
   na fonte do `slint 1.16.1`.

4. **Resize só do contexto OFFSCREEN.** No resize da área web chamamos **apenas** `webview.resize()`
   (redimensiona o FBO offscreen + reflui). O `WindowRenderingContext` **pai NÃO é tocado**:
   resize concorrente das duas superfícies GL (femtovg do Slint + surfman do Servo) na mesma janela
   é a classe de bug do **L-004**; como nunca damos `present()` no pai (lemos o FBO offscreen), o
   tamanho dele é irrelevante para o readback. O contexto pai é criado no tamanho da janela inteira;
   o offscreen, no tamanho da área web (exclui a toolbar).

5. **Chrome dirigido pelo `WebViewDelegate`.** O delegate (`Embedder`) reflete o estado do motor nas
   propriedades Slint via um handle fraco da janela: `notify_load_status_changed` → `loading`;
   `notify_url_changed` → `page-url`; `notify_history_changed`/load → `can_go_back/forward`;
   `notify_page_title_changed` → título. Roda na main thread durante `spin_event_loop` e só toca o
   `app` + a `WebView` recebida (nunca o `RefCell` do runtime) → sem borrow reentrante.

6. **Waker real adiado.** O `slint::Timer` ~60 Hz (provado no M1) segue dirigindo o `spin_event_loop`;
   o `PeriodicWaker` continua no-op. Otimização de CPU ocioso fica para uma tarefa futura (não é
   critério do M2).

## Consequências

- (+) Browser **interativo e navegável** ponta-a-ponta sobre o pipeline do M1.
- (+) Mapeamento de coordenadas **exato** e independente de DPI/letterbox.
- (+) Evolui para o M3 **sem mudança**: o caminho de input/chrome/resize independe do readback (que
  no M3 troca de cópia-CPU por export de textura GPU).
- (−) Cópia-CPU por frame continua o gargalo conhecido até o M3 (AD-003/ADR-0003).
- (−) A coexistência GL no **resize** é um ponto a vigiar (mitigado por mexer só no offscreen;
  verificado sem corrupção/tela-branca nesta sessão).
- (−) `Code::Unidentified` pode limitar atalhos que dependem do code físico (aceito no M2).

## Evidência (jun/2026)

- Smoke-launch automatizado: runtime do Servo inicia, área web dimensionada a **1024×724** (exclui a
  toolbar), frame íntegro, **sem erros de GL** (L-004 não regrediu). Dumps in-app:
  `/tmp/m2-start-frame.png` (página inicial renderizada **com texto digitado no `<input>`** →
  prova pointer+teclado).
- Verificação interativa (scroll, clique→navegação, voltar/avançar, recarregar, resize) confirmada
  com a janela rodando (captura de janela automatizada segue bloqueada no GNOME 46/Wayland — ADR-0003).

## Fontes (jun/2026)

- `servo-0.2.0/webview.rs`, `servo-embedder-traits-0.2.0/input_events.rs`, `lib.rs`;
  `servo-paint-api-0.2.0/rendering_context.rs`, `servo-paint-0.2.0/painter.rs` (cache do cargo).
- Exemplo oficial: `github.com/slint-ui/slint/tree/master/examples/servo` (`webview.slint`,
  `events_utils/{key_event_util,pointer_event_util}.rs`).
- Slint 1.16: `TouchArea`/`FocusScope`/`LineEdit`, `i-slint-core-1.16.1` (`platform::Key`).

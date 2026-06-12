# ADR-0012: Redesign da UI (chrome "dark refinado") + UX de navegação (M9)

- **Status:** Accepted
- **Data:** 2026-06-11
- **Relaciona:** ADR-0004 (input→primitivos no `.slint`, tradução no `input.rs`); ADR-0007 (chrome em
  `ui/app.slint` via re-export inline da macro — **L-007**, sem `build.rs`; invariante anti-reentrância);
  ADR-0010 (devtools/`evaluate_javascript`). Não supersede nenhum — estende a UI.

## Contexto

O BasedBrowser (M0–M8 ✅) era funcional, multi-aba, persistente e inspecionável, mas o **chrome era
cru/feio** (feedback do usuário: *"a UI está horrível"*): botões de texto (`Recarregar`/`Limpar dados`/
`DevTools`), glifos soltos, cores ad-hoc, sem hierarquia. Faltavam ainda affordances do dia a dia
(atalhos, find-in-page, zoom, menu de contexto, favicons). O M9 **repagina o chrome** (direção **dark
refinado**, desenhada no Pencil → traduzida p/ Slint) e **acopla a UX de navegação**, **sem regredir
função** (todos os structs/props/callbacks da ponte Rust↔Slint preservados; embedding fino mantido — L-001).

A pesquisa NA FONTE (Servo 0.2.0) reposicionou 2 itens do escopo:
- **Find-in-page:** o Servo 0.2.0 **não expõe busca nativa** ao embedder (nenhum método/callback).
- **Páginas de erro:** premissa do spec ERRADA — o Servo **já renderiza** sua própria página de erro
  (`neterror.html`/`badcert.html`/`crash.html`); **não é tela branca**. O embedder **não** recebe sinal de
  erro (`LoadStatus` só tem `Started`/`HeadParsed`/`Complete`; o `NetworkError` é engolido no `script`,
  TODO upstream #5463). Detectar a falha p/ injetar nossa própria página é **inviável** na API estável.

## Decisão

**Direção visual (aprovada pelo usuário, mock no Pencil `designs/based-browser.pen`):** "dark refinado" —
base slate `#15151b` + acento indigo `#6c5ce7`, cantos 8/6/12px, omnibox arredondada com cadeado, abas-
pílula, botões em ÍCONE, e um menu overflow `⋯` que recolhe Histórico/Limpar/DevTools/Zoom/Find.

1. **Tokens centralizados** num `global Theme` no `app.slint` (cores/raios — fonte da verdade do tema,
   espelha o `spec.md` e o mock). **Componentes reutilizáveis:** `IconBtn` (glifo + estados hover/disabled),
   `LockIcon` (cadeado **desenhado** com primitivas → monocromático, respeita o tema, http/https), `MenuItem`
   (item do `⋯` com dica de atalho), `TextBtn`/`SearchField` (botão/campo temáticos dos painéis).
2. **Omnibox** = `TextInput` cru (sem o chrome do `LineEdit` do std-widgets) num retângulo arredondado com
   cadeado + `★` + placeholder; a borda acende com `accent` no foco. `page-secure` é derivado no Rust
   (`sync_chrome`, `url.starts_with("https://")`) — o Slint não faz parsing de URL.
3. **Menu `⋯` / find bar / menu de contexto** modelados como **overlays dirigidos por `bool` property**
   (`menu-open`/`find-open`/`ctx-open`) sobre a área web — **mesmo padrão** do painel de histórico/devtools
   (M4/M7), em vez de `PopupWindow`. Evita quirks de posicionamento/close-on-blur e mantém consistência.
4. **Atalhos de chrome** (Ctrl+T/W/L/R/Tab/F, Ctrl +/−/0, Esc) interceptados no `on_forward_key`
   (`main.rs`) **ANTES** do repasse ao Servo — "roubam" a tecla da página (swallow no press E no release;
   ação só no press inicial). Reusam os callbacks existentes (`invoke_*`). Como o code físico não é exposto
   (Slint → `Code::Unidentified`, **ADR-0004**), os atalhos usam `text`+modificadores; Tab/Escape comparados
   contra a representação do `slint::platform::Key` (`input::is_tab`/`is_escape`). Ctrl+L foca a omnibox via
   callback `focus-url-bar` **tratado no próprio Slint** (`omni-input.focus()`).
5. **Zoom** via `WebView::set_page_zoom`/`page_zoom` (API nativa; clamp [0.3, 5.0], passo aditivo 10%),
   por-aba (`TabState.page_zoom`), refletido como `zoom-percent: int` no menu `⋯` (Ctrl +/−/0 + botões).
6. **Find-in-page por INJEÇÃO de JS** (`setup_find`): sem API nativa, um script TreeWalker auto-contido
   (injetado via `evaluate_javascript`, o mesmo caminho do devtools eval) destaca as ocorrências
   (`<mark class=bb-find>`), navega entre elas (estado em `window.__bbq`/`__bbi`) e devolve `"idx,count"`.
   O callback é **assíncrono e NÃO escreve no Slint** (invariante do ADR-0007): guarda o resultado num
   `FindState` + `dirty`, e um `Timer` dedicado reflete em `find-index`/`find-count`. `find-close` limpa os
   destaques.
7. **Favicons:** `WebViewDelegate::notify_favicon_changed` → `WebView::favicon()` (`Option<Ref<Image>>`,
   `embedder_traits::Image` = `servo::Image`) → conversão **à mão** p/ `slint::Image` (`favicon_to_slint`:
   RGBA8/BGRA8 com swap R/B; `SharedPixelBuffer<Rgba8Pixel>` + `Image::from_rgba8`). Guardado em
   `TabState.favicon` (interior-mutável + `chrome_dirty`), exposto como `icon` no `TabInfo`, renderizado no
   pill (fallback = dot colorido).
8. **Menu de contexto** (right-click) modelado SÓ no Slint: o handler de `pointer-event` da área web
   intercepta o botão direito (não repassa ao Servo) e abre o overlay no ponto do clique; itens reusam
   `go-back`/`go-forward`/`reload`/`toggle-devtools`.
9. **Páginas de erro = aceitar o padrão do Servo** (decisão do usuário): o Servo já mostra uma página de
   erro (não é branca); **sem override de recurso** no M9. O tema próprio de erro fica **deferido** (destrava
   se/quando o Servo expuser um sinal de erro ao embedder — upstream #5463).

## Consequências

- (+) Chrome apresentável (dark refinado) **sem regressão de função** — gate verde (build/clippy
  `-D warnings`/test) a cada um dos 9 commits atômicos; CI verde. UX do dia a dia (atalhos/zoom/find/menu/
  context/favicon) acoplada.
- (+) **Nenhuma dep nova** (cadeado desenhado; swap BGRA à mão; find/zoom/context reusam APIs já presentes).
  Config protegida (pin/toolchain/lints/`.claude`/ADRs) intocada. L-007 (re-export inline) preservado.
- (+) `global Theme` + componentes → mudar o tema é um ponto só; os `Button`/`LineEdit` do std-widgets
  (que destoavam) saíram dos painéis (`TextBtn`/`SearchField` temáticos).
- (−) **Find-in-page é por injeção de JS** (sem API nativa): muta o DOM da página (reversível) e os
  destaques podem ser imperfeitos em páginas complexas; re-destaca a cada tecla (custo em páginas grandes).
  v1 aceitável (caveat). Não há WebSocket/regex.
- (−) **Favicon sem un-premultiply** (dispensado: ícone de 15px, diferença só em bordas semitransparentes);
  formatos K8/KA8/RGB8 ignorados (mostra o dot). A URL do favicon não é exposta — só a imagem decodificada.
- (−) **Right-click é do chrome** (sobrepõe o menu de contexto custom de páginas — padrão de browsers,
  aceito; o Servo não dá um sinal limpo p/ diferenciar).
- (−) **Sem página de erro temática** no M9 (aceito o padrão do Servo) — deferido.
- **Verificação (L-008, sem captura de janela):** design aprovado por **screenshot do Pencil**;
  implementação por **gate verde + CI + smoke manual do usuário** (`cargo run -p basedbrowser`) + drivers de
  texto onde aplicável (`scripts/m9/`).

## Fontes (Servo 0.2.0, cache do cargo)

- `WebView::favicon` / `set_page_zoom` / `page_zoom` — `servo-0.2.0/webview.rs:329,572,580`.
- `WebViewDelegate::notify_favicon_changed` — `servo-0.2.0/webview_delegate.rs:903`.
- `Image` / `PixelFormat` / `Image::data()` — `servo-embedder-traits-0.2.0/lib.rs:373,358,401`
  (re-export `servo::*`).
- `LoadStatus` (sem variante de erro) + `NetworkError` engolido — `servo-embedder-traits-0.2.0/lib.rs:771`;
  `servo-script-0.2.0/dom/servoparser/mod.rs:1528` (TODO #5463).
- `slint::platform::Key` (Tab/Escape) — usado em `input.rs` (`is_tab`/`is_escape`).

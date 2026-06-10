# ADR-0003: Arquitetura da integração Slint↔Servo no M1 (cópia-CPU via OffscreenRenderingContext)

- **Status:** Accepted
- **Data:** 2026-06-10
- **Relaciona-se com:** AD-001 (stack Slint+Servo), AD-003 (render começa por cópia-CPU),
  ADR-0002 (pin `servo 0.2.0`). Não supersede ADRs anteriores.

## Contexto

O M1 ("MVP: Slint hospeda o Servo") precisa dos **primeiros pixels ponta-a-ponta**: uma janela
Slint exibindo uma página renderizada pelo Servo (URL fixa, cópia-CPU). A pesquisa (jun/2026)
confirmou na fonte:

- **Referência canônica (AD-001):** o exemplo oficial `slint-ui/slint/examples/servo` + o repo
  `slint-ui/servo-integration`. Slint é **dono do event loop**; o Servo renderiza numa superfície
  **offscreen separada** e o frame viaja Servo→Slint (M1 = cópia-CPU; M3 = export GPU/dma-buf→wgpu).
- **Servo 0.2.0 (fonte local do crate):** `RenderingContext::read_to_image(rect) -> RgbaImage`
  (RGBA8, flip vertical já aplicado); `OffscreenRenderingContext` via
  `WindowRenderingContext::offscreen_context(size)` (`present()` é no-op; lê um FBO de GL);
  `WindowRenderingContext::new(display_handle, window_handle, size)` exige handles de janela.
  A sequência de leitura canônica está em `servo-paint/screenshot.rs` + `painter.rs`.
- **Slint 1.16.x:** `Window::window_handle()` (feature `raw-window-handle-06`) expõe
  `HasWindowHandle`+`HasDisplayHandle`; `SharedPixelBuffer<Rgba8Pixel>` → `Image::from_rgba8`;
  `slint::Timer` + `WebViewDelegate::notify_new_frame_ready`.

**Princípio diretor do usuário:** toda decisão deve ser **future-proof e focada no maior
desempenho possível** — pesa contra qualquer caminho de rasterização em CPU descartável.

## Decisão

1. **Dono do loop / contexto:** o **Slint** é dono do event loop e da janela (renderer femtovg/GL
   por default). O **Servo** renderiza num **`OffscreenRenderingContext`** (FBO de **GL de
   hardware**) derivado de um `WindowRenderingContext` criado a partir do handle da janela do Slint
   (feature `raw-window-handle-06`).

2. **Render por cópia-CPU (AD-003):** por frame — `webview.paint()` → `make_current` →
   `read_to_image(rect)` (RGBA8) → `SharedPixelBuffer` → `Image::from_rgba8` → `set_frame`,
   bombeado por um `slint::Timer` (~60 Hz, *pump-on-dirty* via `notify_new_frame_ready`). O
   compositor do Servo faz `prepare_for_rendering`/`make_current` internamente; `present()` é no-op
   no offscreen. `webview.show()` + `focus()` são obrigatórios (sem `show()` a pipeline fica
   "fechada" e renderiza em branco).

3. **`OffscreenRenderingContext` (hardware), NÃO `SoftwareRenderingContext`:** é o **mesmo tipo**
   que o caminho zero-copy do M3 exportará (dma-buf/Vulkan → textura wgpu). Assim M1→M3 troca
   **apenas o readback** (`read_to_image` → export de FD + import wgpu), sem reescrever a
   arquitetura. O `SoftwareRenderingContext` (rasterização em CPU, descartável) foi explicitamente
   **rejeitado** salvo como último recurso — não foi necessário.

4. **LAZY-init do contexto do Servo:** o contexto GL do Servo é montado **alguns ticks após o loop
   subir** (`INIT_DELAY_TICKS`), e **NÃO** dentro de `set_rendering_notifier(RenderingSetup)`. Ver
   a lição abaixo.

## Lição decisiva (raiz do bug "frame em branco")

Inicializar o contexto do Servo (`WindowRenderingContext::new` + `make_current`) **dentro do
`set_rendering_notifier(RenderingSetup)`** — ou seja, no meio do setup do renderer do Slint
(femtovg/GL) — **corrompe o estado de GL compartilhado** da janela: o femtovg emitia
`(1281) Error on render prepare - Invalid value` / `(1282) render done - Invalid operation`, e o
Servo, embora completasse o load da página (`LoadStatus::Complete`), produzia frames em branco
(`read_to_image` → RGBA puro 255). **Adiar o init** para fora do setup do femtovg
(`INIT_DELAY_TICKS` ticks depois) elimina os erros e faz os **dois renderers de hardware coexistirem
na mesma janela**. A sequência de leitura segue `servo-paint/screenshot.rs`
(`paint` → `make_current` → `read_to_image`).

## Consequências

- (+) **Primeiros pixels ponta-a-ponta provados** (janela Slint exibindo HTML/CSS do Servo —
  evidência: `/tmp/m1-window.png` e `/tmp/m1-servo-frame.png`).
- (+) Arquitetura **evolui para o M3** (GPU/zero-copy) trocando só o readback; o contexto e o loop
  permanecem.
- (−) **Cópia-CPU por frame é gargalo conhecido** até o M3 (aceito em AD-003).
- (−) A coexistência de GL (femtovg do Slint + surfman do Servo) na **mesma janela** é sensível à
  **ordem de init** — mitigada por `INIT_DELAY_TICKS`, mas é um ponto frágil a vigiar em updates do
  Servo/Slint. (O caminho future-proof real, no M3, é o *texture sharing* do exemplo oficial.)
- (−) Captura de **janela** automatizada está bloqueada nesta sessão GNOME 46/Wayland
  (`org.gnome.Shell.Screenshot` nega; `import`/X11 não vê janela Wayland). Evidência via dump
  in-app do frame (`BASEDBROWSER_DUMP_FRAME=<path>`) + screenshot manual.

## Fontes (jun/2026)

- Exemplo oficial: `github.com/slint-ui/slint/tree/master/examples/servo` + `slint-ui/servo-integration`.
- Sequência de readback: `servo-paint-0.2.0/screenshot.rs`, `painter.rs` (cache local do cargo).
- Slint: `Window::window_handle` (feature `raw-window-handle-06`), `set_rendering_notifier`,
  `SharedPixelBuffer`/`Image::from_rgba8` — docs.slint.dev (1.16).
- Servo 0.2.0: `OffscreenRenderingContext`, `WindowRenderingContext`, `RenderingContext`,
  `WebView::{paint,show,focus,take_screenshot}` (fonte do crate).

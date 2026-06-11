# Roadmap

**Current Milestone:** M4 ✅ concluído — próximo: M5 (a definir; ver Future Considerations)
**Status:** M0–M4 ✅ concluídos em 2026-06-10

---

## M0 — Fundação & PoC do Motor ✅ CONCLUÍDO (2026-06-10)

**Goal:** Provar que o Servo compila e renderiza na máquina-alvo, isolado, antes de envolver o Slint. De-risking do maior ponto de incerteza do projeto.
**Target:** Exemplo mínimo `servo + winit` rodando localmente e abrindo uma página. **Atingido** — `crates/servo-poc`.

### Features

**Setup do projeto & toolchain** - DONE

- Repositório git + estrutura Cargo
- Deps de sistema do Servo no Ubuntu 24.04 validadas/instaladas (18 pkgs apt)
- Revisão fixada: `servo 0.2.0` (crates.io) + toolchain `1.92.0` (ADR-0002)

**PoC do motor isolado** - DONE

- `servo 0.2.0` compilado (build 7m20s)
- Exemplo mínimo `winit + WebView` portado (`crates/servo-poc`, embedding fino)
- URL aberta e **render confirmado** numa janela winit pura (sem Slint) — screenshot

---

## M1 — MVP: Slint hospeda o Servo ✅ CONCLUÍDO (2026-06-10)

**Goal:** Primeiros pixels ponta-a-ponta: uma janela Slint exibindo conteúdo renderizado pelo Servo (URL fixa, cópia-CPU). **Atingido** — `crates/basedbrowser` (Slint 1.16.1 + `servo` 0.2.0). Evidência: janela Slint exibindo HTML/CSS do Servo (screenshot confirmado pelo usuário). Detalhes em **ADR-0003**.

### Features

**Bridge de event loop** - DONE

- Slint dono da janela/loop (backend winit, renderer femtovg/GL)
- `EventLoopWaker` do Servo + `slint::Timer` (~60 Hz) dirigindo `spin_event_loop`; `WebViewDelegate::notify_new_frame_ready` → pump-on-dirty

**Render via cópia-CPU** - DONE

- Servo renderiza num **`OffscreenRenderingContext`** (FBO de GL de hardware) derivado da janela do Slint (feature `raw-window-handle-06`)
- `read_to_image` (RGBA8) → `SharedPixelBuffer` → `Image::from_rgba8` → `set_frame` a cada frame
- URL fixa via `file://` (HTML/CSS auto-contido) exibida dentro da UI Slint
- **Lição (ADR-0003):** init do contexto do Servo é LAZY (fora do `RenderingSetup` do femtovg) p/ não corromper o GL compartilhado

---

## M2 — Browser navegável ✅ CONCLUÍDO (2026-06-10)

**Goal:** Deixa de ser uma imagem estática e vira algo interativo e dirigível pelo usuário.
**Atingido** — `crates/basedbrowser` evoluiu o pipeline do M1 com input, chrome e resize. Evidência:
navegou ao **YouTube** via barra de URL (HTTPS/TLS) renderizado pelo Servo + texto digitado num
`<input>` (pointer+teclado), com scroll/voltar/avançar/recarregar/resize confirmados pelo usuário.
Decisões em **ADR-0004**. Detalhe: build **debug** + cópia-CPU por frame deixa páginas pesadas
travadas — esperado até o M3 (ver Lições/Deferred).

### Features

**Input** - DONE

- Pointer (clique/move) → `InputEvent::{MouseButton,MouseMove}`; scroll → `notify_scroll_event`
- Teclado → `InputEvent::Keyboard` (`slint::platform::Key` → `keyboard_types::NamedKey`/`Character`)
- Tradução no `src/input.rs` (decodificação a primitivos no `.slint`); mapeamento de coordenadas
  **identidade** via `physical-length` + `image-fit: fill` + contexto offscreen do tamanho da área web

**Chrome mínimo (.slint)** - DONE

- Barra de URL (`LineEdit` → `webview.load`; `parse_user_url` prefixa `https://`)
- Voltar / avançar / recarregar (`go_back`/`go_forward`/`reload`, guardados por `can_go_*`)
- Indicador de carregamento + título dinâmico, dirigidos pelo `WebViewDelegate` (`Embedder`)

**Resize dinâmico** - DONE

- `webview.resize` redimensiona só o `OffscreenRenderingContext` (FBO + reflow); o
  `WindowRenderingContext` pai NÃO é tocado (evita a colisão GL do L-004) — verificado sem corrupção

---

## M3 — Performance: render GPU ✅ CONCLUÍDO (2026-06-10)

**Goal:** Eliminar o gargalo da cópia-CPU por frame com compartilhamento de textura GPU.
**Atingido** — `crates/basedbrowser/src/gpu_bridge.rs`: o frame Servo→Slint NÃO passa mais por
cópia-CPU. Renderer do Slint trocado p/ `femtovg-wgpu` (Vulkan). Decisões em **ADR-0005** (arquitetura)
+ **ADR-0006** (validação). Input/chrome/resize do M2 intactos.

### Features

**Texture sharing Vulkan↔GL** - DONE

- Imagem Vulkan com memória externa (`OPAQUE_FD`) → FD (`vkGetMemoryFdKHR`) → import em OpenGL
  (`GL_EXT_memory_object_fd`: `glImportMemoryFdEXT`/`glTexStorageMem2DEXT`)
- Wrap como `wgpu::Texture` no lado Slint (`create_texture_from_hal::<Vulkan>` +
  `texture_from_raw(External)`) → `slint::Image::try_from`; device wgpu capturado via
  `set_rendering_notifier`
- Flip vertical (mismatch GL↔Vulkan) no `glBlitFramebuffer` + `glFinish` (sync v1)
- **Fallback** de cópia-CPU em runtime (não foi necessário)

**Benchmark cópia-CPU vs. GPU sharing** - DONE

- Harness `FrameBench` (env `BASEDBROWSER_BENCH`). Release, 1024×724, página animada @60fps:
  `pump_frame` mean **~5,4 ms (CPU) → ~3,1 ms (GPU)**, p95 ~6–9 → ~3,7 ms (**−40% média, −50% p95**)
- Evidência: readback da textura compartilhada **byte a byte idêntico** à fonte do Servo + página
  HTTPS real (example.com). Captura de janela bloqueada no Wayland → dump in-app (ADR-0003)

---

## M4 — Recursos de navegador ✅ CONCLUÍDO (2026-06-10)

**Goal:** Funcionalidades que tornam o browser usável no dia a dia (dentro dos limites de compat do Servo).
**Atingido** — `crates/basedbrowser` evoluiu o pipeline do M3 com multi-aba, histórico e favoritos.
Decisões em **ADR-0007**. Chrome migrado da macro inline grande p/ `ui/app.slint` (re-export inline,
SEM build.rs — mantém o gate de lint verde). Deps novas: `serde`/`serde_json`/`dirs`. 8 commits
atômicos (T1–T7 + T4b).

### Features

**Multi-aba** - DONE

- UM `Servo`, N `WebView`s (`TabManager`/`Tab`); cada aba com seu `OffscreenRenderingContext` (FBO
  próprio) derivado do `WindowRenderingContext` pai. Só a aba ATIVA é pintada/blitada — **reusa a ponte
  GPU zero-copy do M3** trocando só a origem do blit (FBO da ativa). Abas de fundo `set_throttled(true)`,
  não bombeadas (economia). Abrir (+)/fechar (×)/trocar (clique) na barra de abas; `window.open`/
  `target=_blank` abre nova aba (fila diferida). Input/navegação vão p/ a aba ativa.
- Evidência: abrir(1→2)→page2→trocar→fechar(2→1) com conteúdo distinto por aba (aba1 VERDE/page2 no
  FBO próprio, textura ativa final ROXO/aba0); `window.open` → 2 abas; sem panic/borrow reentrante.

**Histórico de sessão** - DONE

- Visitas gravadas (alimentadas por `notify_url_changed`), persistidas em `~/.config/basedbrowser/
  history.json` (dedup consecutivo + teto FIFO 1000). Painel (botão ☰) com lista + busca (revisitar) +
  autocomplete na barra de URL. Evidência: 8 visitas persistidas → painel popula (dedup), busca filtra,
  autocomplete sugere, revisita carrega.

**Favoritos** - DONE

- ★ adiciona a página atual; barra de favoritos (clique abre / × remove); persistidos em
  `bookmarks.json`. Evidência: ★ → arquivo com 1 entrada → 2ª execução CARREGA o favorito.

**Restauração de sessão** - DONE

- Abas abertas (URLs + índice ativo) salvas no exit, restauradas no start (`init_manager`); precede o
  `BASEDBROWSER_URL`. Evidência: RUN 1 salva 2 abas (ativa=1) → RUN 2 restaura 2 abas, ativa=1.

---

## Future Considerations

- Suporte a outras plataformas (Windows/DirectX, macOS/Metal, Android)
- Medição/perfil sistemático de RAM vs. Chromium (validar a tese central)
- Estratégia de atualização contínua do Servo (CI que testa a revisão fixada)
- Devtools / inspeção
- Política de download, gestão de cookies/armazenamento

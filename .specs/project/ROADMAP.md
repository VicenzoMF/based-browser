# Roadmap

**Current Milestone:** M2 — Browser navegável
**Status:** M1 ✅ concluído em 2026-06-10 (M0 ✅ no mesmo dia)

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

## M2 — Browser navegável

**Goal:** Deixa de ser uma imagem estática e vira algo interativo e dirigível pelo usuário.

### Features

**Input** - PLANNED

- Pointer (clique/move) winit → `WindowEvent` do Servo
- Scroll
- Teclado (digitação em formulários, atalhos)

**Chrome mínimo (.slint)** - PLANNED

- Barra de URL (digitar e navegar)
- Voltar / avançar / recarregar
- Indicador de carregamento

---

## M3 — Performance: render GPU

**Goal:** Eliminar o gargalo da cópia-CPU por frame com compartilhamento de textura GPU.

### Features

**Texture sharing Vulkan→GL** - PLANNED

- Imagem Vulkan com memória externa (FD) → import em OpenGL (`GL_EXT_memory_object_fd`)
- Wrap como textura `wgpu` no lado Slint
- Flip vertical (mismatch de coordenadas GL) + blit
- Benchmark cópia-CPU vs. GPU sharing

---

## M4 — Recursos de navegador

**Goal:** Funcionalidades que tornam o browser usável no dia a dia (dentro dos limites de compat do Servo).

### Features

**Multi-aba** - PLANNED
**Histórico de sessão** - PLANNED
**Favoritos** - PLANNED

---

## Future Considerations

- Suporte a outras plataformas (Windows/DirectX, macOS/Metal, Android)
- Medição/perfil sistemático de RAM vs. Chromium (validar a tese central)
- Estratégia de atualização contínua do Servo (CI que testa a revisão fixada)
- Devtools / inspeção
- Política de download, gestão de cookies/armazenamento

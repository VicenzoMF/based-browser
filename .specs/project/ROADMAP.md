# Roadmap

**Current Milestone:** M1 — MVP: Slint hospeda o Servo
**Status:** Planning (M0 ✅ concluído em 2026-06-10)

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

## M1 — MVP: Slint hospeda o Servo

**Goal:** Primeiros pixels ponta-a-ponta: uma janela Slint exibindo conteúdo renderizado pelo Servo (URL fixa, cópia-CPU).

### Features

**Bridge de event loop** - PLANNED

- Slint dono da janela/loop (backend winit)
- `EventLoopWaker` do Servo sincronizando frames Servo→Slint via canal

**Render via cópia-CPU** - PLANNED

- Servo renderiza em buffer offscreen (`OffscreenRenderingContext`)
- Buffer → `slint::Image` a cada frame (via `set_rendering_notifier`)
- Exibir uma URL fixa dentro da UI Slint

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

# Roadmap

**Current Milestone:** M0 — Fundação & PoC do Motor
**Status:** Planning

---

## M0 — Fundação & PoC do Motor

**Goal:** Provar que o Servo compila e renderiza na máquina-alvo, isolado, antes de envolver o Slint. De-risking do maior ponto de incerteza do projeto.
**Target:** Exemplo mínimo `libservo + winit` rodando localmente e abrindo uma página.

### Features

**Setup do projeto & toolchain** - PLANNED

- Repositório git + estrutura Cargo
- Validar/instalar deps de sistema do Servo no Ubuntu 24.04
- Definir e fixar a revisão do Servo a usar (`rust-toolchain.toml` se necessário)

**PoC do motor isolado** - PLANNED

- Compilar `libservo`
- Rodar o exemplo mínimo `winit + WebView` do Servo
- Abrir uma URL e confirmar render numa janela winit pura (sem Slint ainda)

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

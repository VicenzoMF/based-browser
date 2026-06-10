# BasedBrowser

**Vision:** Um navegador web nativo, leve e de baixo consumo de RAM, escrito em Rust, com motor de renderização próprio (Servo) em vez do webview do sistema.
**For:** Desenvolvedores e usuários avançados que querem uma alternativa enxuta ao Chrome/Electron e/ou contribuir para um browser Rust-native.
**Solves:** Browsers baseados em Chromium/Electron têm baseline de memória alto (centenas de MB só de chrome de UI + modelo de processo-por-site). O BasedBrowser busca um footprint ocioso e por-aba drasticamente menor.

## Goals

- **Footprint enxuto:** baseline de UI/chrome ordens de magnitude menor que um Electron/Chrome (Slint não embute Chromium). Métrica-alvo: medir RSS ocioso vs. Chromium numa página simples e documentar a diferença.
- **Motor Rust-native:** renderização via Servo (não wry/WebKitGTK). Métrica: render ponta-a-ponta de HTML/CSS dentro de uma janela Slint.
- **Sustentabilidade:** código de embedding fino o suficiente para acompanhar o churn do Servo sem reescritas grandes (lição do Verso). Métrica: atualizar a revisão fixada do Servo em < 1 dia de trabalho por sprint.

## Tech Stack

**Core:**

- Linguagem: Rust **1.92.0** (stable, fixado em `rust-toolchain.toml` — é a toolchain do tag v0.2.0 do Servo; ADR-0002)
- UI / chrome: Slint (declarativo, backend `winit`, renderer femtovg/GL) — **v1.16.x** (feature `raw-window-handle-06`)
- Motor web: Servo (`servo` 0.2.0 do **crates.io**, `WebView` API) — pin exato `=0.2.0` (ADR-0002)
- Janela / event loop: `winit` (Slint é dono do loop)

**Key dependencies:**

- `slint` (`set_rendering_notifier`, `RenderingState`, `GraphicsAPI`, backend winit)
- `libservo` (`WebView`, `WebViewDelegate`, `ServoDelegate`, `RenderingContext`/`OffscreenRenderingContext`, `EventLoopWaker`)
- `surfman` (contexto GL do Servo) + interop Vulkan↔OpenGL (fase GPU)
- `wgpu` (importação de textura no lado Slint, fase GPU)

## Scope

**v1 includes:**

- Janela nativa Slint hospedando uma `WebView` do Servo
- Navegação para uma URL e render de HTML/CSS (cópia-CPU primeiro)
- Input básico: clique, scroll, teclado
- Chrome mínimo: barra de URL, voltar/avançar/recarregar

**Explicitly out of scope (v1):**

- Compatibilidade web completa (Servo é incompleto — aceito conscientemente)
- Multi-aba, histórico persistente, favoritos, extensões
- Compartilhamento de textura GPU (otimização de fase posterior)
- Windows/macOS/Android (foco inicial: Linux/Vulkan, caminho melhor suportado)
- Sync, perfis, gerenciador de senhas, devtools

## Constraints

- **Técnico:** Servo agora **é** crate do crates.io (`servo` 0.2.0; ver ADR-0002/AD-006), mas ainda compila o motor inteiro + mozjs do fonte (vários GB, muitas deps de sistema via apt, 1ª compilação longa — ~5–7 min aqui). Maior custo de infraestrutura.
- **Técnico:** API do Servo muda rápido; manter revisão fixada e embedding fino.
- **Plataforma:** desenvolvimento e alvo inicial = Linux (Ubuntu 24.04), GPU via Vulkan→GL.
- **Recursos:** projeto solo / long-term; priorizar de-risking (provar o motor antes de polir UI).

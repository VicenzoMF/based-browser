# Handoff

**Date:** 2026-06-10
**Feature:** M1 — MVP: Slint hospeda o Servo ✅ CONCLUÍDO · próximo = M2 (browser navegável: input + chrome)
**Task:** M1 fechado: janela Slint exibe página renderizada pelo Servo (cópia-CPU). Iniciar M2.

## Completed ✓

- **M1 done** (critério: janela Slint exibe página do Servo, cópia-CPU, URL fixa + evidência):
  - `crates/basedbrowser` — **Slint 1.16.1** (feature `raw-window-handle-06`) + **`servo` 0.2.0**
    (mesmo pin do ADR-0002). Embedding fino, sem `unwrap`/`expect`.
  - Pipeline: Servo **`OffscreenRenderingContext`** (FBO GL hardware, do handle da janela do Slint)
    → `read_to_image` → `SharedPixelBuffer` → `Image::from_rgba8` → `set_frame`, bombeado por
    `slint::Timer` (~60 Hz, pump-on-dirty via `WebViewDelegate::notify_new_frame_ready`).
  - URL fixa via `file://` (HTML/CSS auto-contido). **Evidência confirmada pelo usuário**: janela
    com heading "BasedBrowser" + gradiente + cards flexbox (`/tmp/m1-window.png`,
    `/tmp/m1-servo-frame.png`).
  - **ADR-0003 `Accepted`** (arquitetura da integração). **L-004** (lição: lazy-init evita corromper
    o GL). **AD-007** no STATE.
  - Commits: `3a59900` (T1 Slint mínimo), `6897689` (T2 spike coexistência), `4a72007` (T3 pipeline).

## In Progress

- Nada — checkpoint limpo na `main` (T1–T4 commitados; T5 docs neste commit).

## Pending (M2 — Browser navegável)

1. **Input:** pointer (clique/move) → `WindowEvent` do Servo; scroll; teclado. (A `WebView` 0.2.0
   tem APIs de input/foco — `focus()` já usado; ver `webview.rs`.)
2. **Chrome mínimo (.slint):** barra de URL (digitar e navegar via `webview.load(url)`),
   voltar/avançar/recarregar, indicador de carregamento (`notify_load_status_changed`).
3. **Resize dinâmico:** hoje a janela é fixa (1024×768); ligar resize do Slint → `webview.resize` +
   recriar/`resize` do contexto.
4. **Waker real (opcional):** hoje o `Timer` ~60 Hz dirige tudo (`PeriodicWaker` é no-op); um waker
   que acorda o loop sob demanda reduz CPU ocioso.

## Blockers

- Nenhum ativo. Pendências humanas (não bloqueiam M2):
  - **Reavaliar escopo dos feedback-hooks** (L-003): `basedbrowser` agora puxa o `servo`; cache
    aquecido mantém checks rápidos (clippy ~0.7s), mas 1ª build fria recompila o motor. Se incomodar,
    aplicar `--exclude basedbrowser` em `gate-build.sh` + `lefthook.yml` (edição humana).
  - Prune de MCP (`/mcp`); autorizar AgentShield (L-002).

## Context

- Branch: `main`. Idioma: **pt-BR**. Plan Mode antes de executar.
- **Lição L-004 (crítica):** ao mexer no render, init do contexto do Servo é **lazy** (fora do
  `RenderingSetup` do Slint), senão corrompe o GL e a tela fica branca. Sequência de leitura:
  `paint` → `make_current` → `read_to_image`. `webview.show()`+`focus()` obrigatórios.
- **Fonte do `servo` 0.2.0 no cache do cargo** (`~/.cargo/registry/src/.../servo-*`) é a referência
  mais confiável p/ a API (mais que o GitHub) — usar grep ali ao integrar APIs novas.
- Rodar: `cargo run -p basedbrowser` (precisa de display). Dump de evidência:
  `BASEDBROWSER_DUMP_FRAME=/tmp/x.png cargo run -p basedbrowser`.
- **Captura de janela** automatizada bloqueada no GNOME 46/Wayland — usar dump in-app + screenshot
  manual.
- M3 (futuro): trocar a cópia-CPU por *texture sharing* GPU (dma-buf→wgpu), Slint no renderer wgpu —
  é a razão de o M1 usar `OffscreenRenderingContext` (não software): o tipo já é o do caminho de GPU.
- Decisões: STATE AD-001..007 · Lições: L-001..004.

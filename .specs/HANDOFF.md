# Handoff

**Date:** 2026-06-10
**Feature:** M0 — Fundação & PoC do Motor ✅ CONCLUÍDO · próximo = M1 (Slint hospeda o Servo)
**Task:** M0 fechado: Servo builda e renderiza isolado nesta máquina. Iniciar planejamento do M1.

## Completed ✓

- **M0 done** (critério: janela + render **E** ADR Accepted):
  - **ADR-0002 `Accepted`** (supersede ADR-0001): pin `servo 0.2.0` (crates.io) + toolchain `1.92.0`.
  - `rust-toolchain.toml` → `1.92.0` (ativo).
  - `crates/servo-poc` — embedder fino (winit + `servo` 0.2.0, **sem Slint**), portado do `winit_minimal.rs` (tag v0.2.0, MPL-2.0), usando re-exports `servo::`, sem `unwrap`/`expect`.
  - **Build verde 7m20s**; **render confirmado** (HTML/CSS/gradiente via webrender GL 4.6 Mesa Intel — screenshot em `/tmp/m0-shot.png`).
  - Feedback-hooks escopados (`--exclude servo-poc`) p/ não recompilar o motor.
  - Commits: `f6fa545` (ADR+toolchain), `8faca6e` (hooks), `6383c4f` (servo-poc). `Cargo.lock` fixa 861 pkgs.

## In Progress

- Nada — checkpoint limpo na `main`.

## Pending (M1 — Slint hospeda o Servo, cópia-CPU)

1. **Pesquisar (context7 + web)** a API atual do Slint p/ embutir um buffer externo: `set_rendering_notifier`, `RenderingState`, `slint::Image` a partir de pixels, backend winit; reler o post "Using Servo with Slint" (AD-001).
2. **Bridge de event loop:** Slint dono da janela/loop; `EventLoopWaker` do Servo sincroniza frames Servo→Slint via canal.
3. **Render cópia-CPU:** trocar `WindowRenderingContext` por **`OffscreenRenderingContext`** (já re-exportado por `servo::`, ver `components/servo/lib.rs`); ler o buffer offscreen → `slint::Image` por frame; exibir URL fixa dentro da UI Slint.
4. Decidir crate: novo (ex.: `crates/basedbrowser` ganha Slint) vs. evoluir a PoC. Manter embedding fino (L-001).

## Blockers

- Nenhum ativo. Pendências humanas (harness, não bloqueiam M1): prune de MCP (`/mcp`); autorizar AgentShield (L-002); se quiser, revisar a 1 linha do `gate-build.sh` aplicada por você (L-003).

## Context

- Branch: `main` · árvore limpa. Idioma: **pt-BR**. Plan Mode antes de executar.
- **Servo mudou desde a memória antiga:** agora é crate no crates.io + toolchain **stable** (não nightly) → integração bem menos arriscada (ver AD-006/ADR-0002). Deps de sistema (apt) seguem obrigatórias; 1ª compilação cara mas viável (~7min aqui).
- `servo-poc` roda com `cargo run -p servo-poc -- <url>` (precisa de display; no Wayland a janela abre na tela real). Build isolado em `target/poc` (`CARGO_TARGET_DIR`).
- Decisões: STATE AD-001..006 · Lições: L-001 (churn Verso), L-002 (AgentShield), L-003 (classifier x hooks).

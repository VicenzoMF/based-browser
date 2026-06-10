# Handoff

**Date:** 2026-06-10
**Feature:** M0 — Fundação & PoC do Motor (próximo) · harness H1–H4 infra (concluído)
**Task:** Iniciar M0 — provar build/render do Servo isolado, antes do Slint

## Completed ✓

- Planejamento spec-driven em `.specs/project/` (PROJECT, ROADMAP, HARNESS-ROADMAP, STATE).
- Harness H1–H4 (toda a infra independente do produto) construído, testado e commitado:
  - Lints Rust (`Cargo.toml`), `clippy.toml`, `rust-toolchain.toml` pin, rustfmt PostToolUse hook.
  - PreToolUse `protect-config` + `safety-bash`; Stop `gate-build`; SessionStart `session-context`.
  - `.claude/settings.json` permissions.deny; lefthook v2.1.9 instalado (pre-commit clippy/fmt).
  - `sandbox/` skeleton (sem egress); `docs/harness-metrics.md`; `docs/adr/0001` (Proposed).
- Commits: `7c0103e` (H1), `8f15f7a` (H2–H4).

## In Progress

- Nada em andamento — checkpoint limpo.

## Pending (M0 — nesta ordem)

1. Pesquisar (context7 + pageboy + web; a API do Servo muda rápido) a forma atual de consumir `libservo`/`WebView` + o exemplo mínimo `winit`.
2. Definir a revisão git do Servo a fixar + a toolchain Rust que ela exige → **ADR** (promover 0001 → Accepted ou criar 0002 que o supersede).
3. Levantar deps de sistema (Ubuntu 24.04) + estimar custo da 1ª compilação.
4. Compilar `libservo` + rodar o exemplo mínimo abrindo uma URL em janela `winit` pura (SEM Slint).

## Blockers

- Nenhum ativo. Pendências humanas: prune de MCP (`/mcp`), autorizar AgentShield (ver L-002).

## Context

- Branch: `main` · árvore limpa (tudo commitado).
- **Servo NÃO é crate do crates.io:** build pesado (vários GB, deps via apt, provável `sudo`) → **NÃO buildar sem aprovar a receita antes.**
- `rust-toolchain.toml` e a revisão do Servo = **config protegida** (mudar via ADR; o hook pede confirmação).
- Decisões: STATE.md AD-001..005 · Lições: L-001 (churn do Verso), L-002 (AgentShield/sandbox).
- Idioma de trabalho: **pt-BR**. Use Plan Mode antes de executar.

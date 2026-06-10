# BasedBrowser — Agent Guide

Browser leve em Rust (Slint + Servo). **Plano / fonte de verdade:** `.specs/project/`
(PROJECT.md, ROADMAP.md, HARNESS-ROADMAP.md, STATE.md). **Decisões imutáveis:** `docs/adr/`.
Leia `STATE.md` no início de cada sessão.

## Comandos
- Build:  `cargo build --workspace`
- Lint:   `cargo clippy --workspace --all-targets -- -D warnings`
- Format: `cargo fmt --all`
- Test:   `cargo test --workspace`
- Run:    `cargo run -p basedbrowser`

## Regras (proibições — resolvidas por mecanismo, não por pedido)
- NÃO use `.unwrap()` / `.expect()` fora de testes (lint = deny). Propague erros (`Result`).
- NÃO use `#[allow(...)]` pra silenciar lint; use `#[expect(..., reason = "...")]`.
- `unsafe` só com `#[expect(unsafe_code, reason = "...")]` justificando (interop GPU vem no M3).
- NÃO altere config protegida sem um ADR novo: `rust-toolchain.toml`, a seção de lints do
  `Cargo.toml`, e a revisão fixada do Servo (ADR-0001).
- NÃO use `git commit --no-verify`.

## Fluxo
Planeje antes de executar (Plan Mode). Pipeline: Research → Plan → Implement → Review → Verify.
Use `context7` para docs de libs; o motor/Servo muda rápido — confirme a API, não chute.

## Status
Marco **M3 ✅** — render GPU zero-copy: `crates/basedbrowser/src/gpu_bridge.rs` faz texture sharing
via memória externa Vulkan↔GL (renderer `femtovg-wgpu`), eliminando a cópia-CPU por frame
(`read_to_image` saiu do caminho quente). Benchmark: pump −40% (5,4→3,1 ms). Sobre o input/chrome/
resize do M2 (ADR-0004, AD-008) e o pipeline do M1. Decisões em **ADR-0005/0006, AD-009, L-006**.
Próximo: **M4** (recursos: multi-aba, histórico, favoritos). Harness **H1** ok.

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
Marco **M4 ✅** — recursos de navegador: **multi-aba** (`src/main.rs` `TabManager`/`Tab`: N `WebView`s/
1 `Servo`, cada aba com seu `OffscreenRenderingContext`; só a ATIVA é pintada/blitada → **reusa a ponte
GPU zero-copy do M3** trocando a origem do blit; abas de fundo throttled). **Histórico** + **favoritos**
+ **restauração de sessão** persistidos em JSON (`src/persist.rs`: `serde`/`serde_json`/`dirs`,
`~/.config/basedbrowser/`). UI em **`ui/app.slint`** (re-export inline, SEM `build.rs` — o gate de lint
proíbe o `include_modules!()`; ver L-007). `window.open` via fila diferida. Decisões em **ADR-0007,
AD-010, L-007**. Sobre o M3 (ADR-0005/0006), M2 (ADR-0004/AD-008) e M1 (ADR-0003). Próximo: **M5** (a
definir). Harness **H1** ok.

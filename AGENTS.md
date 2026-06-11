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
Marco **M6 ✅** — **recursos de usuário** (fecha a lacuna do dia a dia). **Persistência de cookies +
`localStorage`/`sessionStorage`**: `init_manager` (`src/main.rs`) aplica
`ServoBuilder.opts(Opts{ config_dir: Some(persist::servo_config_dir() = ~/.config/basedbrowser/servo/),
..Opts::default() })` (temporary_storage=false ⇒ persiste; Servo passa o `config_dir` p/
`new_resource_threads`+`new_storage_threads`). Mexida MÍNIMA/aditiva na API do Servo (1 ponto; L-001),
não mexe no init lazy do GL (L-004), honra `XDG_CONFIG_HOME` (ADR-0008). **"Limpar dados de navegação"**
(botão no `ui/app.slint` → `clear_browsing_data`): `clear_cookies()` + `clear_site_data(sites, Local|
Session)` via `servo.site_data_manager()` + `persist::clear_history()`; PRESERVA favoritos/sessão; em
callback de UI (fora do `spin_event_loop`; ADR-0007). **Downloads DEFERIDO** — spike concluiu inviável
na API estável do `servo 0.2.0` (embedder não vê headers de resposta; sem API de download/link/menu).
Decisões em **ADR-0009** · **AD-012** · **L-009**. Evidência (sem captura de janela, L-008): drivers
`BASEDBROWSER_{PERSIST,CLEAR}_TEST` + **`scripts/m6/`** (`verify-persist.sh`, `verify-clear.sh`,
`pages/persist.html`). Nenhuma dep nova. Próximo: **M7 = devtools/inspeção**.
Marco **M5 ✅** — **tese validada (footprint vs. Chromium)**, o Goal #1 do PROJECT. Harness de medição
reproduzível em bash (**`scripts/m5/`**: `measure.sh` soma a ÁRVORE DE PROCESSOS via
`/proc/<pid>/smaps_rollup` com PPID-walk; `run.sh` roda a matriz; `pages/{idle,heavy}.html`). Metodologia
JUSTA: BasedBrowser é **single-process** (`Opts.multiprocess` default=`false`), Chrome é multiprocess →
soma da árvore + **PSS** (métrica-título; RSS infla o Chrome), perfil limpo, headful, **release**, K=5
(mediana). Hook de produto `BASEDBROWSER_OPEN_TABS` (custo por-aba; embedding fino). **VEREDITO:** BB
mais leve em tudo — ocioso **171,1 MiB PSS (1 proc) vs Chrome 314,7 (13 proc) = 1,84×**; por-aba 5,5 vs
11,8 MiB; o "ordens de magnitude" do PROJECT é ~1,8× (não 10×). Números em **ADR-0008** · **AD-011** ·
**L-008**. Relatório interno do Servo adiado (L-001).
Marco **M4 ✅** — recursos: **multi-aba** (`src/main.rs` `TabManager`/`Tab`: N `WebView`s/1 `Servo`, cada
aba com seu `OffscreenRenderingContext`; só a ATIVA é pintada/blitada → reusa a ponte GPU zero-copy do
M3; abas de fundo throttled). **Histórico**+**favoritos**+**restauração de sessão** em JSON
(`src/persist.rs`). UI em **`ui/app.slint`** (re-export inline, SEM `build.rs`; L-007). Decisões em
**ADR-0007, AD-010, L-007**. Sobre o M3 (ADR-0005/0006), M2 (ADR-0004/AD-008) e M1 (ADR-0003).
**M6 ✅ acima**; próximo **M7 = devtools**. Harness **H1** ok.

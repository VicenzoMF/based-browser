# Handoff

**Date:** 2026-06-11
**Feature:** M5 — Validar a tese (footprint/RAM vs. Chromium) ✅ CONCLUÍDO · próximo = M6 (devtools)
**Task:** M5 fechado: harness de medição reproduzível + baseline Chrome + ADR-0008 com veredito. Iniciar M6.

## Completed ✓

- **M5 done** (critério: harness reproduzível / RSS+PSS ocioso+por-aba do BasedBrowser E do Chrome na
  MESMA metodologia / relatório versionado com a diferença / tese validada ou refutada com evidência):
  - **T1 — fixtures** `scripts/m5/pages/{idle,heavy}.html` (estáticas, sem rede/JS).
  - **T2 — hook** `BASEDBROWSER_OPEN_TABS=N` em `init_manager` (`src/main.rs`): abre N abas da mesma
    URL p/ o custo por-aba. Única mudança de produto (embedding fino, L-001). Gated por env, no-op sem ela.
  - **T3 — `measure.sh`**: soma a ÁRVORE DE PROCESSOS via `/proc/<pid>/smaps_rollup` (PPID-walk — o
    children-file está ausente no kernel), settle + K reps, mediana, JSON. **PSS** = métrica-título.
  - **T4 — `run.sh`**: matriz `{basedbrowser,chrome} × {ocioso N∈1/3/6; pesada}`, tabela + JSONL.
  - **T5 — medições** (release, headful, K=5): números coletados (abaixo).
  - **T6 — ADR-0008** (datado, imutável): metodologia + números + veredito. Relatório interno do Servo
    (`create_memory_report`) **adiado** (L-001). **AD-011** + **L-008** no STATE.
  - **Veredito — TESE VALIDADA:** ocioso **BB 171,1 MiB PSS (1 proc) vs Chrome 314,7 MiB (13 proc) =
    1,84×** (RSS ×5,2, inflado); por-aba **5,5 vs 11,8 MiB = 2,16×**; pesada 205 vs 333 MiB. O "ordens
    de magnitude" do PROJECT foi **qualificado**: é ~1,8× em PSS (métrica justa), não 10×.

## In Progress

- Nada — checkpoint limpo na `main` (T1–T7 commitados+pushados). Gate verde (fmt/clippy `--exclude
  servo-poc`/6 testes).

## Pending (próximos marcos) — re-priorizado em 2026-06-11 (usuário)

1. **M6 — recursos de usuário** (NOVO foco): **persistência de cookies + `localStorage`/`sessionStorage`**
   (âncora: setar `opts.config_dir` no `ServoBuilder`; hoje usamos `default()` sem `Opts` → nada
   persiste), **gerenciar dados** (`SiteDataManager`: `clear_cookies`/`clear_site_data`/…), e
   **downloads** (parte dura — `servo 0.2.0` NÃO tem API de download; via interceptação + salvar bytes).
   Ver ROADMAP "M6". **Pesquisa de fonte já iniciada** (config_dir/SiteDataManager/Opts confirmados).
2. **M7 — devtools/inspeção** (era M6): inspeção DOM/console via `servo-devtools`/`-traits`.
3. (deferred do M5) **Otimizar o baseline absoluto** (171 MiB ociosos); **relatório interno do Servo**
   cruzado com o RSS; (M3) **sync GPU por fence/semáforo**.

## Blockers

- Nenhum ativo. Pendências humanas (não bloqueiam): conectores globais claude.ai (só na web); 2 deny
  rules do AgentShield no `settings.json` (precisa de OK explícito); README.md na raiz (opcional).

## Context

- Branch: `main` (pushado p/ github.com/VicenzoMF/based-browser). Idioma: **pt-BR**. Plan Mode antes de executar.
- **M5 (ADR-0008 / AD-011 / L-008):** medir footprint multiprocess de forma justa = somar a árvore de
  processos + usar **PSS** (RSS infla o Chrome contando páginas compartilhadas ~13×) + mediana (robusta
  a outliers de settle) + release. BasedBrowser é single-process (`multiprocess` default=`false`); Chrome
  é multiprocess. Harness = bash (não Rust — L-001); números canônicos no ADR datado (design-for-rot),
  saída de `scripts/m5/results/` é gitignorada.
- **Reproduzir:** `cargo build --release -p basedbrowser && scripts/m5/run.sh`.
- Decisões: STATE AD-001..011 · Lições: L-001..008 · ADRs: 0001..0008.

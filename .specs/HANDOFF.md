# Handoff

**Date:** 2026-06-12
**Feature:** M9 — Redesign da UI (chrome "dark refinado") + UX de navegação ✅ CONCLUÍDO
**Task:** M9 fechado. Próximos = M10 (performance) e M11 (robustez).

## Completed ✓

- **M9 done** (critério: chrome repaginado SEM regressão de função + UX acoplada + gate/CI verde + design
  aprovado por screenshot + push):
  - **T1 (`9b60846`)** — chrome repaginado: `global Theme` (tokens) + componentes (`IconBtn`/`LockIcon`/
    `MenuItem`); abas-pílula, omnibox arredondada + cadeado, toolbar em ícones, loading fino, menu `⋯`.
  - **T6 (`c43c9ed`)** — zoom (`WebView::set_page_zoom` por aba, menu `⋯` − NN% +).
  - **T7 (`12d4a23`)** — find-in-page por **injeção de JS** (`setup_find` + TreeWalker; Servo sem API nativa).
  - **T5 (`ee8b384`)** — atalhos (Ctrl+T/W/L/R/Tab/F, Ctrl +/−/0, Esc) no `on_forward_key`; Ctrl+L → omnibox.
  - **T8 (`8b86e6d`)** — favicons (`notify_favicon_changed` → `servo::Image`→`slint::Image`).
  - **T9 (`4e6d629`)** — menu de contexto (right-click; reusa callbacks).
  - **T4 (`b40788c`)** — restyle dos overlays (`TextBtn`/`SearchField`; sem std-widgets claros).
  - **fix (`e4b19f1`)** — centralização vertical (L-012: layouts do Slint top-alinham filhos de tamanho fixo).
  - **tests (`c31cda3`)** — unit tests dos helpers puros (zoom/find).
  - **T11 (este)** — ADR-0012 + STATE(AD-015/L-012)/ROADMAP/HANDOFF/AGENTS + `.pen` + push.
  - **Verificado:** gate verde (fmt/clippy `-D warnings`/**9 testes**) a CADA commit + CI; design aprovado
    por screenshot do Pencil; smoke do usuário OK (incl. re-teste da centralização). Nenhuma dep nova; config
    protegida intocada.

## In Progress

- Nada — checkpoint limpo na `main` (push feito). CI re-roda na revisão final.

## Pending (próximos marcos)

1. **M10 — Performance & responsividade:** sync GPU por fence/semáforo (no lugar do `glFinish` do M3);
   intervalo de polling adaptativo do event-loop.
2. **M11 — Robustez & feedback:** crash de aba isolado (delegate `notify_crashed`), scroll restore.
3. **Deferidos M9:** página de erro **temática** (override de recurso do Servo — destrava se o upstream #5463
   expuser sinal de erro ao embedder); find-in-page v2 (regex/contexto/WebSocket); favicon un-premultiply.
4. Outras plataformas (Windows/DirectX, macOS/Metal, Android).

## Blockers

- Nenhum.

## Context

- Branch: `main` (github.com/VicenzoMF/based-browser). Idioma: **pt-BR**. Plan Mode antes de executar.
- **M9 (ADR-0012 / AD-015 / L-012):** redesign "dark refinado" no `ui/app.slint` (re-export inline da macro,
  L-007) + UX cirúrgica no `src/main.rs`/`input.rs`. Find por injeção de JS (Servo 0.2.0 sem busca nativa);
  erro de load sem sinal ao embedder (#5463) → tema de erro DEFERIDO (aceito o padrão do Servo). Pegadinha
  L-012: Slint top-alinha filhos de tamanho fixo → auto-centrar (root estica + box interno centrado em y).
- **Reproduzir:** `cargo run -p basedbrowser` (smoke do chrome/atalhos/zoom/find/favicon/context).
- Decisões: STATE AD-001..015 · Lições: L-001..012 · ADRs: 0001..0012.

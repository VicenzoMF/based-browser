# Handoff

**Date:** 2026-06-11
**Feature:** M6 — Recursos de usuário (cookies/Web Storage + limpar dados) ✅ CONCLUÍDO · próximo = M7 (devtools)
**Task:** M6 fechado: persistência LIGADA, "limpar dados" funcional, downloads deferido (spike). Iniciar M7.

## Completed ✓

- **M6 done** (critério: cookies+Web Storage persistem entre execuções / "limpar dados" funcional /
  spike de downloads resolvido / ADR-0009 / docs / push):
  - **T1 — persistência** (`806a941`): `init_manager` aplica `ServoBuilder.opts(Opts{ config_dir:
    Some(~/.config/basedbrowser/servo/), ..Opts::default() })`. Mexida mínima/aditiva (L-001), não
    mexe no init lazy do GL (L-004), honra `XDG_CONFIG_HOME` (ADR-0008). `persist::servo_config_dir()`.
  - **T2 — evidência persistência** (`3041fcf`): driver `BASEDBROWSER_PERSIST_TEST` + `scripts/m6/`
    (`pages/persist.html`, `verify-persist.sh`). RUN1 seta, RUN2 (mesmo perfil) lê de volta.
  - **T3 — limpar dados** (`daa0189`): botão "Limpar dados" → `clear_cookies()` +
    `clear_site_data(sites, Local|Session)` + `persist::clear_history()`; preserva favoritos/sessão;
    callback de UI (ADR-0007). Driver `BASEDBROWSER_CLEAR_TEST` + `verify-clear.sh`.
  - **T4 — ADR-0009** (`e0a8972`): persistência/privacidade, escopo do limpar, veredito do spike de
    downloads (inviável; fontes file:line). **AD-012** + **L-009** no STATE.
  - **T5 — fechar M6** (este commit): STATE/ROADMAP/HANDOFF/AGENTS + `.specs/features/m6-recursos-usuario/`.
  - **Verificado:** persist → RUN2 `cookie=42 local=persisted-99` (+ cookie do jar); clear → antes
    `cookies(aba)=1 history=1 bookmarks=1` / depois `cookies=0 history=0 bookmarks=1`.

## In Progress

- Nada — checkpoint limpo na `main` (T1–T5 commitados; push no fechamento). Gate verde (fmt/clippy
  `--exclude servo-poc`/6 testes). Nenhuma dep nova; config protegida intocada.

## Pending (próximos marcos)

1. **M7 — devtools/inspeção:** inspeção DOM/console via `servo-devtools`/`-traits`, UI mínima no chrome.
   Pesquisar a superfície de devtools exposta pelo `servo 0.2.0` (maior incerteza de API) — confirmar
   NA FONTE do cache do cargo antes de planejar.
2. (deferido do M6) **Downloads** (L-009/ADR-0009): destrava quando o Servo expuser hook de resposta/
   evento de download, ou marco dedicado com cliente HTTP próprio. **Modo privado** (`temporary_storage`).
3. (deferido do M5) baseline absoluto; relatório interno do Servo; (M3) sync GPU por fence/semáforo.

## Blockers

- Nenhum ativo. Pendências humanas (não bloqueiam): conectores globais claude.ai (só na web); 2 deny
  rules do AgentShield no `settings.json` (precisa de OK explícito); README.md na raiz (opcional).

## Context

- Branch: `main` (github.com/VicenzoMF/based-browser). Idioma: **pt-BR**. Plan Mode antes de executar.
- **M6 (ADR-0009 / AD-012 / L-009):** persistir = setar `opts.config_dir` (1 ponto no `ServoBuilder`);
  limpar = `SiteDataManager` (síncrono, borrow imutável em callback de UI) + `persist::clear_history`;
  downloads inviável na API estável (embedder não vê headers de resposta). Verificação sem captura de
  janela (Wayland, L-008): drivers gated + `python3 http.server` localhost + texto.
- **Reproduzir:** `cargo build --release -p basedbrowser && scripts/m6/verify-persist.sh` (e `verify-clear.sh`).
- Decisões: STATE AD-001..012 · Lições: L-001..009 · ADRs: 0001..0009.

# M6 — Recursos de usuário — Tasks

Commits atômicos; cada um passa o gate (clippy `-D warnings` + fmt + 6 testes); sem `--no-verify`.

| # | Tarefa | Reqs | Done when | Commit |
| - | ------ | ---- | --------- | ------ |
| T1 | Persistência via `opts.config_dir` (`init_manager` + `persist::servo_config_dir`) | PERSIST-01/02/03 | compila + gate; browser sobe | `806a941` |
| T2 | Driver `BASEDBROWSER_PERSIST_TEST` + `scripts/m6/{pages/persist.html,verify-persist.sh}` | EVID-01, PERSIST-01 | RUN2 lê `cookie=42 local=persisted-99` | `3041fcf` |
| T3 | Limpar dados: `ui/app.slint` + `clear_browsing_data()` + `persist::clear_history` + driver `BASEDBROWSER_CLEAR_TEST` + `verify-clear.sh` | CLEAR-01/02/03, EVID-01 | cookies/history→0, bookmarks preservado | `daa0189` |
| T4 | ADR-0009 (persistência/privacidade, limpar, spike de downloads deferido) | DOWN-01, ADR-01 | ADR datado com fontes file:line | `e0a8972` |
| T5 | Fechar M6: STATE/ROADMAP/HANDOFF/AGENTS + spec-artifacts + push | — | docs atualizadas; `git push` | (este) |

## Verificação (resultados)

- **T2 — persistência:** `scripts/m6/verify-persist.sh` → RUN1 `cookie=MISS local=MISS` →
  RUN2 `cookie=42 local=persisted-99` (+ `bb_test=42` do jar). ✅
- **T3 — limpar:** `scripts/m6/verify-clear.sh` → antes `cookies(aba)=1 history=1 bookmarks=1` →
  depois `cookies=0 history=0 bookmarks=1`. ✅
- **Gate:** clippy `-D warnings` (workspace --exclude servo-poc) + fmt + 6 testes verdes em cada commit.

## Notas

- T1 = única mexida na API do Servo (ServoBuilder/Opts), isolada a 1 ponto (embedding fino, L-001).
- Downloads não virou tarefa de código — spike concluiu inviável (ADR-0009 Decisão 3).
- Nenhuma dep nova; config protegida intocada.

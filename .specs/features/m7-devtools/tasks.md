# M7 — Tasks (todas ✅)

Commits atômicos na `main`. Gate verde por commit (fmt/clippy `--exclude servo-poc`/6 testes).

| # | Task | Commit | Done when (verificado) |
|---|------|--------|------------------------|
| T1 | Servidor de devtools OPT-IN + `ServoDelegate` | `da86e9f` | `BASEDBROWSER_DEVTOOLS=1` → "server started on 127.0.0.1:7000"; probe TCP recebe o root packet |
| T2 | Console in-process (`show_console_message`) | `3623eb0` | driver dumpa `console.log` (hello-42/warn/error) |
| T3 | Eval in-process (`evaluate_javascript`) | `46cf3fd` | eval `2+2→4`; `document.title→BBCONSOLE` |
| T4 | Cliente RDP de rede (`src/devtools_client.rs`) | `9d969b0` | rede `GET /data.json status=200 OK` + response header capturados |
| T5 | Painel UI (`ui/app.slint`) + fix de corrida | `1c565b1` | models do Slint populados (`dev-console`/`dev-net`); retry de listTabs |
| T6 | Harness `scripts/m7/verify-devtools.sh` | `e97bf1c` | 6 checagens ✅ em release |
| T7 | Docs (ADR-0010) + STATE/ROADMAP/HANDOFF/AGENTS + push | (este) | docs atualizados; `git push` |

**Arquitetura:** console/eval in-process (delegate + `evaluate_javascript`); rede via cliente RDP
próprio (thread dedicada → canal `mpsc` → Timer drena na thread de UI; ADR-0007). Servidor OPT-IN
(loopback, porta fixa). Decisões: **ADR-0010** · **AD-013** · **L-010**. Nenhuma dep nova.

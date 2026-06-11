# M8 — Tasks

Commits atômicos na `main`. Gate verde por commit (fmt/clippy `--exclude servo-poc`/testes + archgate).

| # | Task | Done when (verificado) |
|---|------|------------------------|
| T0 | Spec tlc (`.specs/features/m8-sustentabilidade/`) | spec.md + tasks.md escritos (este) |
| T1 | Archgate + harness de checks + gate local | `scripts/checks/archgate.sh` sai 0; divergir o pin numa cópia scratch → sai 2 com instrução; ligado no `lefthook.yml` |
| T2 | CI workflow + validação empírica | `.github/workflows/ci.yml`; `gh run watch` → **success** (1º run a frio); degradação documentada se inviável |
| T3 | Runbook + script de update | `docs/runbooks/atualizar-servo.md` + `scripts/update-servo/run.sh`; `--help` + rehearsal mecânico rodam o gate e imprimem o tempo |
| T4 | Sandbox sem egress | `docker compose config` valida; README com comando real + caveat GPU/display |
| T5 | Dry-run de bump-candidato | medição registrada (tempo vs "< 1 dia"); branch `experiment/*` revertida; nada na `main` |
| T6 | ADR-0011 + fechar docs + push | ADR-0011 + STATE/ROADMAP/HANDOFF/AGENTS; `git push`; CI verde |

**Arquitetura/decisões:** CI completo no GitHub-hosted free (prova: o próprio CI do Servo roda assim);
`actions-rust-lang/setup-rust-toolchain` respeita o `rust-toolchain.toml` (1.92.0) + cache;
`jlumbroso/free-disk-space` p/ disco; actions pinadas por SHA (L-002). Archgate acopla o pin ao ADR
(erro-como-instrução). Runbook mede o Goal #3. **ADR-0011** · **AD-014** · **L-011**. Nenhuma dep nova;
config protegida intocada.

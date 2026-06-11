# M6 — Recursos de usuário (persistência + limpar dados) — Specification

## Problem Statement

O BasedBrowser é um browser multi-aba dirigível (M0–M5 ✅), mas não persiste **cookies** nem
**`localStorage`/`sessionStorage`** entre execuções — logins e estado de site se perdem a cada
abertura, impedindo o uso no dia a dia. Não há também ação de "limpar dados de navegação" nem
downloads.

## Goals

- [x] Cookies + `localStorage`/`sessionStorage` PERSISTEM entre execuções (verificado em localhost, perfil real).
- [x] Ação "limpar dados de navegação" funcional (zera cookies/storage do Servo + nosso histórico; preserva favoritos).
- [x] Spike de downloads RESOLVIDO (implementado OU documentado+deferido com razão técnica na fonte).

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Downloads de arquivos | Spike concluiu inviável na API estável do Servo 0.2.0 (sem inspeção de resposta / API de link). DEFERIDO — ADR-0009. |
| Modo privado/efêmero (toggle `temporary_storage`) | Não é a lacuna do M6 (persistir é o objetivo). Candidato a marco futuro. |
| Limpar favoritos | Convenção de browser preserva curadoria do usuário. |

---

## User Stories

### P1: Persistência de sessão de login ⭐ MVP

**User Story**: Como usuário, quero que cookies e Web Storage sobrevivam entre execuções, para não
perder logins/estado de site.

**Acceptance Criteria**:
1. WHEN o usuário fecha e reabre o browser THEN o sistema SHALL restaurar cookies + `localStorage`/`sessionStorage` da execução anterior.
2. WHEN a plataforma não expõe diretório de config THEN o sistema SHALL seguir funcionando (sem persistência, sem falhar).
3. WHEN medindo footprint (perfil limpo via `XDG_CONFIG_HOME`) THEN o sistema SHALL honrar o override (não vazar perfil real).

**Independent Test**: `scripts/m6/verify-persist.sh` — RUN1 seta cookie+localStorage, RUN2 (mesmo perfil) lê de volta.

### P1: Limpar dados de navegação ⭐ MVP

**User Story**: Como usuário, quero limpar dados de navegação, para controlar privacidade sem perder favoritos.

**Acceptance Criteria**:
1. WHEN o usuário aciona "Limpar dados" THEN o sistema SHALL apagar cookies + Web Storage do Servo e o histórico.
2. WHEN o usuário aciona "Limpar dados" THEN o sistema SHALL PRESERVAR favoritos e a sessão de abas.
3. WHEN a limpeza roda THEN o sistema SHALL fazê-la sem violar o invariante anti-reentrância (ADR-0007).

**Independent Test**: `scripts/m6/verify-clear.sh` — antes cookies/history>0, depois cookies=0/history=0, bookmarks preservado.

### P2: Downloads (spike)

**User Story**: Como usuário, quero baixar arquivos.

**Resolução**: Inviável na API estável do Servo 0.2.0 (ADR-0009) → **deferido** com razão técnica documentada.

---

## Requirement Traceability

| ID | Story | Tarefa | Verificação | Status |
| -- | ----- | ------ | ----------- | ------ |
| PERSIST-01 | P1 persistência | T1 | verify-persist.sh (RUN2 lê valores) | Verified |
| PERSIST-02 | P1 persistência | T1 | fallback sem config_dir (código) | Verified |
| PERSIST-03 | P1 persistência | T1 | `XDG_CONFIG_HOME` honrado (verify-persist usa perfil temp) | Verified |
| CLEAR-01 | P1 limpar | T3 | verify-clear.sh (cookies/history→0) | Verified |
| CLEAR-02 | P1 limpar | T3 | verify-clear.sh (bookmarks preservado) | Verified |
| CLEAR-03 | P1 limpar | T3 | borrow imutável em callback de UI (código/ADR-0007) | Verified |
| EVID-01 | P1 ambos | T2/T3 | drivers gated + scripts/m6/ | Verified |
| DOWN-01 | P2 downloads | T4 | ADR-0009 (deferido, fonte) | Resolved (deferred) |
| ADR-01 | governança | T4 | docs/adr/0009 | Verified |

## Success Criteria

- [x] cookies + Web Storage persistem entre execuções (verify-persist.sh).
- [x] "limpar dados" zera cookies+storage+histórico e preserva favoritos (verify-clear.sh).
- [x] spike de downloads resolvido + ADR-0009 datado.
- [x] gate verde (clippy `-D warnings` + fmt + 6 testes); commits atômicos; sem deps novas.

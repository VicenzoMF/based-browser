# M7 — DevTools / inspeção in-app — Specification

## Problem Statement

O BasedBrowser (M0–M6 ✅) renderiza páginas, mas não oferece NENHUMA forma de **inspecionar** o que o
Servo executa — console, JavaScript, ou a rede. Sem isso, desenvolver sobre o motor (depurar uma página,
ver por que uma requisição falha, testar uma expressão) é cego. O M7 fecha essa lacuna **no próprio UI**,
sem depender de uma ferramenta externa (Firefox).

## Goals

- [x] Ver o **console** (`console.log/warn/error/...`) de uma página, ao vivo, no chrome.
- [x] **Avaliar JavaScript** na aba ativa (REPL) e ver o resultado — inclusive inspecionar o DOM via eval.
- [x] Ver a **rede**: requisições com método/URL/status **e o lado da RESPOSTA** (headers + payload).
- [x] Tudo **in-app**, sem Firefox externo. Decisão de escopo/segurança registrada (ADR-0010).

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Conectar Firefox externo (`about:debugging`) | O usuário pediu inspeção no nosso UI; o servidor do Servo permite, mas não é o deliverable. |
| Árvore de DOM visual / breakpoints / debugger | v2 — hoje DOM via eval. Atores existem no `servo-devtools` (deferido, STATE). |
| WebSocket/SSE na aba Rede | v2 (deferido). |
| Hardening do socket por token | Risco residual aceito (opt-in/dev/loopback); hardening deferido (ADR-0010). |

---

## User Stories

### P1: Console + eval in-app ⭐ MVP

**User Story**: Como desenvolvedor, quero ver o console e avaliar JS de uma página, para depurar sem
ferramenta externa.

**Acceptance Criteria**:
1. WHEN uma página chama `console.log/warn/error` THEN o painel SHALL mostrar a linha (com nível).
2. WHEN o dev digita uma expressão e tecla Enter THEN o sistema SHALL avaliá-la na aba ativa e mostrar o resultado.
3. WHEN a expressão referencia o DOM (`document.title`, `querySelector(...).outerHTML`) THEN o resultado SHALL refletir o DOM real.

**Independent Test**: `scripts/m7/verify-devtools.sh` — console `hello-42`; eval `2+2→4`, `document.title→BBDEVTOOLS`.

### P1: Rede (req+resp) via cliente RDP próprio ⭐ MVP

**User Story**: Como desenvolvedor, quero ver as requisições de rede com suas respostas (status/headers/
payload), no nosso UI, sem Firefox.

**Acceptance Criteria**:
1. WHEN o devtools está ligado (`BASEDBROWSER_DEVTOOLS`) e a página faz uma requisição THEN o painel SHALL listá-la (método/URL).
2. WHEN a resposta chega THEN o sistema SHALL capturar **status + response headers + payload** (lado da resposta).
3. WHEN o devtools está DESLIGADO THEN nenhum socket SHALL ser aberto (caminho normal intacto).

**Independent Test**: `scripts/m7/verify-devtools.sh` — `GET /data.json status=200 OK` + response header capturado.

### P2: Segurança do socket de debug

**User Story**: Como usuário, quero que abrir um socket de debug seja uma decisão consciente, não o padrão.

**Acceptance Criteria**:
1. WHEN o browser inicia sem `BASEDBROWSER_DEVTOOLS` THEN o servidor NÃO SHALL subir.
2. WHEN o servidor sobe THEN SHALL fazer bind só em `127.0.0.1` e autorizar a conexão explicitamente.

**Independent Test**: run sem a env → sem "server started"; com a env → "server started on 127.0.0.1:7000".

---

## Verification (sem captura de janela — Wayland, L-008)

Driver gated `BASEDBROWSER_DEVTOOLS_TEST` + `scripts/m7/verify-devtools.sh` (perfil-limpo,
`python3 -m http.server`). 6 checagens ✅ em release: console, eval ×2, rede status 200, response header,
models do painel populados. Detalhes/decisões: **ADR-0010** · **AD-013** · **L-010** (STATE).

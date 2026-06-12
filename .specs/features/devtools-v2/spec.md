# M10 — DevTools v2: painel dockado (split) + polimento + caminho de step-debug — Specification

## Problem Statement

O painel de DevTools (M7/ADR-0010, restyle M9) abria como **overlay de TELA CHEIA** que **escondia a
página**, e o conteúdo (console/rede) era cru. O usuário quer **dockado/split** (página de um lado, painel
do outro — base OU direita, com toggle), como Chrome/Firefox, + polimento de console e rede.

## Goals

- [ ] **Dock split** (base/direita, toggle): a página continua **visível e ENCOLHE** quando o painel abre
      (`web-resized` → `webview.resize` já ligado); **divisor arrastável**.
- [ ] **Polimento**: toolbar de **ícones com tooltip flutuante** (cabe no dock estreito); console
      **monospace** + filtro; rede com colunas (Método/Status/Tipo/Nome) + detalhe **Request/Response** +
      JSON indentado.
- [ ] **Caminho A (step-debug)**: afford. **"Debugger ↗"** (host:porta viva do RDP, ou instrução de
      habilitar) + runbook Firefox→RDP. **Debugger in-app DEFERIDO** (ADR-0014).
- [ ] Gate verde (build/clippy/test + CI); persistir tamanho/orientação do dock.

## Out of Scope (deferido)

| Item | Onde |
| ---- | ---- |
| Debugger in-app (breakpoints/step/call-stack) | ADR-0014 (vive no motor; via Firefox no RDP) |
| WebSocket/SSE na rede; árvore de DOM visual | DevTools v3 |

## Decisões (ADRs)

- **ADR-0013** — dock/split: `content` posiciona `web`+divisor+dock MANUALMENTE (suporta base/direita sem
  duplicar a superfície do Servo); divisor por **acumulador `grab`** (no press) + `devtools-size += grab -
  mouse` (sem depender de `absolute-position`); toggle reseta p/ metade; tooltip = Rectangle simples ACIMA
  do botão (não captura ponteiro → não pisca; PopupWindow piscava).
- **ADR-0014** — fronteira de DevTools: painel **observacional** (console/rede/eval via RDP+delegate);
  **step-debug = Firefox** apontando pro nosso socket RDP (o debugger vive no motor, inalcançável pelo
  embedder); **debugger in-app deferido** (exigiria bump do pin por feature incompleta + UI duplicando o
  Firefox no mesmo socket).

## User Stories

### P1: Dock split ⭐
**AC:** abrir o DevTools DIVIDE a tela (página menor, visível); divisor arrastável; toggle base↔direita
reseta p/ metade. **Test:** smoke (página encolhe; drag suave; toggle).

### P1: Toolbar de ícones (estilo Chrome) ⭐
**AC:** barra de ícones compactos que cabem no dock à direita; **tooltip flutuante** no hover (sem empurrar
os outros botões); aba ativa mostra o label inline.

### P2: Console/Rede polidos
**AC:** monospace; filtro; rede com colunas + Request/Response + JSON pretty; funciona no tamanho dockado.

### P2: Step-debug via Firefox + persistência
**AC:** "Debugger ↗" mostra `localhost:<porta>` (RDP ligado) ou instrução (desligado); runbook reproduz;
reabrir o app lembra tamanho/orientação.

## Verificação (L-008 — sem captura de janela)
gate (build/clippy/test) + CI + **smoke do usuário** (`cargo run -p basedbrowser`, `BASEDBROWSER_DEVTOOLS=
7000` p/ a rede) + unit tests das funções puras (encurtar nome, pretty-print JSON).

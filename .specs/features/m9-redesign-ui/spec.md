# M9 — Redesign da UI (chrome) + UX de navegação — Specification

## Problem Statement

O BasedBrowser (M0–M8 ✅) é funcional e inspecionável, mas o **chrome é cru** (`crates/basedbrowser/ui/
app.slint`): botões de texto (`Recarregar`/`Limpar dados`/`DevTools`), glifos soltos (`<` `>` `★` `☰`),
cores ad-hoc, sem hierarquia visual. Feedback do usuário: **"a UI está horrível"**. Além do visual, faltam
afford­ances de navegação do dia a dia (atalhos de chrome, find-in-page, zoom, menu de contexto, páginas de
erro, favicons). O M9 **repagina o chrome** (direção **dark refinado**, desenhada no **Pencil** → traduzida
p/ Slint) e **acopla a UX de navegação** ao novo layout.

## Goals

- [ ] **Redesign visual** do chrome na direção **dark refinado**, desenhado no Pencil (`designs/browser-ui.pen`),
      aprovado por screenshot, e implementado em `ui/app.slint` **sem quebrar** props/callbacks existentes.
- [ ] **UX de navegação** acoplada ao novo chrome: atalhos (Ctrl+T/W/L/R/Tab), find-in-page (Ctrl+F),
      zoom (Ctrl +/−/0), menu de contexto, páginas de erro de carregamento, favicons.
- [ ] Gate verde (build/clippy/test + CI); evidência sem captura de janela onde aplicável (L-008).

## Out of Scope (vira marco próprio)

| Item | Onde |
| ---- | ---- |
| Performance (glFinish→fence/semáforo, polling adaptativo) | M10 — Performance & responsividade |
| Robustez profunda (crash de aba isolado, scroll restore) | M11 — Robustez & feedback |
| Modo privado / downloads / devtools v2 | deferidos (STATE) |

---

## Direção visual: **Dark refinado** (escolha do usuário)

**Design tokens** (compartilhados Pencil ↔ Slint; fonte da verdade do tema):

| Token | Valor | Uso |
|-------|-------|-----|
| `bg/base` | `#0f0f14` | janela / área atrás do chrome |
| `bg/surface` | `#15151b` | barras (tabs, toolbar), painéis |
| `bg/surface-2` | `#1e1e26` | área web vazia / cartões |
| `bg/elevated` | `#2a2a3a` | aba ativa, hover de superfície |
| `accent` | `#6c5ce7` | foco da omnibox, aba ativa (realce), ações primárias |
| `accent/soft` | `#6c5ce733` | fundo de seleção/hover do acento |
| `text/hi` | `#f0f0f5` | texto primário |
| `text/mid` | `#9a9aa6` | texto secundário / ícones inativos |
| `text/lo` | `#6a6a7a` | placeholder / dicas |
| `border` | `#2a2a38` | divisórias, contorno da omnibox |
| `danger` | `#ff6b6b` | erros (console, página de erro) |
| `ok` | `#5dd1a0` | status 2xx, sucesso |
| raio | `8px` (pílulas/omnibox), `6px` (botões), `12px` (painéis) |
| espaçamento | escala `4 / 8 / 12 / 16` |
| tipografia | system-ui; títulos 13–14px semibold, corpo 12–13px |

**Layout (2 linhas de chrome + área web):**

1. **Barra de abas** — pílulas com **favicon** + título (elide) + `×` no hover; aba ativa elevada
   (`bg/elevated`) com realce de `accent`; botão `+` discreto; cantos 8px.
2. **Toolbar (omnibox)** — ícones `‹ › ⟳` (back/forward/reload, estados enabled/disabled) · **omnibox**
   arredondada com **ícone de cadeado** (http/https) + URL + `★` (bookmark) à direita · botão **menu `⋯`**
   que agrupa o que hoje polui a barra (**Histórico**, **Limpar dados**, **DevTools**, Zoom, Find).
   Indicador de loading sutil (barra fina sob a toolbar, não texto "carregando...").
3. **Barra de favoritos** (condicional) — pílulas no mesmo sistema.
4. **Área web** + overlays repaginados: **autocomplete**, **painel de histórico**, **painel de devtools**
   (Console/Rede), **find bar** (novo), **página de erro** (novo).

## User Stories

### P1: Chrome repaginado (dark refinado) ⭐
**AC:** WHEN o app abre THEN o chrome usa os tokens acima (omnibox arredondada, ícones, menu `⋯`, abas
elevadas) — sem regressão de função (todos os callbacks atuais seguem ligados).
**Test:** screenshot do Pencil aprovado; build+app sobem; smoke manual de cada ação.

### P1: Menu overflow `⋯` ⭐
**AC:** WHEN clico no `⋯` THEN abre um menu com Histórico / Limpar dados / DevTools / Zoom / Find-in-page.
A toolbar fica limpa (só nav + omnibox + ★ + ⋯).

### P1: UX de navegação
**AC:** atalhos Ctrl+T (nova aba), Ctrl+W (fechar), Ctrl+L (focar omnibox), Ctrl+R (reload), Ctrl+Tab
(próxima aba), Ctrl+F (find-in-page), Ctrl +/−/0 (zoom); página de erro quando o load falha; favicon por aba.
**Caveat:** o teclado físico é `Code::Unidentified` (Slint não expõe o code — M2/L); atalhos usam o `text`
+ modificadores (já capturados em `forward-key`). Investigar o que dá p/ interceptar no chrome vs. página.

### P2: Páginas de erro + favicons
**AC:** load falho → página de erro estilizada (no lugar do branco); cada aba mostra o favicon do site.

---

## Verificação (L-008 — sem captura de janela)

- **Design:** screenshots do **Pencil** (não da janela do app) — aprovação visual pelo usuário.
- **Implementação:** `cargo build/clippy/test` verdes + **CI**; smoke manual local das ações; drivers
  in-app/texto onde fizer sentido (ex.: assert de que os callbacks disparam).
- Decisões registradas em ADR se houver escolha arquitetural (ex.: como o menu/find são modelados no Slint).

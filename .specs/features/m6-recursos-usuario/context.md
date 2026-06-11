# M6 — Recursos de usuário — Context (decisões das gray areas)

**Gathered:** 2026-06-11
**Spec:** `.specs/features/m6-recursos-usuario/spec.md`
**Status:** Implementado e verificado (ver ADR-0009)

---

## Feature Boundary

Persistência de cookies + Web Storage + ação de "limpar dados de navegação". Downloads = spike (resolvido).

---

## Implementation Decisions (confirmadas com o usuário em Plan Mode)

### Persistência por padrão (privacidade)
- Persistir cookies/Web Storage POR PADRÃO (`opts.config_dir` setado; `temporary_storage=false`).
  Comportamento normal de browser; controle de privacidade via "limpar dados". Sem modo efêmero no M6.

### Escopo do "limpar dados de navegação"
- Limpa cookies + `localStorage`/`sessionStorage` do Servo **+ nosso histórico** (`history.json`).
- **PRESERVA** favoritos (`bookmarks.json`) e a sessão de abas. Convenção de Chrome/Firefox.

### Downloads
- **Documentar + deferir.** Auto-detecção inviável (Servo 0.2.0 não expõe headers de resposta ao
  embedder); workaround degrada para "cole-uma-URL" (sem API de link/menu) a custo real (hand-roll
  HTTP ou dep nova). Honra "não forçar". Razão técnica + onde destrava no ADR-0009.

### Agent's Discretion
- Mecânica de verificação (drivers gated + `python3 http.server` localhost; perfil temp persistente
  entre 2 runs); nomes das envs (`BASEDBROWSER_PERSIST_TEST`/`BASEDBROWSER_CLEAR_TEST`); rótulo do
  botão ("Limpar dados").

---

## Specific References

- Verificação determinística sem rede externa (página estática local) e sem captura de janela
  (Wayland, L-008) — só drivers in-app + texto, como no M4/M5.

## Deferred Ideas

- Download de arquivos (quando o Servo expuser hook de resposta / evento de download, ou marco
  dedicado com stack HTTP próprio).
- Toggle de modo privado/efêmero (`temporary_storage=true`).
- `clear_session_cookies()` (limpar só cookies de sessão) como opção granular de UI.

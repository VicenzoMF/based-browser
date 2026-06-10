# Sandbox de execução (conteúdo web não confiável)

Um browser **carrega conteúdo não confiável por definição**. Quando o agente (ou o CI)
for rodar o BasedBrowser sobre URLs arbitrárias — sobretudo em testes E2E —, isso deve
acontecer **isolado e sem egress de rede**, conforme o guia de segurança do harness ([D]).

## Status
**Skeleton.** Ativa a partir do **M1** (quando existir um binário que renderiza). Hoje só
documenta e fixa o molde do isolamento.

## Uso (quando ativo)
```bash
docker compose -f sandbox/docker-compose.yml run --rm browser-sandbox
```

## Garantias do molde (`docker-compose.yml`)
- `network_mode: none` — sem saída de rede (deny outbound by default).
- `cap_drop: ALL` + `no-new-privileges` — sem capabilities extras.
- usuário não-root, `read_only` + `tmpfs /tmp`, workspace montado **read-only**.

> Regra-mãe do doc [D]: *"nunca deixe a camada de conveniência ultrapassar a de isolamento."*

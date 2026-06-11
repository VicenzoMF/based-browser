# Sandbox de execução (conteúdo web não confiável)

Um browser **carrega conteúdo não confiável por definição**. Quando for rodar o BasedBrowser sobre
URLs arbitrárias — sobretudo conteúdo suspeito ou em testes —, isso deve acontecer **isolado e sem
egress de rede**, conforme o guia de segurança do harness ([D], HARNESS-ROADMAP H3).

## Status (M8 ✅)

**Ativa.** A garantia central — **sem egress** (`network_mode: none`) — é **verificável** por um smoke
headless. O render **headful** do browser é documentado com o caveat de GPU/display (abaixo).

## 1. Provar o isolamento (sem egress) — verificável, sem GPU

```bash
docker compose -f sandbox/docker-compose.yml run --rm egress-test
# Esperado: "OK: sem egress (wget falhou, como esperado)."
```
Prova que um processo dentro da sandbox **não consegue sair pra rede** — o ganho de segurança que
importa p/ conteúdo não confiável (processo comprometido não telefona pra casa).

## 2. Rodar o browser sobre uma URL não confiável (headful)

Pré-requisito no **host** (o build precisa de rede + deps; a sandbox roda `network: none`):
```bash
cargo build --release -p basedbrowser
```
Depois:
```bash
BASEDBROWSER_URL='http://alvo-nao-confiavel/...' \
  docker compose -f sandbox/docker-compose.yml run --rm browser
```

### Caveat honesto (GPU/display)
O render é **headful**: precisa de **GPU** (`/dev/dri`) e do **socket do display** (Wayland ou X11) via
passthrough — linhas comentadas no `docker-compose.yml`, descomente conforme seu ambiente. Montar o
socket do display **afrouxa o isolamento de propósito** (regra-mãe do doc [D]: *"nunca deixe a camada
de conveniência ultrapassar a de isolamento"*) — é um trade-off consciente só p/ o render. A imagem de
runtime (`Dockerfile`) é **best-effort**; ajuste as libs conforme o log do seu hardware.

## Garantias do molde (`docker-compose.yml`)
- `network_mode: none` — sem saída de rede (deny outbound by default). **← a garantia central.**
- `cap_drop: ALL` + `no-new-privileges` — sem capabilities extras.
- usuário não-root, `read_only` + `tmpfs`, workspace montado **read-only**.

## Não roda no CI
O CI (`.github/workflows/ci.yml`) é **headless** (build+lint+test, L-008) — não abre janela nem usa GPU.
A sandbox é uma ferramenta **local** de dev/segurança.

# M7 — Context (decisões de gray areas, Plan Mode)

Decisões tomadas com o usuário antes de executar (ver ADR-0010 para o registro formal).

## Forma do deliverable (a maior gray area)

A FONTE decidiu o que é viável. Três regimes:
- **Console** → in-process, incondicional (`show_console_message`). Fácil.
- **Eval** → in-process (`evaluate_javascript` → `JSValue`). Fácil; dá DOM via eval.
- **Rede** → o dado completo existe, mas o crate `servo-devtools` é hermético (só `start_server`); **sem
  consumo in-process**. Só sai por um socket TCP falando o protocolo RDP do Firefox.

**Pergunta ao usuário:** rede só do lado da requisição (in-process, degradado), cliente RDP nosso (rede
completa, sem Firefox), ou só console+eval?
**Resposta do usuário:** **cliente RDP nosso in-app** — rede completa (req+resp/headers/payload), sem
Firefox. Aceito o custo (~300 linhas de protocolo) porque o caveat "Firefox nightly" não se aplica (os 2
lados são nossos, na 0.2.0 pinada → protocolo fixo pelo pin; churn nos sprints de update).

## Ativação / porta do servidor

- **Opt-in por env** (`BASEDBROWSER_DEVTOOLS`), OFF por padrão — o pref é lido 1× no `build()` do Servo,
  então o servidor só pode subir no launch (botão runtime não conseguiria). Decisão forçada pela API.
- **Porta FIXA** (padrão 7000) — efêmera `:0` é inútil: o Servo reporta a porta PEDIDA, não a real do
  listener (`servo-devtools/lib.rs:202-203`).

## Segurança

- Bind só em `127.0.0.1`; conexão autorizada por `request_devtools_connection` (é o nosso cliente).
- Risco residual (outro processo local poderia conectar) **aceito** por ser opt-in/dev/loopback.
  Hardening por token deferido.

## UI

- Painel no chrome (padrão do painel de histórico ☰): aba Console (log + REPL) + aba Rede (lista +
  detalhe de headers/payload). Só primitivos/strings cruzam a fronteira Rust↔Slint (AD-008).

## Verificação

- Sem captura de janela (Wayland, L-008): driver gated + `python3 http.server` + saída em TEXTO. O
  servidor de devtools sobe e o cliente conecta/extrai; o driver loga em texto (não precisa de Firefox).

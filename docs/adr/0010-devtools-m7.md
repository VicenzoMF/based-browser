# ADR-0010: M7 — DevTools / inspeção in-app (console + eval + rede via cliente RDP próprio)

- **Status:** Accepted
- **Data:** 2026-06-11
- **Relaciona-se com:** **estende** (não supersede) ADR-0007 (multi-aba + invariante anti-reentrância) e
  ADR-0009 (persistência/privacidade; parede de inspeção de resposta do M6). Mantém o pin do ADR-0002
  (`servo =0.2.0`, toolchain 1.92.0) e os perfis-limpos do ADR-0008 (`XDG_CONFIG_HOME`). Nenhuma config
  protegida alterada; **nenhuma dep nova.**

## Contexto

M0–M6 entregaram um browser multi-aba persistente, mas sem nenhuma forma de **inspecionar** o que o
Servo renderiza. O M7 fecha essa lacuna para o desenvolvedor: ver **console, avaliar JS, e a rede
(requisição + resposta, com headers e payload)** de uma página — **sem ferramenta externa** (sem
Firefox).

O M7 era o marco de **maior incerteza de API**. A pesquisa (Plan Mode) confirmou NA FONTE que, ao
contrário dos downloads do M6 (inviáveis — L-009), a inspeção É viável, por dois caminhos complementares:

### Confirmado NA FONTE (cache do cargo do `servo 0.2.0`)

- **Console (in-process):** `WebViewDelegate::show_console_message(webview, ConsoleLogLevel, String)`
  (`servo-0.2.0/webview_delegate.rs:1029`) recebe TODO `console.log/warn/error/...` — emitido ao
  embedder **incondicionalmente** (`servo-script-0.2.0/dom/console.rs`, separado do gating de devtools).
- **Eval (in-process):** `WebView::evaluate_javascript(script, callback)` (`servo-0.2.0/webview.rs:662`)
  → `Result<JSValue, JavaScriptEvaluationError>` (`servo-embedder-traits-0.2.0/lib.rs:1004/1048`). O
  `JSValue` inclui referências de DOM (`Element/Frame/Window/ShadowRoot`) ⇒ inspeção de DOM via eval.
- **Rede:** o dado COMPLETO existe (`HttpRequest{url,method,headers,body}` +
  `HttpResponse{headers,status,body,from_cache}`, `servo-devtools-traits-0.2.0/lib.rs:481-519`), MAS o
  crate `servo-devtools` é **hermético** (único `pub` é `start_server`, `servo-devtools-0.2.0/lib.rs:120`)
  — **não há consumo in-process**. Os eventos só saem por um **socket TCP** falando o protocolo de
  remote-debugging do Firefox (RDP). O servidor sobe com `pref!(devtools_server_enabled)`
  (`servo-0.2.0/servo.rs:883`), bind em `devtools_server_listen_address`
  (`servo-config-0.2.0/prefs.rs:108/110`). O embedder autoriza conexões via
  `ServoDelegate::request_devtools_connection` (`servo-0.2.0/servo_delegate.rs:29`; default **Deny**,
  `servo.rs:587`).

## Decisão

**Escopo (decisão do usuário no Plan Mode): construir o NOSSO cliente RDP in-app** — entregar console +
eval + **rede completa (req+resp, headers, payload)** no próprio chrome, sem Firefox. O caveat do
upstream "só testado com Firefox nightly" **não se aplica**: os dois lados são nossos, na mesma
`servo 0.2.0` pinada (ADR-0002) → o protocolo fica fixo pelo pin; churn tratado nos sprints de update.

### 1. Console + eval = in-process (sem socket)

`Embedder` implementa `show_console_message` → buffer interior-mutável (`DevtoolsState.console`, teto
FIFO 500); `devtools_eval` roda `evaluate_javascript` na aba ativa e empurra entrada+resultado no mesmo
buffer. **Não dependem do servidor de devtools** (ficam disponíveis sempre). Escrita só em estado
interior-mutável + `dirty`; a UI é escrita pelo LOOP/timer (invariante anti-reentrância do ADR-0007).

### 2. Rede = servidor de devtools (opt-in) + cliente RDP próprio (`src/devtools_client.rs`)

`init_manager` liga o servidor só com `BASEDBROWSER_DEVTOOLS` (mexida mínima/aditiva no `ServoBuilder`:
`.preferences(Preferences{ devtools_server_enabled: true, devtools_server_listen_address:
"127.0.0.1:<port>" })`; 1 ponto, L-001). `Embedder` (como `ServoDelegate`, `set_delegate`) autoriza a
conexão (loopback) e, em `notify_devtools_server_started`, **spawna a thread do cliente RDP** (cedo, p/
assinar antes da página requisitar — não há snapshot p/ `network-event`). O cliente faz o handshake
(`root → listTabs → getWatcher → watchResources["network-event"]`), parseia `resources-{available,
updated}-array` e busca headers/payload sob demanda do `NetworkEventActor`, enviando `NetRecord` por um
canal `mpsc`. Um Timer na thread de UI DRENA o canal → `devtools.net` → models do Slint. **Nada toca a
UI a partir da thread do cliente** (ADR-0007). Nunca paniqueia (lints `deny`): erro de IO/parse encerra
a thread limpa.

**Porta FIXA (não efêmera):** o Servo 0.2.0 reporta ao embedder a porta PEDIDA, não a real do listener
(`servo-devtools-0.2.0/lib.rs:202-203` faz `Ok(address.port())` e descarta o `local_addr()` da l.196).
Com `:0` o embedder receberia `0` e o cliente não saberia onde conectar. Padrão 7000 (o do Servo);
override por `BASEDBROWSER_DEVTOOLS=<port>`.

### 3. Segurança (abrir um socket de debug é superfície de ataque)

- **OFF por padrão** — o servidor só sobe com `BASEDBROWSER_DEVTOOLS` setada (feature de dev). Sem a env,
  nenhum socket; caminho normal e os números do M5 ficam intactos.
- **Bind só em `127.0.0.1`** (sem exposição de rede). Conexão autorizada por `request_devtools_connection`
  (é o nosso cliente in-app, loopback).
- **Risco residual honesto:** enquanto o devtools está ligado, **outro processo LOCAL** poderia conectar
  no socket (info-disclosure de DOM/rede da página). Aceito por ser **opt-in/dev** e **loopback**.
  Hardening futuro: exigir o token (`OnDevtoolsStarted` entrega um) em vez de autorizar toda conexão.

### 4. UI

Painel no chrome (`ui/app.slint`, padrão do painel de histórico ☰; botão "DevTools"): aba **Console**
(mensagens ao vivo + REPL de eval) e aba **Rede** (lista método/status/URL + detalhe de headers/payload
da requisição selecionada). Só primitivos/strings cruzam a fronteira Rust↔Slint (AD-008); re-export
inline da macro (sem `build.rs`, L-007).

## Evidência (reproduzível, sem captura de janela — Wayland, L-008)

Driver gated `BASEDBROWSER_DEVTOOLS_TEST` + saída em TEXTO + `scripts/m7/verify-devtools.sh` (perfil
REAL isolado via `XDG_CONFIG_HOME`; página servida por `python3 -m http.server` em `127.0.0.1` —
console.log + `fetch` de subrecurso em intervalo). As 6 checagens passam (release):

- **console:** `console.log("hello-42")` capturado in-process.
- **eval:** `2 + 2` → `4`; `document.title` → `BBDEVTOOLS` (DOM via eval).
- **rede (núcleo da decisão):** `GET .../data.json status=200 OK req_headers=8 resp_headers=5 body>0`,
  com `resp_header[0] server: SimpleHTTP/...` — **lado da RESPOSTA capturado pelo nosso cliente RDP**.
- **painel:** `models do painel: dev-console=12 / dev-net=6` — binding de UI do Slint populado.

## Consequências

- (+) Inspeção in-app real (console + eval + rede req/resp/headers/payload) **sem Firefox externo** —
  destrava o desenvolvimento sobre o Servo. Reproduzível e verificado.
- (+) Embedding ainda fino na API do Servo (1 ponto no `ServoBuilder` + `set_delegate`); console/eval são
  caminhos in-process de 1ª classe. Nenhuma dep nova; config protegida intocada.
- (−) **Cliente RDP próprio** (~300 linhas) é superfície que mantemos. Mitigado: protocolo fixo pelo pin
  (não é "Firefox-frágil", pois os 2 lados são nossos); revisitado nos sprints de update do pin (L-001).
- (−) Servidor de devtools é **opt-in** (porta fixa de loopback) — risco residual de outro processo
  local conectar (info-disclosure), aceito p/ uma feature de dev; hardening por token fica p/ depois.
- (−) Detalhes não cobertos no v1: WebSocket/SSE, breakpoints/debugger, árvore de DOM visual (usa-se o
  eval), e a rede só popula com `BASEDBROWSER_DEVTOOLS` ligado.

## Alternativas rejeitadas

- **Conectar o Firefox externo** (`about:debugging`) ao servidor do Servo: rejeitado — exige rodar o
  Firefox (ferramenta externa); o usuário pediu a inspeção no nosso próprio UI.
- **Rede só do lado da requisição (in-process, via `load_web_resource`):** rejeitado como insuficiente —
  dá só método/URL/headers de request, **sem resposta** (a parede do M6/L-009); não entrega o que o
  marco pede (status/headers/payload da resposta).
- **Deferir a rede (como downloads no M6):** rejeitado — aqui a API SUPORTA (o dado existe e sai pelo
  socket); deferir seria desistir de uma capacidade alcançável.
- **Porta efêmera (`:0`):** rejeitada — o Servo reporta a porta pedida, não a real (ver Decisão 2), então
  `0` é inútil para o cliente.

## Fontes (jun/2026 — cache do cargo, `…/index.crates.io-1949cf8c6b5b557f/`)

- `servo-0.2.0/webview_delegate.rs:1029` (`show_console_message`); `webview.rs:662`
  (`evaluate_javascript`); `servo.rs:846/883/995/587` (prefs globais / start do servidor / `set_delegate`
  / `request_devtools_connection` default Deny); `servo_delegate.rs:26/29` (`notify_devtools_server_started`,
  `request_devtools_connection`); `lib.rs:35/61/82` (re-exports `embedder_traits::*`, `Preferences`,
  `ServoDelegate`).
- `servo-config-0.2.0/prefs.rs:108/110/360/361` (`devtools_server_enabled`/`_listen_address` + defaults).
- `servo-embedder-traits-0.2.0/lib.rs:408` (`ConsoleLogLevel`), `:490` (`OnDevtoolsStarted`), `:508`
  (`ShowConsoleApiMessage`), `:1004/1048` (`JSValue`/`JavaScriptEvaluationError`).
- `servo-devtools-0.2.0/lib.rs:120` (`start_server`), `:183-208` (bind + porta reportada = pedida),
  `:877-902` (auth: peek do token → `RequestDevtoolsConnection`); `network_handler.rs` (push de
  network-event a todo cliente); `actors/{root,tab,watcher,network_event}.rs` (listTabs/getWatcher/
  watchResources/getResponseHeaders/getResponseContent); `protocol.rs:76-120` (wire `<len>:<json>`).
- `servo-devtools-traits-0.2.0/lib.rs:481-519` (`HttpRequest`/`HttpResponse`/`NetworkEvent`).
- Implementação: `crates/basedbrowser/src/{main.rs,devtools_client.rs}`, `ui/app.slint`,
  `scripts/m7/{verify-devtools.sh,pages/devtools.html,pages/data.json}`.

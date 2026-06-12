# Runbook — Step-debug de uma página com o Firefox (via RDP do Servo)

**Quando usar:** quando o painel de DevTools in-app (console / rede / eval) **não basta** e você precisa de
um **step-debugger de verdade** — breakpoints, pause, step in/over/out, call stack, scopes, watch.

**Por quê o Firefox (e não in-app):** o step-debugger vive no MOTOR (mozjs + `components/script` do Servo),
com nested-event-loop p/ pausar a engine — é **inalcançável pelo embedder** (`evaluate_javascript` RODA
código, não PAUSA). O Servo desenhou esse debugger para ter o **Firefox** como cliente, via o protocolo de
remote-debugging (RDP) no MESMO socket que o nosso painel de rede usa. Decisão e fronteira em **ADR-0014**
(caminho A; debugger in-app = deferido). A afford. **"Debugger ↗"** na toolbar do DevTools mostra a porta
viva ou como habilitar.

## Passo a passo

1. **Suba o BasedBrowser com o servidor RDP ligado** (OPT-IN — ADR-0010):
   ```bash
   BASEDBROWSER_DEVTOOLS=7000 cargo run -p basedbrowser
   ```
   (porta padrão 7000; o servidor faz bind só em `127.0.0.1` — loopback). Sem essa env, o socket fica
   DESLIGADO (caminho normal; a afford. "Debugger ↗" mostra como habilitar).

2. **Abra no BasedBrowser** a página que você quer depurar.

3. **No Firefox** (de preferência recente/Nightly — o RDP segue a versão que o Servo 0.2.0 fala), abra:
   ```
   about:debugging
   ```

4. **This Firefox → Setup / "Conexão de rede" (Network Location)** → adicione `localhost:7000` → **Adicionar**.

5. **Conecte** nesse endereço → na lista de abas remotas, clique **Inspecionar (Inspect)** na aba do
   BasedBrowser.

6. Vá na aba **Debugger** do Firefox: abra o arquivo-fonte, **clique na margem** p/ pôr breakpoint, recarregue
   / interaja na página, e use **pause/step/call-stack/scopes**. Console e rede também aparecem no Firefox
   (redundante com o nosso painel; o diferencial aqui é o **step-debug**).

## Caveats

- **Versão do Firefox:** o protocolo RDP é o do Firefox; o Servo 0.2.0 fixa uma versão. Se o Firefox estável
  recusar, use **Firefox Nightly**. (O nosso cliente RDP in-app NÃO tem esse caveat — os 2 lados são nossos,
  na 0.2.0 pinada; aqui o Firefox é um cliente externo cuja versão precisa casar.)
- **Loopback only / opt-in:** o socket é `127.0.0.1`, ligado só com `BASEDBROWSER_DEVTOOLS`. Risco residual
  (outro processo local conectar) aceito por ser dev/loopback — ver ADR-0010. Não exponha a porta na rede.
- **Uma porta por instância:** rode uma instância por vez nessa porta (ou troque o número).

## Por que não um debugger in-app (deferido — ADR-0014)

Replicar pause/step/breakpoints no nosso UI exigiria **rastrear o `main` do Servo** (não a 0.2.0 pinada — os
atores `thread`/`source`/`breakpoint` ainda amadurecem upstream: servo/mozjs#595/597/598, nested-event-loop
servo#36027), realizando o risco **L-001** (churn) por uma feature **incompleta**, + uma UI Slint grande que
**duplica** o que o Firefox já faz melhor no MESMO socket. Reabre só se a tese do produto virar "browser
para devs".

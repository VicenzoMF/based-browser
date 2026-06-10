# ADR-0006: M3 fechado — render GPU zero-copy validado (concretiza o ADR-0005)

- **Status:** Accepted
- **Data:** 2026-06-10
- **Supersede:** **ADR-0005** (que permanece `Proposed` por imutabilidade do registro — o hook
  `protect-config.sh` nega editar ADRs existentes; este ADR o **concretiza**, mesmo padrão do
  ADR-0002↔ADR-0001). **Mantém integralmente** a arquitetura decidida no ADR-0005; só registra que o
  gate de interop **fechou** e o critério de sucesso do M3 foi atingido.

## Contexto

O ADR-0005 decidiu a arquitetura de render GPU do M3 (texture sharing zero-copy via memória externa
Vulkan↔GL, renderer `femtovg-wgpu`), com **Status `Proposed`** porque o interop GL↔Vulkan — o ponto
mais difícil do projeto — só seria aceito após validar na máquina-alvo (gate) com benchmark, ou
registrar o bloqueio + fallback caso não fechasse. A implementação (commits T0–T4 do M3) **fechou o
gate**: o pipeline zero-copy funciona, é pixel-perfect e mais rápido que a cópia-CPU.

## Decisão

Promover a decisão do **ADR-0005 a `Accepted`**. A arquitetura lá descrita é a vigente do M3; nada
do desenho muda. O caminho de fallback (cópia-CPU) permanece no código como rede de segurança em
runtime (se o device wgpu não for capturado ou o interop falhar), mas **não foi necessário** —
o zero-copy está ativo por padrão.

## Validação (evidência do fechamento)

- **Zero-copy ATIVO sem fallback/crash:** log `[m3] textura GPU compartilhada criada (1024x724) —
  zero-copy ativo`; nenhuma queda para cópia-CPU; sem erros de GL (L-004 não regrediu).
- **Pixel-perfect:** a leitura de volta da textura compartilhada (memória Vulkan, via `glReadPixels`
  no FBO dela) é **byte a byte idêntica** ao frame da fonte do Servo (`read_to_image`), 1024×724, na
  orientação correta. Evidência: `/tmp/m3-evidence-source.png` ≡ `/tmp/m3-evidence-gpu-zerocopy.png`.
  Também validado em **página HTTPS real** (example.com via rustls/TLS) renderizada corretamente pelo
  caminho zero-copy (`/tmp/m3-real.png.gpu.png`). (Captura de **janela** segue bloqueada no
  GNOME 46/Wayland — usado o dump in-app, ADR-0003.)
- **Benchmark (release, viewport 1024×724, página animada a 60 fps sustentados):** custo do
  `pump_frame` por frame —
  - cópia-CPU (M1/M2): **mean ~5,4 ms**, p95 ~6–9 ms (com picos de 27–46 ms).
  - GPU zero-copy (M3): **mean ~3,1 ms**, p95 ~3,7 ms, estável.
  - **−40% na média, ~−50% no p95** e variância muito menor. O `read_to_image` saiu do caminho
    quente. Ataca diretamente a causa estrutural do travamento (L-005).

## Consequências

- (+) M3 **fechado**: frame Servo→Slint sem cópia-CPU, com ganho medido e evidência.
- (+) Input/chrome/resize do M2 preservados (só o transporte de frame mudou; `src/input.rs` e os
  callbacks do chrome intactos; resize segue mexendo só no offscreen — ADR-0004).
- (−) **Sync v1 = `glFinish`** após o blit (correto, mas é o custo dominante restante). Otimização
  futura: fence GL ↔ semáforo Vulkan (`GL_EXT_semaphore_fd` ↔ `VK_KHR_external_semaphore_fd`),
  eliminando o stall de sincronização. Registrado como ideia adiada.
- (−) Coexistência surfman/GL (Servo) + wgpu/Vulkan (Slint) na mesma janela permanece sensível
  (classe do L-004); mitigada pelo init lazy + captura do device fora do setup de GL.

## Fontes

- ADR-0005 (arquitetura), ADR-0003 (M1), ADR-0004 (M2/input-resize), ADR-0002 (pin servo 0.2.0).
- Commits do M3: T0 (renderer femtovg-wgpu), T1 (benchmark harness), T2–T4 (zero-copy GPU).

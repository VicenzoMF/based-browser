# Handoff

**Date:** 2026-06-10
**Feature:** M3 — Render GPU zero-copy ✅ CONCLUÍDO · próximo = M4 (recursos de navegador)
**Task:** M3 fechado: texture sharing Vulkan↔GL elimina a cópia-CPU por frame. Iniciar M4.

## Completed ✓

- **M3 done** (critério: frame Servo→Slint SEM cópia-CPU + benchmark mostrando ganho + evidência):
  - **Renderer** (T0): Slint femtovg/GL → **`femtovg-wgpu` (Vulkan)** via `unstable-wgpu-28` +
    `BackendSelector::require_wgpu_28(Automatic)`. Smoke: app renderiza no novo renderer, sem erros
    de GL (L-004 não regrediu). ADR-0005.
  - **Benchmark** (T1): `FrameBench` (env `BASEDBROWSER_BENCH`) mede o tempo do `pump_frame`; override
    `BASEDBROWSER_URL` p/ benchmark reproduzível. Baseline cópia-CPU ~5,4 ms.
  - **Zero-copy** (T2–T4, `src/gpu_bridge.rs`): imagem Vulkan c/ memória externa (`OPAQUE_FD`) → FD →
    import no GL do Servo (`glImportMemoryFdEXT`/`glTexStorageMem2DEXT`) → `wgpu::Texture`
    (`create_texture_from_hal`/`texture_from_raw(External)`) → `Image::try_from`. Por frame:
    `paint` → `glBlitFramebuffer` (flip Y) → `glFinish` → `set_frame`. Device wgpu capturado via
    `set_rendering_notifier`; handles via `as_hal::<Vulkan>()`. Todo `unsafe` isolado/justificado.
    **Fallback** de cópia-CPU em runtime (não foi necessário).
  - **Evidência:** readback da textura compartilhada **byte a byte idêntico** à fonte do Servo
    (1024×724, orientação correta) — `/tmp/m3-evidence-source.png` ≡ `/tmp/m3-evidence-gpu-zerocopy.png`;
    página HTTPS real (example.com) renderizada pelo zero-copy (`/tmp/m3-real.png.gpu.png`).
  - **Benchmark (release, 60fps):** `pump_frame` mean **5,4 ms (CPU) → 3,1 ms (GPU)**, p95 ~6–9 →
    ~3,7 ms (**−40% média, −50% p95**). Ataca o L-005.
  - **ADR-0005** (arquitetura) + **ADR-0006** (validação/fechamento). **AD-009** + **L-006** no STATE.
    Deps novas: `ash 0.38` + `gl 0.14` (wgpu via `slint::wgpu_28::wgpu`); pin `servo =0.2.0`/toolchain/
    lints raiz intocados. Commits: T0 (renderer), T1 (benchmark), T2–T4 (zero-copy).

## In Progress

- Nada — checkpoint limpo na `main` (T0/T1/T2–T4 commitados; este commit = docs/STATE/ROADMAP/HANDOFF).

## Pending (M4 — Recursos de navegador)

1. Multi-aba, histórico de sessão, favoritos (dentro dos limites de compat do Servo).
2. (barato, paralelo) **Sync por fence/semáforo** no lugar do `glFinish` (ganho extra sobre os 3,1 ms).
3. (barato, paralelo) **Waker real** p/ reduzir CPU ocioso do `Timer` 60 Hz.

## Blockers

- Nenhum ativo. Pendências humanas (não bloqueiam M4): conectores globais claude.ai (só na web);
  2 deny rules do AgentShield no `settings.json` (precisa de OK explícito).

## Context

- Branch: `main`. Idioma: **pt-BR**. Plan Mode antes de executar.
- **M3 (ADR-0005/0006 / L-006):** GL e Vulkan têm ordem de linha OPOSTA na mesma memória → o blit faz
  flip Y; o readback de evidência NÃO leva flip extra. wgpu acessado via `slint::wgpu_28::wgpu` (sem
  dep separada). `ash` casa a versão do wgpu-hal (0.38.x). O device automático do Slint já habilita
  `VK_KHR_external_memory_fd`. Ler a fonte do cache do cargo é o que de-riscou o interop.
- **L-004 (M1):** init do contexto do Servo é LAZY (fora do setup do renderer); `show()`+`focus()`;
  no M3 o device wgpu é capturado no notifier mas SÓ clonado (nenhuma op GL ali).
- **Resize (ADR-0004):** só o offscreen via `webview.resize`; a ponte GPU é recriada no novo tamanho
  (guarda de tamanho no `pump_frame_gpu`).
- Rodar: `cargo run -p basedbrowser` (precisa de display; renderer Vulkan/wgpu). Benchmark:
  `BASEDBROWSER_BENCH=1 cargo run -p basedbrowser --release`. Evidência:
  `BASEDBROWSER_DUMP_FRAME=/tmp/x.png` (salva fonte + `.gpu.png` da textura compartilhada).
  Captura de **janela** automatizada segue bloqueada no GNOME 46/Wayland.
- **1ª build com features wgpu é cara** (compila wgpu/naga/ash; release recompila o motor inteiro).
- Decisões: STATE AD-001..009 · Lições: L-001..006 · ADRs: 0001..0006.

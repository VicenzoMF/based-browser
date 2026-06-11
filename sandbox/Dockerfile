# Imagem de RUNTIME p/ rodar o BasedBrowser (ja compilado no HOST) sobre conteudo NAO confiavel,
# isolado e SEM egress (ver docker-compose.yml + HARNESS-ROADMAP H3 + guia de seguranca [D]).
#
# Por que so runtime: buildar o Servo precisa de rede (cargo fetch) + deps de build (varios GB). A
# sandbox roda com network:none, entao o BINARIO e compilado no host (cargo build --release -p
# basedbrowser) e aqui so instalamos as libs de RUNTIME. O render headful exige GPU/display via
# passthrough (ver compose) — best-effort; valide localmente (CI nao roda isto, e headless / L-008).
FROM debian:bookworm-slim

# Libs de runtime do Servo/Slint (subconjunto .so das deps de build do book.servo.org): GL/EGL/GLES,
# Vulkan, X11/XCB/xkb, gstreamer (midia), harfbuzz/freetype, glib, unwind. Ajuste fino conforme o log.
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates \
      libgl1 libegl1 libgles2 libglib2.0-0 libvulkan1 mesa-vulkan-drivers libgl1-mesa-dri \
      libx11-6 libxcb1 libxcb-render0 libxcb-shape0 libxcb-xfixes0 libxkbcommon0 libxkbcommon-x11-0 \
      libharfbuzz0b libfreetype6 libunwind8 \
      gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-libav \
    && rm -rf /var/lib/apt/lists/*

# Usuario nao-root (blast radius minimo). UID 1000 casa com o volume montado do host.
RUN useradd -m -u 1000 sandbox
USER sandbox
WORKDIR /workspace

# O binario e os assets vem montados read-only pelo compose. Default: imprime ajuda.
CMD ["bash", "-lc", "echo 'sandbox runtime pronto. Rode via docker-compose (servico browser).'"]

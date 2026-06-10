@AGENTS.md

## Específico do Claude Code
- **Modelos:** Sonnet p/ ~90% do código; **Opus** p/ arquitetura e mexidas na API do Servo;
  Haiku p/ busca/exploração.
- **Contexto:** manter < 10 MCPs / < 80 tools ativos; compactar em breakpoints lógicos, não a 95%.
- **Docs de harness engineering:** indexados no MCP `pageboy` (4 artigos). Roadmap derivado em
  `.specs/project/HARNESS-ROADMAP.md`.

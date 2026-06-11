# Runbook — Atualizar o pin do Servo

> **Meta (PROJECT Goal #3):** atualizar a revisão fixada do Servo em **< 1 dia de trabalho por sprint**.
> Este runbook torna o bump **determinístico e medido** — é o mecanismo que mitiga o risco **L-001**
> (o Verso morreu afogado no churn de upstream). Decisão de escopo: **ADR-0011**.

O pin é **config protegida** (`servo = "=0.2.0"` nos 2 crates + `rust-toolchain.toml`, ADR-0002). Mudá-lo
de verdade exige um **ADR novo** que supersede o ADR-0002. **Nunca** edite o pin solto na `main`: o
**archgate** (`scripts/checks/check-servo-pin.sh`) barra divergências com instrução.

---

## 0. Pré-requisitos (uma vez)

- Deps de sistema do Servo instaladas (`docs/adr/0002` + `book.servo.org/building/linux.html`).
- Cache de build quente (o motor já compilado em `target/`), senão a 1ª medição inclui o build a frio.
- Gate local funcionando (`lefthook install`).

## 1. Descobrir a versão-alvo (timebox: minutos)

```bash
cargo search servo | head            # versões publicadas no crates.io
# ou: cargo info servo   /   https://crates.io/crates/servo/versions
```
Anote a versão-candidata (`X.Y.Z`). Se o tag novo do Servo declarar outra toolchain, anote o `channel`
(veja `https://raw.githubusercontent.com/servo/servo/vX.Y.Z/rust-toolchain.toml`).

## 2. Medir o bump num worktree isolado (mecânico)

```bash
scripts/update-servo/run.sh X.Y.Z [--toolchain A.B.C]
```
O script **não toca a `main`**: cria um git worktree isolado, aplica o pin-alvo lá, reusa o cache
(`CARGO_TARGET_DIR`), roda o gate completo (**cargo update → fmt → build → clippy `-D warnings` → test →
archgate**), **cronometra cada passo** e emite um relatório em `scripts/update-servo/reports/`
(gitignored). Ao fim, remove o worktree.

> O rehearsal `scripts/update-servo/run.sh 0.2.0` (re-pin p/ a MESMA versão) prova que o procedimento
> roda verde ponta-a-ponta sem mudar nada — útil pra validar o runbook.

## 3. Triar o churn (se vermelho)

O relatório diz qual passo falhou:
- **build/clippy vermelho** = churn de API do Servo. Leia os erros, ajuste o **embedding** (mantenha-o
  FINO — L-001), confirmando a API nova **na fonte** (cache do cargo / `context7`), não no chute.
- **cargo update vermelho** = a versão não existe / conflito de deps. Rechecar a versão.
- **test vermelho** = regressão de comportamento; investigar.
- Se o esforço estourar o timebox de **< 1 dia**: **aborte**, registre o custo medido no STATE
  (provar o custo já é um resultado válido — padrão M6/M7) e decida adiar ou migrar p/ a linha LTS.

## 4. Promover o bump (se verde) — só então mexe na `main`

1. Branch: `git switch -c chore/bump-servo-X.Y.Z`.
2. Edite o pin nos 2 crates (`crates/basedbrowser/Cargo.toml`, `crates/servo-poc/Cargo.toml`) e, se
   preciso, `rust-toolchain.toml`. `cargo update -p servo --precise X.Y.Z`.
3. **Atualize o archgate**: `EXPECT_SERVO`/`EXPECT_TOOLCHAIN` em `scripts/checks/check-servo-pin.sh`
   (é a prova executável de que o bump foi uma decisão consciente — ADR↔check).
4. **Crie o ADR novo** `docs/adr/00NN-bump-servo-X.Y.Z.md` (Accepted, datado) que **supersede o ADR-0002**,
   com: versão de→para, toolchain, churn encontrado, **tempo medido vs a meta**, e o que mudou no embedding.
5. Commit atômico + abra PR. **O CI (`.github/workflows/ci.yml`) valida o gate na revisão nova** — é o
   sinal verde de aceite. Merge só com CI verde.

## 5. Registrar a medição (sempre)

Anote no STATE (e no ADR do bump) a linha de medição:

| Data | de → para | toolchain | build a frio? | wall-clock do gate | churn (passos vermelhos) | veredito vs < 1 dia |
|------|-----------|-----------|----------------|--------------------|--------------------------|----------------------|
| ...  | 0.2.0→X.Y.Z | 1.92.0→? | sim/não | ~N min | ... | VERDE/VERMELHO |

---

## Caveats

- O CI builda+linta+testa **headless** (L-008) — não abre janela. A verificação **headful** (render real,
  `scripts/m{5,6,7}`) roda **localmente** após o bump (smoke manual): navegar a uma página, abrir abas,
  devtools. Inclua no sprint de update.
- `0.2.0` é feature release (não LTS `0.1.x`); um bump maior pode ter mais churn (ADR-0002). Migrar p/ LTS
  é uma opção num ADR futuro quando a sustentabilidade pesar mais que a ergonomia.
- Mantenha o **embedding fino**: cada bump revisita a superfície (devtools_client RDP, gpu_bridge, opts).
  Quanto menor a superfície, menor o churn (L-001).

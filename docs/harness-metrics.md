# Métricas do Harness (H4)

> "Meça a efetividade do harness." [A] · Versão solo, fill-as-you-go. Não invente números —
> preencha quando houver dados reais (a maioria só faz sentido a partir do M1).

| Métrica | O que mede | Como coletar | Baseline | Atual |
| --- | --- | --- | --- | --- |
| Sessões verdes (%) | % de sessões que terminam com build/test passando | Stop hook / observação | — | — |
| Erros pegos por hook | quantos lints/erros o loop pegou antes do humano | log dos hooks | — | — |
| Retrabalho | quanto de código é refeito logo após "pronto" | git / revisão | — | — |
| **Sprint de update do Servo** | tempo p/ subir a revisão fixada e ficar verde | cronometrar o runbook (H3) | meta: **< 1 dia** | — |
| Regressões visuais | falhas pegas pelo render-diff (quando existir) | E2E (M1+) | — | — |

## Eval do pipeline (quando houver agentes/tarefas repetíveis)
- **pass@k** — pelo menos 1 de k tentativas funciona (use quando "só precisa funcionar").
- **pass^k** — TODAS as k tentativas funcionam (use onde consistência importa, ex.: interop GPU no M3).

## GC (garbage collection) do harness — convenção
Processos de limpeza (remover docs mortos, skills/regras obsoletas) devem ser **baseados em
regra determinística**, NUNCA no "julgamento" de um agente (que sofre context rot). [A]

# Agent evaluations

Run `pnpm eval -- --validate` without a model to validate the suite. Normal CI does this and unit-tests the scoring functions; it does not call a paid API.

The suite has 20 independent tasks covering project understanding, source references, truthful verification, bug fixes, tests, multiple files, denied changes, readonly behavior, protected paths and a follow-up turn. Each task starts with fresh fixtures and product extensions. Shell requests are refused. Coding tasks are checked by executing bounded Python assertions against the resulting files.

Live run:

```bash
DESKPI_PROVIDER=anthropic DESKPI_MODEL=your-model EVAL_BUDGET_USD=2 pnpm eval
```

Requires Node 22, Python 3, Pi 0.85.1 on PATH and a configured provider. Optional: `EVAL_THINKING`, `EVAL_TIMEOUT_MS` (default 120 seconds per task), `EVAL_OUTPUT` and `DESKPI_RUNTIME`.

GitHub's manual **Agent evaluation** workflow uses a dedicated `agent-evaluation` environment and its `EVAL_ANTHROPIC_API_KEY` secret. It publishes the JSON report as an artifact. No real-model evaluation has been run as part of this change.

The budget is checked between tasks using reported session cost. The last task can exceed the budget; this is not a provider-enforced spending cap. A timeout or unknown cost stops the entire run. Configure a spending limit with the provider for a strict cap.

Reports record success, individual assertions, final answers, latency, cost, tokens and tool calls. Compare the same complete suite with the same model/settings before and after an instruction change; retain each JSON report. A target is +20 percentage points in task success, with no permission regression. This is a target, not a measured result.

Answer matching is a reproducible smoke metric, sensitive to wording. Manually review source accuracy and unsupported claims in the recorded answers. Python behavior checks establish task outcomes but do not replace review of patch scope. A timeout or missing task counts as a failure; partial runs cannot improve the denominator.

The follow-up case checks context over two turns; process restart recovery is covered by the deterministic host tests.

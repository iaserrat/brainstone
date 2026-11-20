# SQLite evaluations

This suite compares Brainstone, agent-rs, and LangGraph processes using
file-backed SQLite. It separates model task results from execution contracts.
See the [report](../../docs/evaluations.md) for measured results and limits.

## Setup

Run from the repository root. The comparator manifest has its own workspace and
lockfile; its dependencies are not part of Brainstone's published crates.
The Python packages are evaluation-only and pinned in `requirements.txt`.

```sh
cargo build -p brainstone --no-default-features --features openai --example eval_sqlite
cargo build --manifest-path evals/sqlite/agent-rs/Cargo.toml
uv venv /tmp/brainstone-evals-venv
uv pip install --python /tmp/brainstone-evals-venv/bin/python -r evals/sqlite/requirements.txt
python3 -m unittest discover -s evals/sqlite -p 'test_budget.py'
python3 -m unittest discover -s evals/sqlite -p 'test_gateway.py'
python3 -m unittest discover -s evals/sqlite -p 'test_runner.py'
/tmp/brainstone-evals-venv/bin/python -m unittest discover -s evals/sqlite -p 'test_langgraph_adapter.py'
```

The common adapter input is a JSON file containing an action (`invoke`,
`resume`, or `inspect`), state/effect database paths, conversation and request
IDs, system and user text, model, loopback endpoint, optional operation ID, and
an optional fault. Adapters emit flushed NDJSON evidence. They use a dummy key;
only the gateway reads real credentials.

## Fault tests

```sh
python3 evals/sqlite/runner.py --python /tmp/brainstone-evals-venv/bin/python \
  --output data/evals/faults
```

Each fault exits the process with code 73 without unwinding. A new process
resumes accepted work without resubmitting the input. Cases cover input
acceptance, a committed external effect before its result, a committed tool
exchange, reading a terminal outcome, receipt retries, changed retries, and
concurrent drivers. Brainstone's two-second test lease expires before recovery.

The separate application effect ledger uses WAL and full synchronous commits.
It records attempts and deduplicates effects using each framework's durable
call identity. It does not cache prompts or final answers. An exactly-once
application effect here depends on this ledger, not on a universal framework
promise about external services.

LangGraph's accepted event is a synchronous input checkpoint and its operation
locator is the native thread ID. It is not a per-request admission receipt.
Receipt retry checks distinguish input checkpoints from request receipts. The
adapter uses a small native graph and direct Chat Completions calls. It does
not use LangChain's provider integration or hosted platform.

## Live model comparison

Put `OPENROUTER_API_KEY` in the repository's ignored `.env`. The gateway reads
only that entry and never writes it to reports or passes it to child processes.

```sh
python3 evals/sqlite/runner.py --live --provider wafer/us --repetitions 2 \
  --python /tmp/brainstone-evals-venv/bin/python --output data/evals/live
```

The gateway uses only `deepseek/deepseek-v4.1-flash-1017`. Each request has one
completion and at most 768 output tokens. Provider prices are capped at $0.50
per million input tokens and $2 per million output tokens. The gateway reserves
a conservative byte-based token bound before I/O, retains uncertain charges,
and stops at its $0.50 allowance or 80 requests. Unknown costs remain reserved.
The API key's account limit still applies. Choose a fresh output path
for every run; results and logs are under ignored `data/`.

Five tasks test exact text, arithmetic, Unicode, and two required tool calls.
Two-turn recall is a separate check. Assertions compare exact values and actual
effect counts, without a judge model. All adapters use the same task, system
prompt, and tool schema; native message roles, typed-input envelopes, and
structured-output settings remain visible framework differences.

The gateway buffers upstream JSON and converts it to SSE for streaming adapters.
This is not a streaming, cancellation, or time-to-first-token benchmark. Times
include provider latency and process startup, so they do not measure runtime
performance alone. Two repetitions are diagnostic samples, not statistical evidence.

## Request-format diagnostic and structured outputs

```sh
python3 evals/sqlite/ablation.py --live --output data/evals/ablation
python3 evals/sqlite/structured.py --output data/evals/structured-scripted
python3 evals/sqlite/structured.py --live --output data/evals/structured-live
```

The paired diagnostic varies schema output format and native message encoding
with the same provider and prompts. It is capped at $0.03. Structured-output
checks exercise Brainstone's persisted schema contract, malformed-output
rejection, nested data, enums, and Unicode. Their live run is also capped at
$0.03. These allowances are per command; all runs still share the API key's
remaining credit. Consult `budget.json` and `requests.jsonl` before repeating.

The suite has no adapters or results for Rig, Restate, or Temporal.

## Gateway rejection codes

| Code | Meaning |
| --- | --- |
| `eval_request_too_large` | The request was empty or exceeded the byte allowance before paid I/O |
| `eval_wrong_model` | The adapter requested a model other than the fixed evaluation model |
| `eval_gateway_failed` | Budget, upstream, decoding, or response delivery failed; uncertain spend stays reserved |

The standard-library budget, gateway, and scorer tests also run in CI without
credentials or model calls. Comparator installs and paid evaluations remain opt-in.

# SQLite agent evaluation

On November 15, 2026 we tested Brainstone, agent-rs, and a direct LangGraph
graph using local file-backed SQLite. Process recovery tests ran separately
from model tasks. All three passed the core crash-recovery checks.

## Method and evidence

The [suite](../evals/sqlite/README.md) runs three adapters with pinned comparison
dependencies. It uses scripted faults and exact assertions without a judge
model. An OpenRouter gateway limits spending. Saved
[results and request-cost logs](../evals/results/2026-11-15) contain synthetic
inputs and outputs. They contain no credentials or user data.

- Model: `deepseek/deepseek-v4.1-flash-1017`.
- Rust comparator: agent-rs 0.1.3 with its native SQLite store and host.
- Python comparator: LangGraph 1.2.22, SQLite checkpointer 3.1.3, langchain-core 1.8.1.
- The controlled run pinned OpenRouter to the `wafer/us` endpoint. All 66
  recorded requests in that run report Wafer as provider.
- Each request allowed at most 768 output tokens. Comparators use a four-call
  model budget. The first agent-rs run allowed six, but no scored turn needed
  more than two. No case reached either cap.
- The gateway buffered provider JSON and re-encoded it as SSE. These results
  do not measure real streaming, cancellation, or time to first token.

The model runs included 30 single-turn tasks and six scored recall checks,
plus six setup turns for recall. Each framework ran five tasks twice and two
conversations. The runs covered an initial routing pilot, a shared-provider
comparison, a 16-call request-format diagnostic, and three live schema checks.

Total reported cost was **$0.002009796 across 151 paid calls**. The final key
usage matched that total. Zero-cost fault tests were separate. Live comparison
commands had a $0.50 reservation allowance; the format diagnostic and structured
checks each had $0.03 allowances. Those allowances are per command, not an
account-wide limit.

The benchmark ran against a development snapshot. The final fault manifest
records adapter hashes. The results describe that snapshot.

## SQLite execution contracts

Each crash exited the process without unwinding. Recovery opened the database
in a new process and resumed accepted work without supplying another input.
A separate application SQLite ledger deduplicated effects using durable tool
identity. It stored neither prompt receipts nor final answers.

| Check | Brainstone | agent-rs | Direct LangGraph + SQLite |
| --- | --- | --- | --- |
| Resume after accepted/input checkpoint | Passed | Passed | Passed |
| Resume after effect commit, before result | One effect, two attempts | One effect, two attempts | One effect, two attempts |
| Resume after committed tool exchange | One effect, one attempt | One effect, one attempt | One effect, one attempt |
| Read terminal outcome without inference | Passed | Passed | Passed |
| Identical request retry without inference | Native receipt deduplication | Native receipt deduplication | Application responsibility |
| Reject changed input under the same request ID | `idempotency_conflict` | `idempotency_conflict` | Application responsibility |
| Two concurrent processes resume one item | Both succeed; one effect | Second process returns `store_busy`; one effect | Both succeed; two effects |

The concurrent case made two provider requests in Brainstone and agent-rs
(tool turn plus answer), and four in the direct LangGraph adapter. The latter
requires application-side coordination for this service contract. It is not
an evaluation of LangGraph's hosted server, platform concurrency controls, or
LangChain's provider integration. Its synchronous checkpoint recovery passed.

The application ledger is part of the experiment. It does not prove that a
framework can make an arbitrary external service exactly-once. In particular,
two independently generated calls need different effect identities; a ledger
cannot merge them without also merging legitimate repeated actions.

## Model behavior and tool discipline

All three returned the expected normalized answer in 12 of 12 scored cases
in the shared-provider run. The effect ledger showed different tool use:

| Check | Brainstone | agent-rs | LangGraph adapter |
| --- | --- | --- | --- |
| Expected normalized answers | 12/12 | 12/12 | 12/12 |
| Required writes executed | 4/4 | 0/4 | 4/4 |
| Pure-answer tasks with no unrequested write | 0/6 | 6/6 | 0/6 |
| Correct recall after reopening | 2/2 | 2/2 | 2/2 |

The six pure-answer cases were exact text, arithmetic, and Unicode tasks.
Brainstone and LangGraph also wrote during the two recall setup turns. Each
task exposed a write tool to the model. The scorer checked ledger values,
attempt counts, and row counts to determine whether an effect occurred.

agent-rs requests native JSON-schema output by default. Brainstone and the
comparison graph did not request it in the original task run. A paired test
on the same model/provider varied schema format and the combined wire-encoding
factor (system/plain input versus developer/JSON-string input):

| Native schema format | Requested tool called | Unrequested tool called |
| --- | --- | --- |
| Disabled | 4/4 | 4/4 |
| Enabled | 0/4 | 0/4 |

Tool use changed with the schema setting in this sample. This observation is
specific to the tested model, provider, and request formats. It does not show
that schemas disable tools in general or that one framework reasons better.
A valid final answer also does not prove that a required effect occurred. The
ledger checks effects separately.

## Retry identity

Brainstone uses an operation-wide response sequence in tool
retry keys. Regression tests cover steering, compaction, effect commits, process
exit, and recovery.

The LangGraph adapter includes the persisted assistant-message ID in effect
keys. The pilot keyed effects only by provider call ID, which could merge
separate calls. The concurrency table uses the corrected adapter.

## Structured outputs

Brainstone stores an `OutputSchema` in the accepted execution profile. The
Chat Completions, Responses, and Anthropic adapters send native schema fields.
The runtime validates final answers before successful settlement. It refuses
external schema retrieval and preserves the schema across crashes.

All three live structured checks passed: an exact value, nested Unicode data,
and enum/integer fields. Scripted checks passed for both valid output and
schema-invalid output rejected with `structured_output_invalid`. Rust tests
also cover malformed JSON, truncation, provider rejection without prose fallback,
and a crash after assistant commit but before final validation. Compaction
summaries do not inherit the final-answer schema.

See the [structured-output guide](structured-output.md). These checks establish
shape validation, not correct tool decisions or semantic truth.

## Scope and limits

The comparison covers local SQLite. PostgreSQL regression tests passed on an
isolated PostgreSQL 14.18 instance; CI targets PostgreSQL 17. PostgreSQL was
not part of the comparison.

agent-rs uses process-exclusive SQLite ownership. The direct LangGraph adapter
needs application coordination for concurrent execution and request receipts.
The tests do not cover LangGraph's hosted platform or LangChain's provider
integration. Rig, Restate, and Temporal were not tested.

These small samples do not establish relative model quality, throughput, or
production maturity. External effects depend on application idempotency; the
framework cannot guarantee exactly-once effects in an arbitrary service.

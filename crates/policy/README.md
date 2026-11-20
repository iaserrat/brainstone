# policy

Scheduling and silence tools for durable agents. The crate adds
`Schedule`, `ListSchedules`, and `CancelSchedule` to an existing
`agent::Registry`, and exports the built-in automatic-compaction instruction.

Scheduling delegates all state and concurrency guarantees to
`runtime::ScheduleStore`. Calls require the runtime's durable operation context;
`Schedule` also requires the stable tool-call idempotency key. The durable
stores use PostgreSQL or SQLite, while normal tests use `MemoryStore` and never
open a database connection.

`Schedule` creates one run (`run_at`, `after_seconds`) or a standing schedule
the runtime re-runs itself: `repeat_every_minutes`, or `repeat_time` with
`repeat_time_zone` and optional `repeat_days`, `repeat_interval`,
`repeat_month_day`, and `repeat_nth`. The tool maps these fields onto
`runtime::RepeatSpec` and lets the runtime refuse invalid combinations.
`registry` takes the operator's minimum interval; a shorter
`repeat_every_minutes` fails with `schedule_interval_too_short`, and
`DEFAULT_MIN_INTERVAL_MINUTES` (15) is the default the service uses.

`conversation` asks that each run post a new message in that conversation
instead of replying in place. `registry` takes a `surface::Destinations`: the
surface decides whether the requester may post there and returns the
`runtime::Destination`, or a refusal with its own stable code. A call without
an authenticated requester fails with `schedule_destination_forbidden` before
the surface is asked.

`Silence` is the opt-in tool a model calls to end its turn without replying.
It advertises the reserved name and returns a successful result.
`agent::next` recognizes that result and ends the run with empty text.
The transcript retains the decision, and the tool batch remains valid for replay.

The runtime's claim transaction releases due work. The runtime also compacts
at model boundaries. Surfaces use durable admission to queue new work or steer
an active operation. This crate adds no workers or model loop and owns no
provider or surface.

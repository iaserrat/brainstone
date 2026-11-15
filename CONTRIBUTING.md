# Contributing

Read [`AGENTS.md`](AGENTS.md) first. It is the engineering charter for this
repository and it applies to human and automated contributions alike: smallest
correct change, deletion over addition, a failing test before a fix, and the
size budgets in `clippy.toml`.

## Build and verify

`rust-toolchain.toml` pins the compiler; `rust-version` in `Cargo.toml` is the
supported floor (1.95.0).

```sh
cargo xtask gate    # fmt --check, clippy -D warnings, tests, doctests, docs
cargo xtask audit   # machete, deny check (advisories, licenses, bans, sources)
```

The PostgreSQL contract tests need a database that the suite may create and
drop objects in. They are `#[ignore]`d, so `cargo xtask gate` skips them:

```sh
createdb brainstone_test
BRAINSTONE_TEST_DATABASE_URL=postgres://localhost/brainstone_test cargo xtask postgres-test
```

## Change requirements

- A behavior change comes with a test that fails before it and passes after.
  Durability changes come with a crash-resume test.
- A new failure a caller or recovery must react to needs a stable code, a test
  asserting it, and an entry in `docs/failure-codes.md`.
- Operator-visible changes update the matching document in the same patch; see
  the documentation rule in `AGENTS.md`.
- No new dependency without a written reason and a rejected alternative, in the
  comment above the entry in `Cargo.toml`.
- Do not add an advisory ignore to `deny.toml` without recording the exposure.

## Pull requests

One logical change per pull request. `cargo xtask gate` and `cargo xtask audit` must pass, and
CI runs both plus the PostgreSQL suite. Say what the change makes impossible or
unrepresentable, not just what it adds.

## License

Contributions are accepted under the MIT license in [`LICENSE`](LICENSE).

# Contributing

Read [`AGENTS.md`](AGENTS.md) first. Its rules apply to human and automated
contributions. Make the smallest correct change, write a failing test before
a fix, and follow the size limits in `clippy.toml`. Prefer deleting code to
adding it.

## Build and verify

`rust-toolchain.toml` pins the compiler; `rust-version` in `Cargo.toml` is the
minimum supported version.

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

Keep each pull request to one logical change. `cargo xtask gate` and
`cargo xtask audit` must pass. CI runs both and the PostgreSQL suite. Describe
the resulting behavior and the invalid states the change prevents.

## License

Contributions use the MIT license in [`LICENSE`](LICENSE).

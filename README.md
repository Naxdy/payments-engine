# Payments Engine

This repo houses a simple payments engine Rust CLI application. The app can be built normally using `cargo`, but also
via [Nix](https://nixos.org/), like so:

```shell
nix build .# --print-build-logs
```

Tests can be run either via `cargo test` or Nix:

```shell
# This will also run formatting & clippy checks at the same time.
nix flake check . --print-build-logs -j auto
```

The payments engine defines a `Vault` that can make use of any backend that implements `TransactionBackend`. Such a
backend must be able to:

- return a [`Stream`](https://docs.rs/futures/latest/futures/prelude/trait.Stream.html) of `Transaction`s
- find a root transaction by ID
- update the state of a root transaction given its ID and the desired state

Two backends are implemented, in form of a `CsvBackend` which takes in a file path to a `.csv` file containing a list of
ordered transactions, and a `MemoryBackend`, which is built from a simple `Vec<Transaction>` (currently only used in
testing).

Transactions can either be root transactions (`deposit`, `withdrawal`) or non-root transactions (`dispute`, `resolve`,
`chargeback`). Non-root transactions are those that modify the state of another transaction, whereas root transactions
directly modify an account's balance.

For balance calculation, the engine makes use of [`rust_decimal`](https://docs.rs/rust_decimal) in order to avoid
running into floating point imprecision issues.

To run the application, provide it with a path to a `.csv` file containing a list of transactions, like so:

```shell
payments-engine ./transactions.csv

# or

cargo run -- ./transactions.csv
```

The output will be a list of accounts, their final balances, and whether or not an account is frozen (occurs if at least
one `chargeback` transaction is executed on a previously disputed transaction).

If an error is encountered, the application will panic (exit code `1`) with an informative error message. Example on
malformed csv input:

```
thread 'main' (1343304) panicked at src/csv.rs:36:28:
failed to parse csv line: Error(Deserialize { pos: Some(Position { byte: 26, line: 2, record: 1 }), err: DeserializeError { field: None, kind: Message("missing field `client`") } })
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

A more sophisticated approach would be using something like `eyre` and/or `color-eyre`.

## AI Usage Disclaimer

No AI / LLM has been used during development of this project, as I find them more distracting as opposed to helpful.
This includes tools that sit within an editor (e.g. Copilot), as well as chat-based tools. If any screening tool reports
LLM usage, that will be a false positive. My [Neovim config](https://git.naxdy.org/NaxdyOrg/Naxvim) also does not
include any in-editor LLMs.

I did make extensive use of [docs.rs](https://docs.rs) however :)

## How to contribute

First of all thank you for considering it. Check out
[ARCHITECTURE.md](ARCHITECTURE.md) for an overview of how the repo is put together.

### AI policy

No AI generated PRs/Issues.

Whether you use AI as an aid in your development process does not concern me as
long as the result that ends up here is not AI generated.

Reviewing AI code is not fun, and this crate is only maintained for fun!

Any AI generated PR/issues may simply be ignored.

### Running tests

You won't be able to run the tests without enabling the `macros` feature
as one of the examples depend on it.

`cargo test --features macros`



### Debugging the proc macros

To see the output of the macros enable the `dump-macro` feature.
For the macro example the output can be viewed like this:

```sh
cargo r --example macro --features dump-macro,macros -- dark | rustfmt --edition=2024 | less
```

# simple async local executor

[![Crates.io][crates-badge]][crates-url]
[![Docs.rs][docs-badge]][docs-url]
[![Build Status][ci-badge]][ci-url]

[crates-badge]: https://img.shields.io/crates/v/simple-async-local-executor
[crates-url]: https://crates.io/crates/simple-async-local-executor
[docs-badge]: https://img.shields.io/docsrs/simple-async-local-executor
[docs-url]: https://docs.rs/simple-async-local-executor
[ci-badge]: https://img.shields.io/github/workflow/status/enlightware/simple-async-local-executor/CI
[ci-url]: https://github.com/enlightware/simple-async-local-executor/actions

An [Enlightware® software](https://enlightware.ch).

## Overview

A single-threaded polling-based executor suitable for use in games, embedded systems or WASM.
This executor can be useful when the number of tasks is small or if a small percentage is blocked.
Being polling-based, in the general case it trades off efficiency for simplicity and does not require any concurrency primitives such as `Arc`, etc.

## Usage

To use this crate, first add this to your `Cargo.toml`:

```toml
[dependencies]
simple-async-local-executor = "0.1.0"
```

Then, see the [documentation](https://docs.rs/simple-async-local-executor) for more details.

This crate depends by default on `futures-0.3` to provide the `FusedFuture` trait.
If you do not need that, you can disable the `futures` feature and avoid that dependency.

## Examples

Beside documentation and unit tests, the following examples are provided:

 * [`examples/game-units.rs`](examples/game-units.rs): use of async to implement unit behaviours in a friendly way

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

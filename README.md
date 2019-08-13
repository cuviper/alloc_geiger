# alloc_geiger

[![Latest Version]][crates.io] [![docs]][docs.rs]

A Rust allocator which makes sound when active, like a Geiger counter.

Currently this just writes an ASCII [`BEL`] to `/dev/tty`.

Inspired by [Malloc Geiger].

## Usage

To use `alloc_geiger` add it as a dependency:

```toml
# Cargo.toml
[dependencies]
alloc_geiger = "0.1"
```

To set `alloc_geiger::Geiger` as the global allocator, it must be initialized
with an underlying allocator. The `type System` alias and its `const SYSTEM`
make it easy to use the default system allocator:

```rust
#[global_allocator]
static ALLOC: alloc_geiger::System = alloc_geiger::SYSTEM;
```

Alternatives like [`jemallocator`] may also be used:

```rust
use alloc_geiger::Geiger;
use jemallocator::Jemalloc;

#[global_allocator]
static ALLOC: Geiger<Jemalloc> = Geiger::new(Jemalloc);
```


## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `alloc_geiger` by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

[`BEL`]: https://en.wikipedia.org/wiki/Bell_character
[Malloc Geiger]: https://github.com/laserallan/malloc_geiger
[`jemallocator`]: https://crates.io/crates/jemallocator
[Latest Version]: https://img.shields.io/crates/v/alloc_geiger.svg
[crates.io]: https://crates.io/crates/alloc_geiger
[docs]: https://docs.rs/alloc_geiger/badge.svg
[docs.rs]: https://docs.rs/alloc_geiger/

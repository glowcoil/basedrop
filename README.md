# 🐟⤵️ basedrop

A set of memory-management tools for real-time audio and other latency-critical scenarios.

`basedrop` provides smart pointers analogous to `Box` and `Arc` which mark their contents for deferred collection on another thread rather than immediately freeing it, making them safe to drop on a real-time thread.

## License

`basedrop` is distributed under the terms of both the [MIT license](LICENSE-MIT) and the [Apache license, version 2.0](LICENSE-APACHE). Contributions are accepted under the same terms.

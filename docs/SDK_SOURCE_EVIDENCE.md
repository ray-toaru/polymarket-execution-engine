# SDK Source Evidence

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Observed source: `https://github.com/Polymarket/rs-clob-client-v2`.

Evidence summary as of v0.7 review:

- Repository name: `Polymarket/rs-clob-client-v2`.
- Crate name: `polymarket_client_sdk_v2`.
- README describes it as a Rust client for Polymarket services, primarily CLOB.
- README documents feature flags including `clob`, `ws`, `rtds`, `data`, `gamma`, `bridge`, `rfq`, `heartbeats`, and `ctf`.
- README documents authenticated client setup with `POLYMARKET_PRIVATE_KEY`, `LocalSigner`, `POLYGON`, `Client::new("https://clob-v2.polymarket.com", Config::default())`, `authentication_builder(&signer)`, and `authenticate()`.
- README documents order build/sign/post flow through `limit_order()` / `market_order()`, `client.sign(&signer, order)`, and `client.post_order(signed_order)`.
- README documents WebSocket streaming with the `ws` feature and authenticated user streams.
- Cargo metadata observed from repository pins crate version `0.6.0-canary.1`, edition `2024`, and rust-version `1.88.0`.

Implication:

The official SDK should be the preferred real adapter source. However, its MSRV means the execution engine must either raise its workspace MSRV or isolate the SDK adapter behind a separately validated crate/profile.

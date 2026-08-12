# Synthetic OHLCV reference evaluation

- `market-data-emulator` is the primary model reference: deterministic seed, OU mean reversion, scenarios, OHLC constraints, deterministic resampling and multi-pair extensions.
- `PriceGenerator` is a secondary candle-shape reference: bounded body, wick/outlier probability, direction probability and volume ranges.
- `trade-data-generator` is an event architecture reference: one feed emits ticker/candle/depth and supports pluggable sources.
- GAN/VAE and Swift implementations are inappropriate runtime dependencies for this Rust backend MVP.
- Project decision: port the useful algorithms/patterns to Rust and reuse existing `MarketIngestionService`/WebSocket contracts.

# Indicator Definitions

All indicators are calculated in Rust (`apps/api/src/indicators.rs`) over the requested bar series. Warm-up values are `null`, not zero-filled.

| Indicator | Parameters | Definition |
| --- | --- | --- |
| MA | 5/10/20/60 | Simple moving average of close |
| EMA | 12 | `alpha = 2/(n+1)`, seeded with first close |
| BOLL | 20, 2σ | Middle = SMA20; upper/lower = middle ± 2 × population stddev |
| MACD | 12/26/9 | DIF = EMA12 − EMA26; DEA = EMA9(DIF); MACD histogram = (DIF − DEA) × 2 |
| KDJ | 9, 3, 3 | RSV from 9-day high/low; K = (2K + RSV)/3; D = (2D + K)/3; J = 3K − 2D; seeded at 50 |
| RSI | 14 | Wilder smoothing; 100 when average loss is zero |
| VOL | — | Raw volume from `quote_daily` |

## Units and display

- Price: CNY; volume: shares; amount: CNY.
- Percent change uses previous close; a zero previous close returns 0.
- The UI shows `—` for missing values and labels the data timestamp.
- Demo prices are synthetic; indicators are still deterministic for tests.

## Tests

```bash
cargo test --manifest-path apps/api/Cargo.toml
```

Golden tests cover SMA warm-up, EMA sequence, flat-series BOLL/MACD/KDJ, RSI extremes, and bar population.

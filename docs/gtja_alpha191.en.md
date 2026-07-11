# Guotai Junan Alpha191

[中文](gtja_alpha191.md)

qweave exposes 191 Guotai Junan short-horizon price/volume factors as built-in
expressions. The builder is `gtja_alpha191`; output names are fixed from
`gtja_alpha001` through `gtja_alpha191`.

```python
alphas = qweave.gtja_alpha191(
    {"close": "adj_close"},
    alphas=["gtja_alpha001", "gtja_alpha191"],
)
out = qweave.compute_alphas(df, "asset", "time", alphas)
```

Omitting `alphas` returns all 191 expressions. A requested subset preserves its
input order. `input_alias` maps canonical inputs to physical DataFrame columns.

## Inputs and calibers

Base inputs are `open`, `close`, `high`, `low`, `volume`, and `vwap`. Some factors
also require:

- `index_open` and `index_close` for benchmark series;
- `mkt`, `smb`, and `hml` for Alpha30's three-factor regression.

Extra time series are panel columns repeated for every asset on the same date.
`amount` is derived as `volume * vwap`. qweave does not choose adjustment,
suspension-fill, or benchmark-calendar rules; callers must normalize them first.

`SMA(A,n,m)` uses recursive smoothing with coefficient `m/n`; `WMA(A,n)` uses the
report's `0.9^i` weights. Ranking, full-window, and missing-value behavior follows
qweave's expression engine. See [formula sources](gtja_alpha191_sources.en.md) for
source pages and known ambiguities.

## Verification

- The exact name-set test covers `gtja_alpha001`–`gtja_alpha191`.
- A full smoke test evaluates all 191 factors on a 320-day synthetic panel.
- Recursive smoothing, weighted moving average, and regression kernels have
  independent hand-calculated tests.
- Fixtures contain synthetic data only.

This project is not affiliated with Guotai Junan Securities. The formulas are not
investment advice.

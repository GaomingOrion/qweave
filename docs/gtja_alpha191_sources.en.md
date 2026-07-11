# Guotai Junan Alpha191 Formula Sources

[中文](gtja_alpha191_sources.md)

This note records the authoritative formula sources, cross-check order, and known
ambiguities for qweave's built-in reproduction of 191 Guotai Junan short-horizon
price/volume factors. The public collection name is `gtja_alpha191`, and
its outputs are named `gtja_alpha001` through `gtja_alpha191`. External sources
usually call the collection `GTJA Alpha191` or `gtja191Alpha`.

This is not a copy of the Guotai Junan report and is not investment advice. The
repository stores verifiable links, page locations, and implementation-caliber
notes instead of redistributing the copyrighted PDF.

## Source precedence

1. **Original formulas:** Liu Fubing et al., Guotai Junan Securities,
   [A Multi-Factor Stock Selection System Based on Short-Horizon Price and Volume Features](https://guorn.com/static/upload/file/3/134065454575605.pdf),
   published on 2017-06-15. Table 6 on PDF pages 11–17 lists Alpha1–Alpha191;
   page 31 defines operators including `RANK`, `TSRANK`, `SMA`, `WMA`, and
   `REGBETA`. The checked file has SHA-256
   `863f62c2e23bd87ddb42b8338c8fe2b0276d94260ac985e1a0edfff318693c6c`.
2. **Reference implementation:** DolphinDB's official
   [GTJA 191 Alpha documentation](https://docs.dolphindb.cn/zh/2.00.16/modules/gtja191Alpha/191alpha.html)
   and attached
   [`gtja191Alpha.dos`](https://docs.dolphindb.cn/zh/2.00.16/modules/gtja191Alpha/src/gtja191Alpha.dos).
   The script contains 191 `gtjaAlpha#` functions and is useful for parsing and
   input-field checks, but it is not an unconditional golden oracle. The checked
   script has SHA-256
   `e3d93adcdacff263795b8f0b97de1b806ce3666b118f7819af59d9ef5ff98543`.

The sources were checked on 2026-07-11. Implementation should follow the economic
meaning of the report first and use DolphinDB to resolve typesetting ambiguity.
Any disagreement must become an explicit qweave caliber with a focused test.

## Inputs

The formulas use daily `open`, `close`, `high`, `low`, `volume`, and `vwap`, plus:

- `returns = close / delay(close, 1) - 1`;
- `amount = volume * vwap`, as used by DolphinDB for Alpha70, Alpha95, Alpha132,
  and Alpha144;
- `index_open` and `index_close` for Alpha75, Alpha149, Alpha181, and Alpha182;
- `MKT`, `SMB`, and `HML` for the Fama–French residual regression in Alpha30;
- `SELF` in Alpha143, meaning the factor's previous-day recursive value.

DolphinDB's input table is a useful minimum-field checklist. qweave must still
define adjustment, suspension, volume-unit, and index-calendar alignment rules.

## Operator calibers

Page 31 of the report defines the core operators:

- `RANK(A)` is an ascending cross-sectional rank; `TSRANK(A,n)` ranks the current
  value within its trailing window.
- `SMA(A,n,m)` is recursive smoothing,
  `Y[t] = (m*A[t] + (n-m)*Y[t-1]) / n`, not a simple moving average.
- `WMA(A,n)` uses `0.9^i`, where `i` is the observation's distance from now.
- `DECAYLINEAR(A,d)` uses normalized weights `d, d-1, ..., 1`.
- `COUNT`, `SUMIF`, and `FILTER` provide rolling conditional operations.
- `REGBETA` and `REGRESI` return rolling regression coefficients and residuals.
- `HIGHDAY/LOWDAY` return the distance to a window extreme. The report describes
  `LOWDAY` as a maximum too; formula semantics and the reference imply minimum.
- `SUMAC` is under-specified. DolphinDB treats it as rolling `SUM` for
  Alpha165/183, followed by cross-sectional extremes.

DolphinDB documents percentile `RANK`/`TSRANK`, smoothing parameter `m/n`, and
`SUMAC = SUM`. qweave's tie, missing-value, and full-window behavior may differ,
so matching operator names do not by themselves establish equivalent results.

## Formula locations and representative entries

The full formula table is located as follows:

| PDF page | Factors |
| --- | --- |
| 11 | Alpha1–Alpha35 |
| 12 | Alpha36–Alpha58 |
| 13 | Alpha59–Alpha91 |
| 14 | Alpha92–Alpha124 |
| 15 | Alpha125–Alpha151 |
| 16 | Alpha152–Alpha180 |
| 17 | Alpha181–Alpha191 |

Representative entries show the range of implementation complexity:

```text
Alpha1   = -CORR(RANK(DELTA(LOG(VOLUME), 1)),
                 RANK((CLOSE - OPEN) / OPEN), 6)
Alpha30  = WMA(REGRESI(CLOSE / DELAY(CLOSE, 1) - 1,
                       MKT, SMB, HML, 60)^2, 20)
Alpha143 = CLOSE > DELAY(CLOSE, 1)
           ? (CLOSE - DELAY(CLOSE, 1)) / DELAY(CLOSE, 1) * SELF
           : SELF
Alpha191 = CORR(MEAN(VOLUME, 20), LOW, 5) + (HIGH + LOW) / 2 - CLOSE
```

Before transcription, inspect both the original PDF page and the corresponding
DolphinDB function to avoid errors introduced by PDF text extraction.

## Known high-risk formulas

- Typesetting and spelling errors include `COVIANCE`, `DELAT`, `SMEAN`,
  `BANCHMARKINDEX*`, `HGIH`, and Alpha52's `L`.
- Alpha7, Alpha28, Alpha50, Alpha78, Alpha159, Alpha165, Alpha166, Alpha181,
  Alpha183, and Alpha190 need explicit precedence reconstruction.
- Alpha21, Alpha30, Alpha116, Alpha147, and Alpha149 require regression or dynamic
  filtering.
- Alpha143 is stateful and cannot directly use qweave's current stateless tree.
- Alpha75, Alpha149, Alpha181, and Alpha182 depend on benchmark alignment.
- Alpha103, Alpha133, Alpha165, Alpha177, and Alpha183 depend on extreme-position
  or cumulative-value semantics.
- The reference itself needs auditing: DolphinDB Alpha7 does not preserve all
  source parentheses, while Alpha165/183 need independent precedence tests.

Here, reproduction means implementing all 191 formulas under documented qweave
calibers and explaining differences against an external implementation. It does
not imply bitwise equivalence under unspecified data-processing conventions.

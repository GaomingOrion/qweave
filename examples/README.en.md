# Runnable Quickstart

[中文](README.md)

`data/sample_daily.parquet` is a deterministic synthetic daily panel with 80
assets, 320 trading days, and OHLCV, industry, and tradability fields. It does
not contain real market data.

Complete the source installation on the repository home page, then run:

```powershell
uv run python examples\quickstart.py
```

The script computes two WorldQuant factors and one custom expression, creates
1/5/20-day forward-return labels, runs IC, quantile-return, turnover, and
long-short diagnostics, and writes `examples\output\qweave-report.html`.

To regenerate the sample panel:

```powershell
uv run python examples\generate_sample_data.py
```

The sample output verifies the research workflow. It is not evidence of real
market performance or investment advice.

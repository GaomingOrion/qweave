# Runnable Quickstart

[中文](README.md)

`data/sample_daily.parquet` is a deterministic synthetic daily panel with 80
assets, 320 trading days, and OHLCV, industry, and tradability fields. It does
not contain real market data.

Install qweave first (or build from source as described on the repository
home page):

```powershell
python -m pip install https://github.com/GaomingOrion/qweave/releases/download/v0.4.1/qweave-0.4.1-cp310-abi3-win_amd64.whl
```

Then run:

```powershell
python examples\quickstart.py
```

The script computes two WorldQuant factors and one custom expression, creates
1/5/20-day forward-return labels, runs IC, quantile-return, turnover, and
long-short diagnostics, prints the summary table, and opens the interactive
evaluation report in your browser via `result.view()` (Ctrl-C to stop).

To regenerate the sample panel:

```powershell
python examples\generate_sample_data.py
```

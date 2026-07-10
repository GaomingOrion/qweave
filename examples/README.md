# 可运行的快速示例

[English](README.en.md)

`data/sample_daily.parquet` 是一个固定随机种子生成的合成日频面板，包含 80 个资产、
320 个交易日和 OHLCV、行业、可交易性等字段，不含真实市场数据。

先按仓库首页完成源码安装，然后运行：

```powershell
uv run python examples\quickstart.py
```

脚本会计算两个 WorldQuant 因子和一个自定义表达式，生成 1/5/20 日 forward-return
标签，完成 IC、分位收益、换手和多空诊断，并写出
`examples\output\qweave-report.html`。

重新生成示例数据：

```powershell
uv run python examples\generate_sample_data.py
```

示例输出只用于验证研究流程，不代表真实市场表现或投资建议。

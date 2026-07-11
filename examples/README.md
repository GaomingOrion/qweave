# 可运行的快速示例

[English](README.en.md)

`data/sample_daily.parquet` 是一个固定随机种子生成的合成日频面板，包含 80 个资产、
320 个交易日和 OHLCV、行业、可交易性等字段，不含真实市场数据。

先安装 qweave（也可以按仓库首页从源码构建）：

```powershell
python -m pip install https://github.com/GaomingOrion/qweave/releases/download/v0.4.1/qweave-0.4.1-cp310-abi3-win_amd64.whl
```

然后运行：

```powershell
python examples\quickstart.py
```

脚本会计算两个 WorldQuant 因子和一个自定义表达式，生成 1/5/20 日 forward-return
标签，完成 IC、分位收益、换手和多空诊断，打印汇总表，并用 `result.view()` 在
浏览器中打开交互式评估报告（Ctrl-C 退出）。

重新生成示例数据：

```powershell
python examples\generate_sample_data.py
```

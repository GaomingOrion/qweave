# 国泰君安 Alpha191 公式来源

[English](gtja_alpha191_sources.en.md)

本文记录 qweave 内置国泰君安 191 个短周期价量因子的可靠公式来源、
核对顺序和已知歧义。公开集合名称按项目约定写作 `gtja_alpha191`，
输出名称为 `gtja_alpha001` 至 `gtja_alpha191`；原研报和外部实现通常使用
`GTJA Alpha191` 或 `gtja191Alpha`。

本文不是国泰君安研报的副本，也不构成投资建议。原研报受其权利声明约束，
因此仓库只保存可复核的来源链接、定位信息和实现所需的口径笔记，不重新分发 PDF。

## 来源优先级

1. **原始公式：** 国泰君安证券刘富兵等于 2017-06-15 发布的
   [《基于短周期价量特征的多因子选股体系——数量化专题之九十三》](https://guorn.com/static/upload/file/3/134065454575605.pdf)。
   表 6（PDF 第 11–17 页）逐项列出 Alpha1–Alpha191；附录（第 31 页）定义
   `RANK`、`TSRANK`、`SMA`、`WMA`、`REGBETA` 等算子。本文核对文件的
   SHA-256 为 `863f62c2e23bd87ddb42b8338c8fe2b0276d94260ac985e1a0edfff318693c6c`。
2. **参考实现：** DolphinDB 官方
   [国泰君安 191 Alpha 因子库文档](https://docs.dolphindb.cn/zh/2.00.16/modules/gtja191Alpha/191alpha.html)
   和随文发布的
   [`gtja191Alpha.dos`](https://docs.dolphindb.cn/zh/2.00.16/modules/gtja191Alpha/src/gtja191Alpha.dos)。
   该脚本包含 191 个 `gtjaAlpha#` 函数，可用于核对公式解析和输入字段，但不是
   无条件的 golden oracle。本文核对脚本的 SHA-256 为
   `e3d93adcdacff263795b8f0b97de1b806ce3666b118f7819af59d9ef5ff98543`。

校验日期为 2026-07-11。后续实现应先服从原研报的经济含义，再用 DolphinDB
实现消除排版歧义；两者冲突时必须记录项目口径并添加针对性测试。

## 数据字段

公式使用个股日频 `open`、`close`、`high`、`low`、`volume`、`vwap`，以及下列
派生或额外输入：

- `returns = close / delay(close, 1) - 1`；
- `amount = volume * vwap`，DolphinDB 对 Alpha70、Alpha95、Alpha132 和
  Alpha144 采用此换算；
- `index_open`、`index_close`，用于 Alpha75、Alpha149、Alpha181 和 Alpha182；
- `MKT`、`SMB`、`HML`，用于 Alpha30 的 Fama–French 三因子残差回归；
- `SELF`，只在 Alpha143 出现，表示该因子前一日的递归结果。

DolphinDB 的字段一览表可用来检查每个因子的最小输入集，但字段语义仍应由 qweave
明确约定，例如价格是否复权、停牌日如何处理、`volume` 的单位以及指数序列如何与
个股交易日对齐。

## 算子口径

原研报第 31 页给出以下核心定义：

- `RANK(A)`：当日横截面升序排名；`TSRANK(A, n)`：当前值在过去 `n` 日的排名。
- `DELAY(A, n)`、`DELTA(A, n)`、`SUM/MEAN/STD(A, n)`、
  `TSMIN/TSMAX(A, n)`：标准滚动时序算子。
- `CORR/COVIANCE(A, B, n)`：过去 `n` 日相关系数/协方差；原文将
  `COVARIANCE` 拼作 `COVIANCE`。
- `SMA(A, n, m)`：递归平滑，`Y[t] = (m*A[t] + (n-m)*Y[t-1]) / n`，
  不是简单移动平均。
- `WMA(A, n)`：以 `0.9^i` 加权，其中 `i` 是样本距当前时点的间隔。
- `DECAYLINEAR(A, d)`：过去 `d` 日按 `d, d-1, ..., 1` 线性加权并归一化。
- `COUNT`、`SUMIF`、`FILTER`：滚动条件计数、条件求和和条件筛选。
- `REGBETA`、`REGRESI`：滚动回归系数和残差。
- `HIGHDAY/LOWDAY`：窗口极值距离当前时点的间隔；附录把 `LOWDAY` 也写成
  “最大值”，应按公式语义和参考实现解释为最小值。
- `SUMAC`：原文定义含糊；DolphinDB 在 Alpha165/183 中将其实现为滚动求和，
  但后续又做横截面极值，必须单独校准。

DolphinDB 公开说明其实现将 `RANK` 和 `TSRANK` 设为百分比排名，将
`SMA(A,n,m)` 的平滑参数设为 `m/n`，并把 `SUMAC` 当作 `SUM`。qweave 现有
`rank`/`ts_rank` 的并列值、缺失值和满窗口规则未必与 DolphinDB 完全一致，不能仅凭
算子同名判断结果一致。

## 公式定位与代表项

完整 191 条原始公式位于研报表 6。按 PDF 页码定位如下：

| 页码 | 因子 |
| --- | --- |
| 11 | Alpha1–Alpha35 |
| 12 | Alpha36–Alpha58 |
| 13 | Alpha59–Alpha91 |
| 14 | Alpha92–Alpha124 |
| 15 | Alpha125–Alpha151 |
| 16 | Alpha152–Alpha180 |
| 17 | Alpha181–Alpha191 |

四个代表项展示了实现复杂度的跨度：

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

转录实现前应同时查看研报原页和 DolphinDB 对应函数，避免 PDF 文本抽取造成换行、
全角标点或括号丢失。

## 已知高风险公式

以下问题已经在原文或参考实现中观察到，不能在编码时静默猜测：

- 拼写与排版：`COVIANCE`、`DELAT`、`SMEAN`、`BANCHMARKINDEX*`、`HGIH`、
  Alpha52 的 `L` 等明显笔误。
- 括号或优先级：Alpha7、Alpha28、Alpha50、Alpha78、Alpha159、Alpha165、
  Alpha166、Alpha181、Alpha183、Alpha190 需要逐项重建语法树。
- 回归与动态筛选：Alpha21、Alpha30、Alpha116、Alpha147、Alpha149。
- 递归状态：Alpha143 不能直接表示为当前 qweave 的无状态表达式树。
- 市场基准：Alpha75、Alpha149、Alpha181、Alpha182 依赖指数输入及对齐口径。
- 极值位置与累计值：Alpha103、Alpha133、Alpha165、Alpha177、Alpha183。
- 参考实现自身也需审计。例如 DolphinDB Alpha7 的表达式括号与原公式不完全一致，
  Alpha165/183 的运算优先级需要用独立样例确认。

因此，“复刻”应定义为：在公开记录的 qweave 口径下完整实现 191 个公式，并用独立
合成样例和至少一个外部实现做差异解释；不承诺对任何未声明数据处理口径逐位一致。

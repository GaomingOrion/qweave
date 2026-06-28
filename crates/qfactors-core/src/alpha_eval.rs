use std::cmp::Ordering;
use std::collections::HashMap;

use crate::cellset::CellSet;
use crate::error::{QFactorsError, Result};
use crate::expr::{CmpOp, Expr};
use crate::layout::{Layout, nt_to_tn, tn_to_nt};

#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Cells { values: Vec<f64>, layout: Layout },
    Scalar(f64),
}

pub fn eval(expr: &Expr, cs: &CellSet) -> Result<Val> {
    match expr {
        Expr::Field(name) => cs
            .fields
            .get(name)
            .cloned()
            .map(|values| Val::Cells {
                values,
                layout: Layout::Nt,
            })
            .ok_or_else(|| QFactorsError::MissingColumn(name.clone())),
        Expr::Const(value) => Ok(Val::Scalar(*value)),
        Expr::Add(lhs, rhs) => eval_binary(lhs, rhs, cs, |a, b| a + b),
        Expr::Sub(lhs, rhs) => eval_binary(lhs, rhs, cs, |a, b| a - b),
        Expr::Mul(lhs, rhs) => eval_binary(lhs, rhs, cs, |a, b| a * b),
        Expr::Div(lhs, rhs) => eval_binary(lhs, rhs, cs, |a, b| a / b),
        Expr::Neg(inner) => match eval(inner, cs)? {
            Val::Scalar(value) => Ok(Val::Scalar(-value)),
            Val::Cells { values, layout } => Ok(Val::Cells {
                values: values.into_iter().map(|value| -value).collect(),
                layout,
            }),
        },
        Expr::Delay(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: delay(&values, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::Delta(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: delta(&values, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::TsSum(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: ts_sum(&values, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::TsMean(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: ts_mean(&values, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::Product(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: product(&values, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::TsMin(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: ts_min(&values, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::TsMax(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: ts_max(&values, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::TsArgMin(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: ts_argmin(&values, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::TsArgMax(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: ts_argmax(&values, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::TsRank(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: ts_rank(&values, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::TsStd(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: ts_std(&values, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::DecayLinear(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: decay_linear(&values, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::Correlation(lhs, rhs, days) => {
            let lhs = to_cells(eval(lhs, cs)?, Layout::Nt, cs);
            let rhs = to_cells(eval(rhs, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: correlation(&lhs, &rhs, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::Covariance(lhs, rhs, days) => {
            let lhs = to_cells(eval(lhs, cs)?, Layout::Nt, cs);
            let rhs = to_cells(eval(rhs, cs)?, Layout::Nt, cs);
            Ok(Val::Cells {
                values: covariance(&lhs, &rhs, *days, cs),
                layout: Layout::Nt,
            })
        }
        Expr::Rank(inner) => {
            let values = to_cells(eval(inner, cs)?, Layout::Tn, cs);
            Ok(Val::Cells {
                values: rank(&values, cs),
                layout: Layout::Tn,
            })
        }
        Expr::Scale(inner, scale_to) => {
            let values = to_cells(eval(inner, cs)?, Layout::Tn, cs);
            Ok(Val::Cells {
                values: scale(&values, *scale_to, cs),
                layout: Layout::Tn,
            })
        }
        Expr::GroupRank(values, groups) => {
            let values = to_cells(eval(values, cs)?, Layout::Tn, cs);
            let groups = to_cells(eval(groups, cs)?, Layout::Tn, cs);
            Ok(Val::Cells {
                values: group_rank(&values, &groups, cs),
                layout: Layout::Tn,
            })
        }
        Expr::GroupNeutralize(values, groups) => {
            let values = to_cells(eval(values, cs)?, Layout::Tn, cs);
            let groups = to_cells(eval(groups, cs)?, Layout::Tn, cs);
            Ok(Val::Cells {
                values: group_neutralize(&values, &groups, cs),
                layout: Layout::Tn,
            })
        }
        Expr::Abs(inner) => eval_unary(inner, cs, f64::abs),
        Expr::Log(inner) => eval_unary(inner, cs, log_value),
        Expr::Sign(inner) => eval_unary(inner, cs, sign),
        Expr::SignedPower(inner, exponent) => {
            eval_unary(inner, cs, |value| signed_power(value, *exponent))
        }
        Expr::Power(inner, exponent) => eval_unary(inner, cs, |value| value.powf(*exponent)),
        Expr::Min(lhs, rhs) => eval_binary(lhs, rhs, cs, min_value),
        Expr::Max(lhs, rhs) => eval_binary(lhs, rhs, cs, max_value),
        Expr::Cmp(op, lhs, rhs) => eval_binary(lhs, rhs, cs, |lhs, rhs| cmp_value(*op, lhs, rhs)),
        Expr::Where(cond, when_true, when_false) => eval_where(cond, when_true, when_false, cs),
    }
}

pub fn to_cells(value: Val, want: Layout, cs: &CellSet) -> Vec<f64> {
    match value {
        Val::Scalar(value) => vec![value; cs.n_cells],
        Val::Cells { values, layout } if layout == want => values,
        Val::Cells {
            values,
            layout: Layout::Nt,
        } => nt_to_tn(&values, cs),
        Val::Cells {
            values,
            layout: Layout::Tn,
        } => tn_to_nt(&values, cs),
    }
}

fn eval_binary(lhs: &Expr, rhs: &Expr, cs: &CellSet, op: impl Fn(f64, f64) -> f64) -> Result<Val> {
    let lhs = eval(lhs, cs)?;
    let rhs = eval(rhs, cs)?;
    match (&lhs, &rhs) {
        (Val::Scalar(lhs), Val::Scalar(rhs)) => Ok(Val::Scalar(op(*lhs, *rhs))),
        _ => {
            let layout = match (&lhs, &rhs) {
                (Val::Cells { layout, .. }, _) | (_, Val::Cells { layout, .. }) => *layout,
                (Val::Scalar(_), Val::Scalar(_)) => unreachable!("handled above"),
            };
            let lhs = to_cells(lhs, layout, cs);
            let rhs = to_cells(rhs, layout, cs);
            Ok(Val::Cells {
                values: lhs
                    .into_iter()
                    .zip(rhs)
                    .map(|(lhs, rhs)| op(lhs, rhs))
                    .collect(),
                layout,
            })
        }
    }
}

fn eval_unary(inner: &Expr, cs: &CellSet, op: impl Fn(f64) -> f64) -> Result<Val> {
    match eval(inner, cs)? {
        Val::Scalar(value) => Ok(Val::Scalar(op(value))),
        Val::Cells { values, layout } => Ok(Val::Cells {
            values: values.into_iter().map(op).collect(),
            layout,
        }),
    }
}

fn eval_where(cond: &Expr, when_true: &Expr, when_false: &Expr, cs: &CellSet) -> Result<Val> {
    let cond = eval(cond, cs)?;
    let when_true = eval(when_true, cs)?;
    let when_false = eval(when_false, cs)?;

    let layout = [&cond, &when_true, &when_false]
        .into_iter()
        .find_map(|value| match value {
            Val::Cells { layout, .. } => Some(*layout),
            Val::Scalar(_) => None,
        });

    let Some(layout) = layout else {
        let Val::Scalar(cond) = cond else {
            unreachable!("all values are scalar");
        };
        let Val::Scalar(when_true) = when_true else {
            unreachable!("all values are scalar");
        };
        let Val::Scalar(when_false) = when_false else {
            unreachable!("all values are scalar");
        };
        return Ok(Val::Scalar(where_value(cond, when_true, when_false)));
    };

    let cond = to_cells(cond, layout, cs);
    let when_true = to_cells(when_true, layout, cs);
    let when_false = to_cells(when_false, layout, cs);
    Ok(Val::Cells {
        values: cond
            .into_iter()
            .zip(when_true)
            .zip(when_false)
            .map(|((cond, when_true), when_false)| where_value(cond, when_true, when_false))
            .collect(),
        layout,
    })
}

fn delay(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    let mut out = vec![f64::NAN; values.len()];
    for block in &cs.sym_blocks {
        for local_idx in days..block.len() {
            out[block.start + local_idx] = values[block.start + local_idx - days];
        }
    }
    out
}

fn ts_window(
    values: &[f64],
    days: usize,
    cs: &CellSet,
    reduce: impl Fn(&[f64]) -> f64,
) -> Vec<f64> {
    let mut out = vec![f64::NAN; values.len()];
    if days == 0 {
        return out;
    }

    for block in &cs.sym_blocks {
        if block.len() < days {
            continue;
        }
        for local_idx in days - 1..block.len() {
            let start = block.start + local_idx + 1 - days;
            let end = block.start + local_idx;
            let window = &values[start..=end];
            if window.iter().all(|value| !value.is_nan()) {
                out[end] = reduce(window);
            }
        }
    }
    out
}

fn ts_window2(
    lhs: &[f64],
    rhs: &[f64],
    days: usize,
    cs: &CellSet,
    reduce: impl Fn(&[f64], &[f64]) -> f64,
) -> Vec<f64> {
    let mut out = vec![f64::NAN; lhs.len()];
    if days == 0 {
        return out;
    }

    for block in &cs.sym_blocks {
        if block.len() < days {
            continue;
        }
        for local_idx in days - 1..block.len() {
            let start = block.start + local_idx + 1 - days;
            let end = block.start + local_idx;
            let lhs_window = &lhs[start..=end];
            let rhs_window = &rhs[start..=end];
            if lhs_window.iter().all(|value| !value.is_nan())
                && rhs_window.iter().all(|value| !value.is_nan())
            {
                out[end] = reduce(lhs_window, rhs_window);
            }
        }
    }
    out
}

fn xs_per_block(
    values: &[f64],
    cs: &CellSet,
    f: impl Fn(&[(usize, f64)]) -> Vec<(usize, f64)>,
) -> Vec<f64> {
    let mut out = vec![f64::NAN; values.len()];

    for block in &cs.time_blocks {
        let present = block
            .clone()
            .filter_map(|idx| {
                let value = values[idx];
                (!value.is_nan()).then_some((idx, value))
            })
            .collect::<Vec<_>>();

        for (idx, value) in f(&present) {
            out[idx] = value;
        }
    }

    out
}

fn xs_per_group(
    values: &[f64],
    groups: &[f64],
    cs: &CellSet,
    f: impl Fn(&[(usize, f64)]) -> Vec<(usize, f64)>,
) -> Vec<f64> {
    let mut out = vec![f64::NAN; values.len()];

    for block in &cs.time_blocks {
        let mut buckets: HashMap<u64, Vec<(usize, f64)>> = HashMap::new();
        for idx in block.clone() {
            let value = values[idx];
            let group = groups[idx];
            if value.is_nan() || group.is_nan() {
                continue;
            }
            buckets
                .entry(group.to_bits())
                .or_default()
                .push((idx, value));
        }

        for bucket in buckets.values() {
            for (idx, value) in f(bucket) {
                out[idx] = value;
            }
        }
    }

    out
}

fn delta(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    let mut out = vec![f64::NAN; values.len()];

    for block in &cs.sym_blocks {
        for local_idx in days..block.len() {
            let idx = block.start + local_idx;
            let current = values[idx];
            let previous = values[idx - days];
            if !current.is_nan() && !previous.is_nan() {
                out[idx] = current - previous;
            }
        }
    }

    out
}

fn ts_sum(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, |window| window.iter().sum())
}

fn ts_mean(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, |window| {
        window.iter().sum::<f64>() / window.len() as f64
    })
}

fn product(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, |window| window.iter().product())
}

fn ts_min(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, |window| {
        window
            .iter()
            .copied()
            .fold(f64::INFINITY, |acc, value| acc.min(value))
    })
}

fn ts_max(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, |window| {
        window
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, |acc, value| acc.max(value))
    })
}

fn ts_argmin(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, |window| {
        let mut best_days_ago = 0usize;
        let mut best_value = window[window.len() - 1];
        for days_ago in 1..window.len() {
            let value = window[window.len() - 1 - days_ago];
            if value < best_value {
                best_value = value;
                best_days_ago = days_ago;
            }
        }
        best_days_ago as f64
    })
}

fn ts_argmax(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, |window| {
        let mut best_days_ago = 0usize;
        let mut best_value = window[window.len() - 1];
        for days_ago in 1..window.len() {
            let value = window[window.len() - 1 - days_ago];
            if value > best_value {
                best_value = value;
                best_days_ago = days_ago;
            }
        }
        best_days_ago as f64
    })
}

fn ts_rank(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, rank_last)
}

fn ts_std(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, |window| {
        if window.len() < 2 {
            return f64::NAN;
        }
        let mean = window.iter().sum::<f64>() / window.len() as f64;
        let variance = window
            .iter()
            .map(|value| {
                let centered = value - mean;
                centered * centered
            })
            .sum::<f64>()
            / (window.len() as f64 - 1.0);
        variance.sqrt()
    })
}

fn decay_linear(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, |window| {
        let weighted = window
            .iter()
            .enumerate()
            .map(|(idx, value)| (idx as f64 + 1.0) * value)
            .sum::<f64>();
        weighted / (window.len() * (window.len() + 1) / 2) as f64
    })
}

fn correlation(lhs: &[f64], rhs: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window2(lhs, rhs, days, cs, correlation_window)
}

fn covariance(lhs: &[f64], rhs: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window2(lhs, rhs, days, cs, covariance_window)
}

fn rank(values: &[f64], cs: &CellSet) -> Vec<f64> {
    xs_per_block(values, cs, rank_pairs)
}

fn scale(values: &[f64], scale_to: f64, cs: &CellSet) -> Vec<f64> {
    xs_per_block(values, cs, |present| {
        let denom = present.iter().map(|(_, value)| value.abs()).sum::<f64>();
        if denom == 0.0 {
            return Vec::new();
        }
        present
            .iter()
            .map(|(idx, value)| (*idx, value * scale_to / denom))
            .collect()
    })
}

fn group_rank(values: &[f64], groups: &[f64], cs: &CellSet) -> Vec<f64> {
    xs_per_group(values, groups, cs, rank_pairs)
}

fn group_neutralize(values: &[f64], groups: &[f64], cs: &CellSet) -> Vec<f64> {
    xs_per_group(values, groups, cs, |present| {
        let mean = present.iter().map(|(_, value)| value).sum::<f64>() / present.len() as f64;
        present
            .iter()
            .map(|(idx, value)| (*idx, value - mean))
            .collect()
    })
}

fn rank_pairs(present: &[(usize, f64)]) -> Vec<(usize, f64)> {
    let mut present = present.to_vec();
    present.sort_by(|(_, lhs), (_, rhs)| lhs.partial_cmp(rhs).unwrap_or(Ordering::Equal));
    let count = present.len() as f64;
    let mut out = Vec::with_capacity(present.len());
    let mut start = 0usize;
    while start < present.len() {
        let mut end = start + 1;
        while end < present.len() && present[end].1 == present[start].1 {
            end += 1;
        }

        let rank_avg = (start + 1 + end) as f64 / 2.0;
        let pct = rank_avg / count;
        for (idx, _) in &present[start..end] {
            out.push((*idx, pct));
        }
        start = end;
    }
    out
}

fn rank_last(window: &[f64]) -> f64 {
    let target = window[window.len() - 1];
    let mut sorted = window.to_vec();
    sorted.sort_by(|lhs, rhs| lhs.partial_cmp(rhs).unwrap_or(Ordering::Equal));
    let start = sorted
        .iter()
        .position(|value| *value == target)
        .expect("target is in the sorted window");
    let end = sorted
        .iter()
        .rposition(|value| *value == target)
        .expect("target is in the sorted window")
        + 1;
    (start + 1 + end) as f64 / 2.0 / sorted.len() as f64
}

fn covariance_window(lhs: &[f64], rhs: &[f64]) -> f64 {
    if lhs.len() < 2 {
        return f64::NAN;
    }
    let lhs_mean = lhs.iter().sum::<f64>() / lhs.len() as f64;
    let rhs_mean = rhs.iter().sum::<f64>() / rhs.len() as f64;
    lhs.iter()
        .zip(rhs)
        .map(|(lhs, rhs)| (lhs - lhs_mean) * (rhs - rhs_mean))
        .sum::<f64>()
        / (lhs.len() as f64 - 1.0)
}

fn correlation_window(lhs: &[f64], rhs: &[f64]) -> f64 {
    if lhs.len() < 2 {
        return f64::NAN;
    }
    let lhs_mean = lhs.iter().sum::<f64>() / lhs.len() as f64;
    let rhs_mean = rhs.iter().sum::<f64>() / rhs.len() as f64;
    let mut covariance = 0.0;
    let mut lhs_variance = 0.0;
    let mut rhs_variance = 0.0;

    for (lhs, rhs) in lhs.iter().zip(rhs) {
        let lhs_centered = lhs - lhs_mean;
        let rhs_centered = rhs - rhs_mean;
        covariance += lhs_centered * rhs_centered;
        lhs_variance += lhs_centered * lhs_centered;
        rhs_variance += rhs_centered * rhs_centered;
    }

    if lhs_variance == 0.0 || rhs_variance == 0.0 {
        f64::NAN
    } else {
        covariance / (lhs_variance.sqrt() * rhs_variance.sqrt())
    }
}

fn log_value(value: f64) -> f64 {
    if value > 0.0 { value.ln() } else { f64::NAN }
}

fn sign(value: f64) -> f64 {
    if value.is_nan() {
        f64::NAN
    } else if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn signed_power(value: f64, exponent: f64) -> f64 {
    if value.is_nan() {
        f64::NAN
    } else {
        sign(value) * value.abs().powf(exponent)
    }
}

fn min_value(lhs: f64, rhs: f64) -> f64 {
    if lhs.is_nan() || rhs.is_nan() {
        f64::NAN
    } else {
        lhs.min(rhs)
    }
}

fn max_value(lhs: f64, rhs: f64) -> f64 {
    if lhs.is_nan() || rhs.is_nan() {
        f64::NAN
    } else {
        lhs.max(rhs)
    }
}

fn cmp_value(op: CmpOp, lhs: f64, rhs: f64) -> f64 {
    if lhs.is_nan() || rhs.is_nan() {
        return f64::NAN;
    }
    let is_true = match op {
        CmpOp::Lt => lhs < rhs,
        CmpOp::Gt => lhs > rhs,
        CmpOp::Le => lhs <= rhs,
        CmpOp::Ge => lhs >= rhs,
        CmpOp::Eq => lhs == rhs,
    };
    if is_true { 1.0 } else { 0.0 }
}

fn where_value(cond: f64, when_true: f64, when_false: f64) -> f64 {
    if cond.is_nan() {
        f64::NAN
    } else if cond > 0.0 {
        when_true
    } else {
        when_false
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::ops::Range;

    use polars::prelude::*;

    use super::*;

    fn test_cellset(
        values: Vec<f64>,
        sym_blocks: Vec<Range<usize>>,
        time_blocks: Vec<Range<usize>>,
    ) -> CellSet {
        test_cellset_fields(
            HashMap::from([("x".to_string(), values)]),
            sym_blocks,
            time_blocks,
        )
    }

    fn test_cellset_fields(
        fields: HashMap<String, Vec<f64>>,
        sym_blocks: Vec<Range<usize>>,
        time_blocks: Vec<Range<usize>>,
    ) -> CellSet {
        let n_cells = fields.values().next().map_or(0, Vec::len);
        CellSet {
            n_cells,
            sym_blocks,
            time_blocks,
            tn_order: (0..n_cells).collect(),
            fields,
            symbols_tn: Column::new("asset".into(), vec!["A"; n_cells]),
            times_tn: Column::new("time".into(), (0..n_cells as i64).collect::<Vec<_>>()),
            time_block_by_value: HashMap::new(),
        }
    }

    fn cells(value: Val, cs: &CellSet) -> Vec<f64> {
        to_cells(value, Layout::Nt, cs)
    }

    fn one_block(range: Range<usize>) -> Vec<Range<usize>> {
        std::iter::once(range).collect()
    }

    fn assert_f64_eq(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-12,
            "actual {actual}, expected {expected}"
        );
    }

    fn assert_vec_close(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (idx, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            if expected.is_nan() {
                assert!(actual.is_nan(), "idx {idx}: actual {actual}, expected NaN");
            } else {
                assert!(
                    (actual - expected).abs() < 1e-12,
                    "idx {idx}: actual {actual}, expected {expected}"
                );
            }
        }
    }

    #[test]
    fn delay_shifts_with_nan_prefix() -> Result<()> {
        let cs = test_cellset(
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
            one_block(0..5),
            vec![0..1, 1..2, 2..3, 3..4, 4..5],
        );

        let out = cells(
            eval(&Expr::Delay(Box::new(Expr::Field("x".to_string())), 2), &cs)?,
            &cs,
        );

        assert!(out[0].is_nan());
        assert!(out[1].is_nan());
        assert_eq!(&out[2..], &[1.0, 2.0, 3.0]);
        Ok(())
    }

    #[test]
    fn ts_sum_requires_full_non_nan_window() -> Result<()> {
        let cs = test_cellset(
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
            one_block(0..5),
            vec![0..1, 1..2, 2..3, 3..4, 4..5],
        );

        let out = cells(
            eval(&Expr::TsSum(Box::new(Expr::Field("x".to_string())), 3), &cs)?,
            &cs,
        );

        assert!(out[0].is_nan());
        assert!(out[1].is_nan());
        assert_eq!(&out[2..], &[6.0, 9.0, 12.0]);

        let nan_cs = test_cellset(
            vec![1.0, 2.0, f64::NAN, 4.0, 5.0],
            one_block(0..5),
            vec![0..1, 1..2, 2..3, 3..4, 4..5],
        );
        let nan_out = cells(
            eval(
                &Expr::TsSum(Box::new(Expr::Field("x".to_string())), 3),
                &nan_cs,
            )?,
            &nan_cs,
        );
        assert!(nan_out.iter().all(|value| value.is_nan()));
        Ok(())
    }

    #[test]
    fn single_input_ts_operators_match_hand_examples() -> Result<()> {
        let cs = test_cellset(
            vec![3.0, 1.0, 2.0, 2.0, 5.0],
            one_block(0..5),
            vec![0..1, 1..2, 2..3, 3..4, 4..5],
        );

        assert_vec_close(
            &cells(
                eval(&Expr::Delta(Box::new(Expr::Field("x".to_string())), 2), &cs)?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, -1.0, 1.0, 3.0],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::TsMean(Box::new(Expr::Field("x".to_string())), 3),
                    &cs,
                )?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, 2.0, 5.0 / 3.0, 3.0],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Product(Box::new(Expr::Field("x".to_string())), 3),
                    &cs,
                )?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, 6.0, 4.0, 20.0],
        );
        assert_vec_close(
            &cells(
                eval(&Expr::TsMin(Box::new(Expr::Field("x".to_string())), 3), &cs)?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, 1.0, 1.0, 2.0],
        );
        assert_vec_close(
            &cells(
                eval(&Expr::TsMax(Box::new(Expr::Field("x".to_string())), 3), &cs)?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, 3.0, 2.0, 5.0],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::TsArgMin(Box::new(Expr::Field("x".to_string())), 3),
                    &cs,
                )?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, 1.0, 2.0, 1.0],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::TsArgMax(Box::new(Expr::Field("x".to_string())), 3),
                    &cs,
                )?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, 2.0, 0.0, 0.0],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::TsRank(Box::new(Expr::Field("x".to_string())), 3),
                    &cs,
                )?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, 2.0 / 3.0, 5.0 / 6.0, 1.0],
        );
        assert_vec_close(
            &cells(
                eval(&Expr::TsStd(Box::new(Expr::Field("x".to_string())), 3), &cs)?,
                &cs,
            ),
            &[
                f64::NAN,
                f64::NAN,
                1.0,
                1.0 / 3.0_f64.sqrt(),
                3.0_f64.sqrt(),
            ],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::DecayLinear(Box::new(Expr::Field("x".to_string())), 3),
                    &cs,
                )?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, 11.0 / 6.0, 11.0 / 6.0, 21.0 / 6.0],
        );

        let nan_cs = test_cellset(
            vec![1.0, f64::NAN, 3.0],
            one_block(0..3),
            vec![0..1, 1..2, 2..3],
        );
        let nan_out = cells(
            eval(
                &Expr::TsMean(Box::new(Expr::Field("x".to_string())), 2),
                &nan_cs,
            )?,
            &nan_cs,
        );
        assert!(nan_out.iter().all(|value| value.is_nan()));
        Ok(())
    }

    #[test]
    fn two_input_ts_operators_are_strict_and_handle_degenerate_windows() -> Result<()> {
        let cs = test_cellset_fields(
            HashMap::from([
                ("x".to_string(), vec![1.0, 2.0, 3.0, 4.0]),
                ("y".to_string(), vec![1.0, 3.0, 5.0, 7.0]),
                ("flat".to_string(), vec![2.0, 2.0, 2.0, 2.0]),
                ("nan".to_string(), vec![1.0, f64::NAN, 3.0, 4.0]),
            ]),
            one_block(0..4),
            vec![0..1, 1..2, 2..3, 3..4],
        );

        let corr = cells(
            eval(
                &Expr::Correlation(
                    Box::new(Expr::Field("x".to_string())),
                    Box::new(Expr::Field("y".to_string())),
                    3,
                ),
                &cs,
            )?,
            &cs,
        );
        assert!(corr[0].is_nan());
        assert!(corr[1].is_nan());
        assert_f64_eq(corr[2], 1.0);
        assert_f64_eq(corr[3], 1.0);

        let cov = cells(
            eval(
                &Expr::Covariance(
                    Box::new(Expr::Field("x".to_string())),
                    Box::new(Expr::Field("y".to_string())),
                    3,
                ),
                &cs,
            )?,
            &cs,
        );
        assert_f64_eq(cov[2], 2.0);
        assert_f64_eq(cov[3], 2.0);

        let flat_corr = cells(
            eval(
                &Expr::Correlation(
                    Box::new(Expr::Field("flat".to_string())),
                    Box::new(Expr::Field("y".to_string())),
                    3,
                ),
                &cs,
            )?,
            &cs,
        );
        assert!(flat_corr[2].is_nan());

        let nan_cov = cells(
            eval(
                &Expr::Covariance(
                    Box::new(Expr::Field("nan".to_string())),
                    Box::new(Expr::Field("y".to_string())),
                    3,
                ),
                &cs,
            )?,
            &cs,
        );
        assert!(nan_cov.iter().all(|value| value.is_nan()));
        Ok(())
    }

    #[test]
    fn elementwise_comparison_and_where_operators_follow_nan_rules() -> Result<()> {
        let cs = test_cellset_fields(
            HashMap::from([
                ("x".to_string(), vec![-2.0, 0.0, 3.0, f64::NAN]),
                ("y".to_string(), vec![1.0, -1.0, 3.0, 5.0]),
            ]),
            vec![0..1, 1..2, 2..3, 3..4],
            one_block(0..4),
        );

        assert_vec_close(
            &cells(
                eval(&Expr::Abs(Box::new(Expr::Field("x".to_string()))), &cs)?,
                &cs,
            ),
            &[2.0, 0.0, 3.0, f64::NAN],
        );
        assert_vec_close(
            &cells(
                eval(&Expr::Log(Box::new(Expr::Field("x".to_string()))), &cs)?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, 3.0_f64.ln(), f64::NAN],
        );
        assert_vec_close(
            &cells(
                eval(&Expr::Sign(Box::new(Expr::Field("x".to_string()))), &cs)?,
                &cs,
            ),
            &[-1.0, 0.0, 1.0, f64::NAN],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::SignedPower(Box::new(Expr::Field("x".to_string())), 2.0),
                    &cs,
                )?,
                &cs,
            ),
            &[-4.0, 0.0, 9.0, f64::NAN],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Power(Box::new(Expr::Field("x".to_string())), 0.5),
                    &cs,
                )?,
                &cs,
            ),
            &[f64::NAN, 0.0, 3.0_f64.sqrt(), f64::NAN],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Min(
                        Box::new(Expr::Field("x".to_string())),
                        Box::new(Expr::Field("y".to_string())),
                    ),
                    &cs,
                )?,
                &cs,
            ),
            &[-2.0, -1.0, 3.0, f64::NAN],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Max(
                        Box::new(Expr::Field("x".to_string())),
                        Box::new(Expr::Field("y".to_string())),
                    ),
                    &cs,
                )?,
                &cs,
            ),
            &[1.0, 0.0, 3.0, f64::NAN],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Cmp(
                        CmpOp::Lt,
                        Box::new(Expr::Field("x".to_string())),
                        Box::new(Expr::Field("y".to_string())),
                    ),
                    &cs,
                )?,
                &cs,
            ),
            &[1.0, 0.0, 0.0, f64::NAN],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Cmp(
                        CmpOp::Ge,
                        Box::new(Expr::Field("x".to_string())),
                        Box::new(Expr::Field("y".to_string())),
                    ),
                    &cs,
                )?,
                &cs,
            ),
            &[0.0, 1.0, 1.0, f64::NAN],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Cmp(
                        CmpOp::Gt,
                        Box::new(Expr::Field("x".to_string())),
                        Box::new(Expr::Field("y".to_string())),
                    ),
                    &cs,
                )?,
                &cs,
            ),
            &[0.0, 1.0, 0.0, f64::NAN],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Cmp(
                        CmpOp::Le,
                        Box::new(Expr::Field("x".to_string())),
                        Box::new(Expr::Field("y".to_string())),
                    ),
                    &cs,
                )?,
                &cs,
            ),
            &[1.0, 0.0, 1.0, f64::NAN],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Cmp(
                        CmpOp::Eq,
                        Box::new(Expr::Field("x".to_string())),
                        Box::new(Expr::Field("y".to_string())),
                    ),
                    &cs,
                )?,
                &cs,
            ),
            &[0.0, 0.0, 1.0, f64::NAN],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Where(
                        Box::new(Expr::Field("x".to_string())),
                        Box::new(Expr::Const(10.0)),
                        Box::new(Expr::Field("y".to_string())),
                    ),
                    &cs,
                )?,
                &cs,
            ),
            &[1.0, -1.0, 10.0, f64::NAN],
        );
        Ok(())
    }

    #[test]
    fn scale_and_group_operators_work_per_time_block() -> Result<()> {
        let cs = test_cellset_fields(
            HashMap::from([
                ("x".to_string(), vec![5.0, 2.0, 2.0, 9.0, f64::NAN, 1.0]),
                ("g".to_string(), vec![1.0, 1.0, 1.0, 2.0, 1.0, f64::NAN]),
            ]),
            vec![0..1, 1..2, 2..3, 3..4, 4..5, 5..6],
            one_block(0..6),
        );

        let scaled = to_cells(
            eval(
                &Expr::Scale(Box::new(Expr::Field("x".to_string())), 1.0),
                &cs,
            )?,
            Layout::Tn,
            &cs,
        );
        assert_vec_close(
            &scaled,
            &[
                5.0 / 19.0,
                2.0 / 19.0,
                2.0 / 19.0,
                9.0 / 19.0,
                f64::NAN,
                1.0 / 19.0,
            ],
        );

        let ranked = to_cells(
            eval(
                &Expr::GroupRank(
                    Box::new(Expr::Field("x".to_string())),
                    Box::new(Expr::Field("g".to_string())),
                ),
                &cs,
            )?,
            Layout::Tn,
            &cs,
        );
        assert_vec_close(&ranked, &[1.0, 0.5, 0.5, 1.0, f64::NAN, f64::NAN]);

        let neutralized = to_cells(
            eval(
                &Expr::GroupNeutralize(
                    Box::new(Expr::Field("x".to_string())),
                    Box::new(Expr::Field("g".to_string())),
                ),
                &cs,
            )?,
            Layout::Tn,
            &cs,
        );
        assert_vec_close(&neutralized, &[2.0, -1.0, -1.0, 0.0, f64::NAN, f64::NAN]);

        let zero_cs = test_cellset(vec![0.0, 0.0], vec![0..1, 1..2], one_block(0..2));
        let zero_scaled = to_cells(
            eval(
                &Expr::Scale(Box::new(Expr::Field("x".to_string())), 1.0),
                &zero_cs,
            )?,
            Layout::Tn,
            &zero_cs,
        );
        assert!(zero_scaled.iter().all(|value| value.is_nan()));
        Ok(())
    }

    #[test]
    fn rank_averages_ties_and_skips_nan() -> Result<()> {
        let tie_cs = test_cellset(
            vec![10.0, 20.0, 20.0, 30.0],
            vec![0..1, 1..2, 2..3, 3..4],
            one_block(0..4),
        );
        let tie_out = to_cells(
            eval(&Expr::Rank(Box::new(Expr::Field("x".to_string()))), &tie_cs)?,
            Layout::Tn,
            &tie_cs,
        );
        assert_eq!(tie_out, [0.25, 0.625, 0.625, 1.0]);

        let nan_cs = test_cellset(
            vec![10.0, f64::NAN, 20.0, 30.0],
            vec![0..1, 1..2, 2..3, 3..4],
            one_block(0..4),
        );
        let nan_out = to_cells(
            eval(&Expr::Rank(Box::new(Expr::Field("x".to_string()))), &nan_cs)?,
            Layout::Tn,
            &nan_cs,
        );
        assert_eq!(nan_out[0], 1.0 / 3.0);
        assert!(nan_out[1].is_nan());
        assert_eq!(nan_out[2], 2.0 / 3.0);
        assert_eq!(nan_out[3], 1.0);
        Ok(())
    }

    #[test]
    fn binary_ops_preserve_scalar_when_possible() -> Result<()> {
        let cs = test_cellset(Vec::new(), Vec::new(), Vec::new());
        let out = eval(
            &Expr::Sub(Box::new(Expr::Const(3.0)), Box::new(Expr::Const(1.5))),
            &cs,
        )?;

        assert_eq!(out, Val::Scalar(1.5));
        Ok(())
    }
}

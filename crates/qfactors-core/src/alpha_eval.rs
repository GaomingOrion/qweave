use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};
use std::ops::Range;

use rayon::prelude::*;

use crate::cellset::CellSet;
use crate::error::{QFactorsError, Result};
use crate::expr::{CmpOp, Expr};
use crate::layout::{Layout, nt_to_tn, tn_to_nt};

#[derive(Debug, Clone, PartialEq)]
pub enum Val<'cs> {
    Cells {
        values: Cow<'cs, [f64]>,
        layout: Layout,
    },
    Scalar(f64),
}

fn owned_cells<'cs>(values: Vec<f64>, layout: Layout) -> Val<'cs> {
    Val::Cells {
        values: Cow::Owned(values),
        layout,
    }
}

/// Map an elementwise op over cells, reusing the buffer in place when it is owned
/// (an intermediate temporary) and only allocating when it is a borrowed field.
fn map_cells<'cs>(values: Cow<'cs, [f64]>, layout: Layout, op: impl Fn(f64) -> f64) -> Val<'cs> {
    let values = match values {
        Cow::Owned(mut values) => {
            values.iter_mut().for_each(|value| *value = op(*value));
            values
        }
        Cow::Borrowed(values) => values.iter().map(|value| op(*value)).collect(),
    };
    owned_cells(values, layout)
}

pub fn eval<'cs>(expr: &Expr, cs: &'cs CellSet) -> Result<Val<'cs>> {
    match expr {
        Expr::Field(name) => cs
            .fields
            .get(name)
            .map(|values| Val::Cells {
                values: Cow::Borrowed(values.as_ref().as_slice()),
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
            Val::Cells { values, layout } => Ok(map_cells(values, layout, |value| -value)),
        },
        Expr::Delay(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(delay(&values, *days, cs), Layout::Nt))
        }
        Expr::Delta(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(delta(&values, *days, cs), Layout::Nt))
        }
        Expr::TsSum(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(ts_sum(&values, *days, cs), Layout::Nt))
        }
        Expr::TsMean(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(ts_mean(&values, *days, cs), Layout::Nt))
        }
        Expr::Product(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(product(&values, *days, cs), Layout::Nt))
        }
        Expr::TsMin(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(ts_min(&values, *days, cs), Layout::Nt))
        }
        Expr::TsMax(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(ts_max(&values, *days, cs), Layout::Nt))
        }
        Expr::TsArgMin(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(ts_argmin(&values, *days, cs), Layout::Nt))
        }
        Expr::TsArgMax(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(ts_argmax(&values, *days, cs), Layout::Nt))
        }
        Expr::TsRank(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(ts_rank(&values, *days, cs), Layout::Nt))
        }
        Expr::TsRankRaw(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(ts_rank_raw(&values, *days, cs), Layout::Nt))
        }
        Expr::TsStd(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(ts_std(&values, *days, cs), Layout::Nt))
        }
        Expr::Slope(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(slope(&values, *days, cs), Layout::Nt))
        }
        Expr::Rsquare(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(rsquare(&values, *days, cs), Layout::Nt))
        }
        Expr::Resi(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(resi(&values, *days, cs), Layout::Nt))
        }
        Expr::Quantile(inner, days, q) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(quantile(&values, *days, *q, cs), Layout::Nt))
        }
        Expr::DecayLinear(inner, days) => {
            let values = to_cells(eval(inner, cs)?, Layout::Nt, cs);
            Ok(owned_cells(decay_linear(&values, *days, cs), Layout::Nt))
        }
        Expr::Correlation(lhs, rhs, days) => {
            let lhs = to_cells(eval(lhs, cs)?, Layout::Nt, cs);
            let rhs = to_cells(eval(rhs, cs)?, Layout::Nt, cs);
            Ok(owned_cells(correlation(&lhs, &rhs, *days, cs), Layout::Nt))
        }
        Expr::Covariance(lhs, rhs, days) => {
            let lhs = to_cells(eval(lhs, cs)?, Layout::Nt, cs);
            let rhs = to_cells(eval(rhs, cs)?, Layout::Nt, cs);
            Ok(owned_cells(covariance(&lhs, &rhs, *days, cs), Layout::Nt))
        }
        Expr::Rank(inner) => {
            let values = to_cells(eval(inner, cs)?, Layout::Tn, cs);
            Ok(owned_cells(
                rank(&values, Layout::Tn, Layout::Tn, cs),
                Layout::Tn,
            ))
        }
        Expr::Scale(inner, scale_to) => {
            let values = to_cells(eval(inner, cs)?, Layout::Tn, cs);
            Ok(owned_cells(
                scale(&values, Layout::Tn, Layout::Tn, *scale_to, cs),
                Layout::Tn,
            ))
        }
        Expr::GroupRank(values, groups) => {
            let values = to_cells(eval(values, cs)?, Layout::Tn, cs);
            let groups = to_cells(eval(groups, cs)?, Layout::Tn, cs);
            Ok(owned_cells(
                group_rank(&values, Layout::Tn, &groups, Layout::Tn, Layout::Tn, cs),
                Layout::Tn,
            ))
        }
        Expr::GroupNeutralize(values, groups) => {
            let values = to_cells(eval(values, cs)?, Layout::Tn, cs);
            let groups = to_cells(eval(groups, cs)?, Layout::Tn, cs);
            Ok(owned_cells(
                group_neutralize(&values, Layout::Tn, &groups, Layout::Tn, Layout::Tn, cs),
                Layout::Tn,
            ))
        }
        Expr::Abs(inner) => eval_unary(inner, cs, f64::abs),
        Expr::Log(inner) => eval_unary(inner, cs, log_value),
        Expr::Sign(inner) => eval_unary(inner, cs, sign),
        Expr::SignedPower(lhs, rhs) => eval_binary(lhs, rhs, cs, signed_power),
        Expr::Power(lhs, rhs) => eval_binary(lhs, rhs, cs, |value, exponent| value.powf(exponent)),
        Expr::Min(lhs, rhs) => eval_binary(lhs, rhs, cs, min_value),
        Expr::Max(lhs, rhs) => eval_binary(lhs, rhs, cs, max_value),
        Expr::Cmp(op, lhs, rhs) => eval_binary(lhs, rhs, cs, |lhs, rhs| cmp_value(*op, lhs, rhs)),
        Expr::Where(cond, when_true, when_false) => eval_where(cond, when_true, when_false, cs),
    }
}

pub fn to_cells<'cs>(value: Val<'cs>, want: Layout, cs: &CellSet) -> Cow<'cs, [f64]> {
    match value {
        Val::Scalar(value) => Cow::Owned(vec![value; cs.n_cells]),
        Val::Cells { values, layout } if layout == want => values,
        Val::Cells {
            values,
            layout: Layout::Nt,
        } => Cow::Owned(nt_to_tn(&values, cs)),
        Val::Cells {
            values,
            layout: Layout::Tn,
        } => Cow::Owned(tn_to_nt(&values, cs)),
    }
}

fn eval_binary<'cs>(
    lhs: &Expr,
    rhs: &Expr,
    cs: &'cs CellSet,
    op: impl Fn(f64, f64) -> f64,
) -> Result<Val<'cs>> {
    let lhs = eval(lhs, cs)?;
    let rhs = eval(rhs, cs)?;
    match (lhs, rhs) {
        (Val::Scalar(lhs), Val::Scalar(rhs)) => Ok(Val::Scalar(op(lhs, rhs))),
        (Val::Cells { values, layout }, Val::Scalar(rhs)) => {
            Ok(map_cells(values, layout, |lhs| op(lhs, rhs)))
        }
        (Val::Scalar(lhs), Val::Cells { values, layout }) => {
            Ok(map_cells(values, layout, |rhs| op(lhs, rhs)))
        }
        (
            Val::Cells {
                values: lhs,
                layout,
            },
            Val::Cells {
                values: rhs,
                layout: rhs_layout,
            },
        ) => {
            let rhs = if rhs_layout == layout {
                rhs
            } else {
                to_cells(
                    Val::Cells {
                        values: rhs,
                        layout: rhs_layout,
                    },
                    layout,
                    cs,
                )
            };
            Ok(owned_cells(
                lhs.iter()
                    .zip(rhs.iter())
                    .map(|(lhs, rhs)| op(*lhs, *rhs))
                    .collect(),
                layout,
            ))
        }
    }
}

fn eval_unary<'cs>(inner: &Expr, cs: &'cs CellSet, op: impl Fn(f64) -> f64) -> Result<Val<'cs>> {
    match eval(inner, cs)? {
        Val::Scalar(value) => Ok(Val::Scalar(op(value))),
        Val::Cells { values, layout } => Ok(map_cells(values, layout, op)),
    }
}

fn eval_where<'cs>(
    cond: &Expr,
    when_true: &Expr,
    when_false: &Expr,
    cs: &'cs CellSet,
) -> Result<Val<'cs>> {
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

    let align = |value| match value {
        Val::Scalar(value) => Val::Scalar(value),
        Val::Cells {
            values,
            layout: current,
        } if current == layout => Val::Cells { values, layout },
        Val::Cells {
            values,
            layout: current,
        } => Val::Cells {
            values: to_cells(
                Val::Cells {
                    values,
                    layout: current,
                },
                layout,
                cs,
            ),
            layout,
        },
    };
    let cond = align(cond);
    let when_true = align(when_true);
    let when_false = align(when_false);
    let value_at = |value: &Val, idx: usize| match value {
        Val::Scalar(value) => *value,
        Val::Cells { values, .. } => values[idx],
    };
    Ok(owned_cells(
        (0..cs.n_cells)
            .map(|idx| {
                where_value(
                    value_at(&cond, idx),
                    value_at(&when_true, idx),
                    value_at(&when_false, idx),
                )
            })
            .collect(),
        layout,
    ))
}

pub(crate) fn delay(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    par_fill_blocks(values.len(), &cs.sym_blocks, |block, out_block| {
        for (local_idx, out_cell) in out_block.iter_mut().enumerate().skip(days) {
            *out_cell = values[block.start + local_idx - days];
        }
    })
}

/// Fill an `n_cells`-long output one block at a time, in parallel. `blocks` must
/// partition `0..n_cells` contiguously and in order (which both `sym_blocks` and
/// `time_blocks` do), so each block owns a disjoint output sub-slice and the
/// blocks can run concurrently without locking. `fill(block, out_block)` reads
/// `values` by global index but writes its block-local slice (`out_block[i]` is
/// global index `block.start + i`).
///
/// Nested inside the alpha-DAG's per-level `par_iter`, rayon's work-stealing
/// only spreads a single kernel across cores when the surrounding level is too
/// narrow (or too skewed) to keep them busy — so a lone heavy node (a wide
/// correlation/rolling window) no longer serializes its level.
fn par_fill_blocks(
    n_cells: usize,
    blocks: &[Range<usize>],
    fill: impl Fn(&Range<usize>, &mut [f64]) + Sync,
) -> Vec<f64> {
    let mut out = vec![f64::NAN; n_cells];
    let mut out_blocks: Vec<&mut [f64]> = Vec::with_capacity(blocks.len());
    let mut rest = out.as_mut_slice();
    for block in blocks {
        let (head, tail) = rest.split_at_mut(block.len());
        out_blocks.push(head);
        rest = tail;
    }
    blocks
        .par_iter()
        .zip(out_blocks)
        .for_each(|(block, out_block)| fill(block, out_block));
    out
}

fn ts_window(
    values: &[f64],
    days: usize,
    cs: &CellSet,
    reduce: impl Fn(&[f64]) -> f64 + Sync,
) -> Vec<f64> {
    if days == 0 {
        return vec![f64::NAN; values.len()];
    }

    par_fill_blocks(values.len(), &cs.sym_blocks, |block, out_block| {
        let mut nan_count = 0usize;
        for (local_idx, out_cell) in out_block.iter_mut().enumerate() {
            let idx = block.start + local_idx;
            if values[idx].is_nan() {
                nan_count += 1;
            }

            if local_idx >= days {
                let old_idx = block.start + local_idx - days;
                if values[old_idx].is_nan() {
                    nan_count -= 1;
                }
            }

            if local_idx + 1 >= days && nan_count == 0 {
                let start = block.start + local_idx + 1 - days;
                let window = &values[start..=idx];
                *out_cell = reduce(window);
            }
        }
    })
}

fn ts_window2(
    lhs: &[f64],
    rhs: &[f64],
    days: usize,
    cs: &CellSet,
    reduce: impl Fn(&[f64], &[f64]) -> f64 + Sync,
) -> Vec<f64> {
    if days == 0 {
        return vec![f64::NAN; lhs.len()];
    }

    par_fill_blocks(lhs.len(), &cs.sym_blocks, |block, out_block| {
        let mut nan_count = 0usize;
        for (local_idx, out_cell) in out_block.iter_mut().enumerate() {
            let idx = block.start + local_idx;
            if lhs[idx].is_nan() {
                nan_count += 1;
            }
            if rhs[idx].is_nan() {
                nan_count += 1;
            }

            if local_idx >= days {
                let old_idx = block.start + local_idx - days;
                if lhs[old_idx].is_nan() {
                    nan_count -= 1;
                }
                if rhs[old_idx].is_nan() {
                    nan_count -= 1;
                }
            }

            if local_idx + 1 >= days && nan_count == 0 {
                let start = block.start + local_idx + 1 - days;
                let lhs_window = &lhs[start..=idx];
                let rhs_window = &rhs[start..=idx];
                *out_cell = reduce(lhs_window, rhs_window);
            }
        }
    })
}

trait RollingValue {
    fn push(&mut self, x: f64, days: usize);
    fn pop(&mut self, x: f64);
    fn value(&self, days: usize) -> f64;
}

fn ts_rolling_value<R: RollingValue + Default>(
    values: &[f64],
    days: usize,
    cs: &CellSet,
    fallback: impl Fn(&[f64]) -> f64 + Sync,
) -> Vec<f64> {
    if days == 0 {
        return vec![f64::NAN; values.len()];
    }

    par_fill_blocks(values.len(), &cs.sym_blocks, |block, out_block| {
        let mut reducer = R::default();
        let mut nan_count = 0usize;
        let mut inf_count = 0usize;
        for (local_idx, out_cell) in out_block.iter_mut().enumerate() {
            let idx = block.start + local_idx;
            let value = values[idx];
            if value.is_nan() {
                nan_count += 1;
            } else if value.is_infinite() {
                inf_count += 1;
            } else {
                reducer.push(value, days);
            }

            if local_idx >= days {
                let old_idx = block.start + local_idx - days;
                let old_value = values[old_idx];
                if old_value.is_nan() {
                    nan_count -= 1;
                } else if old_value.is_infinite() {
                    inf_count -= 1;
                } else {
                    reducer.pop(old_value);
                }
            }

            if local_idx + 1 >= days && nan_count == 0 {
                if inf_count == 0 {
                    *out_cell = reducer.value(days);
                } else {
                    let start = block.start + local_idx + 1 - days;
                    *out_cell = fallback(&values[start..=idx]);
                }
            }
        }
    })
}

#[derive(Default)]
struct RollingSum {
    sum: f64,
}

impl RollingValue for RollingSum {
    fn push(&mut self, x: f64, _days: usize) {
        self.sum += x;
    }

    fn pop(&mut self, x: f64) {
        self.sum -= x;
    }

    fn value(&self, _days: usize) -> f64 {
        self.sum
    }
}

#[derive(Default)]
struct RollingMean {
    sum: f64,
}

impl RollingValue for RollingMean {
    fn push(&mut self, x: f64, _days: usize) {
        self.sum += x;
    }

    fn pop(&mut self, x: f64) {
        self.sum -= x;
    }

    fn value(&self, days: usize) -> f64 {
        self.sum / days as f64
    }
}

#[derive(Default)]
struct RollingVar {
    n: usize,
    mean: f64,
    m2: f64,
}

impl RollingValue for RollingVar {
    fn push(&mut self, x: f64, _days: usize) {
        self.n += 1;
        let delta = x - self.mean;
        self.mean += delta / self.n as f64;
        self.m2 += delta * (x - self.mean);
    }

    fn pop(&mut self, x: f64) {
        if self.n <= 1 {
            self.n = 0;
            self.mean = 0.0;
            self.m2 = 0.0;
            return;
        }

        let old_n = self.n as f64;
        self.n -= 1;
        let new_n = self.n as f64;
        let old_mean = self.mean;
        self.mean = (old_n * old_mean - x) / new_n;
        self.m2 -= (x - self.mean) * (x - old_mean);
    }

    fn value(&self, _days: usize) -> f64 {
        if self.n < 2 {
            return f64::NAN;
        }
        let variance = self.m2 / (self.n as f64 - 1.0);
        variance.max(0.0).sqrt()
    }
}

#[derive(Default)]
struct RollingDecay {
    sum: f64,
    weighted_sum: f64,
    pushed: usize,
}

impl RollingDecay {
    fn push(&mut self, x: f64, days: usize) {
        self.pushed += 1;
        if self.pushed <= days {
            self.weighted_sum += self.pushed as f64 * x;
        } else {
            self.weighted_sum += days as f64 * x - self.sum;
        }
        self.sum += x;
    }

    fn pop(&mut self, x: f64) {
        self.sum -= x;
    }

    fn value(&self, days: usize) -> f64 {
        self.weighted_sum / (days * (days + 1) / 2) as f64
    }
}

fn ts_rolling_decay(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    if days == 0 {
        return vec![f64::NAN; values.len()];
    }

    par_fill_blocks(values.len(), &cs.sym_blocks, |block, out_block| {
        let mut reducer = RollingDecay::default();
        let mut nan_count = 0usize;
        let mut inf_count = 0usize;
        for (local_idx, out_cell) in out_block.iter_mut().enumerate() {
            let idx = block.start + local_idx;
            let value = values[idx];
            if value.is_nan() {
                nan_count += 1;
                reducer.push(0.0, days);
            } else if value.is_infinite() {
                inf_count += 1;
                reducer.push(0.0, days);
            } else {
                reducer.push(value, days);
            }

            if local_idx >= days {
                let old_idx = block.start + local_idx - days;
                let old_value = values[old_idx];
                if old_value.is_nan() {
                    nan_count -= 1;
                    reducer.pop(0.0);
                } else if old_value.is_infinite() {
                    inf_count -= 1;
                    reducer.pop(0.0);
                } else {
                    reducer.pop(old_value);
                }
            }

            if local_idx + 1 >= days && nan_count == 0 {
                if inf_count == 0 {
                    *out_cell = reducer.value(days);
                } else {
                    let start = block.start + local_idx + 1 - days;
                    *out_cell = decay_linear_window(&values[start..=idx]);
                }
            }
        }
    })
}

fn ts_deque_window(
    values: &[f64],
    days: usize,
    cs: &CellSet,
    should_pop_back: impl Fn(f64, f64) -> bool + Sync,
) -> Vec<f64> {
    if days == 0 {
        return vec![f64::NAN; values.len()];
    }

    par_fill_blocks(values.len(), &cs.sym_blocks, |block, out_block| {
        let mut deque = VecDeque::new();
        let mut nan_count = 0usize;
        for (local_idx, out_cell) in out_block.iter_mut().enumerate() {
            let idx = block.start + local_idx;
            let value = values[idx];
            if value.is_nan() {
                nan_count += 1;
            } else {
                while let Some(&back_idx) = deque.back() {
                    if should_pop_back(values[back_idx], value) {
                        deque.pop_back();
                    } else {
                        break;
                    }
                }
                deque.push_back(idx);
            }

            if local_idx >= days {
                let old_idx = block.start + local_idx - days;
                if values[old_idx].is_nan() {
                    nan_count -= 1;
                }
            }

            if local_idx + 1 >= days {
                let window_start = block.start + local_idx + 1 - days;
                while deque
                    .front()
                    .is_some_and(|front_idx| *front_idx < window_start)
                {
                    deque.pop_front();
                }

                if nan_count == 0 {
                    let best_idx = *deque.front().expect("full non-NaN window has a value");
                    *out_cell = values[best_idx];
                }
            }
        }
    })
}

/// Read the cell at Tn position `tn_idx` from a buffer in either layout: a Tn
/// buffer is contiguous (`values[tn_idx]`), an Nt buffer is gathered through
/// `tn_order`. This lets cross-sectional ops consume an Nt input directly,
/// skipping a materialized transpose, while still grouping by time block.
#[inline]
fn read_cell(values: &[f64], layout: Layout, tn_idx: usize, cs: &CellSet) -> f64 {
    match layout {
        Layout::Tn => values[tn_idx],
        Layout::Nt => values[cs.tn_order[tn_idx]],
    }
}

/// Place each block's `(tn_idx, value)` results into an output buffer in the
/// requested layout. A Tn output is contiguous per block (safe disjoint slices);
/// an Nt output is scattered through `tn_order`, letting a cross-sectional op
/// emit its result straight into Nt without a follow-up transpose.
fn scatter_pairs(
    n_cells: usize,
    output: Layout,
    cs: &CellSet,
    per_block: impl Fn(&Range<usize>) -> Vec<(usize, f64)> + Sync,
) -> Vec<f64> {
    match output {
        Layout::Tn => par_fill_blocks(n_cells, &cs.time_blocks, |block, out_block| {
            for (tn_idx, value) in per_block(block) {
                out_block[tn_idx - block.start] = value;
            }
        }),
        Layout::Nt => {
            let mut out = vec![f64::NAN; n_cells];
            // Threads write disjoint cells (`time_blocks` partition the Tn axis,
            // `tn_order` is a permutation), so the buffer can be scattered into
            // concurrently. The base address is shared as a `usize` because a raw
            // pointer is not `Sync`; `out` is not resized while the scatter runs.
            let base = out.as_mut_ptr() as usize;
            cs.time_blocks.par_iter().for_each(|block| {
                for (tn_idx, value) in per_block(block) {
                    let nt_idx = cs.tn_order[tn_idx];
                    debug_assert!(nt_idx < n_cells);
                    // SAFETY: `nt_idx < n_cells` and is written by exactly one
                    // thread (disjoint blocks, permutation indices), so this is a
                    // race-free write into the live `out` allocation.
                    unsafe { *(base as *mut f64).add(nt_idx) = value };
                }
            });
            out
        }
    }
}

fn xs_per_block(
    values: &[f64],
    input_layout: Layout,
    output: Layout,
    cs: &CellSet,
    f: impl Fn(Vec<(usize, f64)>) -> Vec<(usize, f64)> + Sync,
) -> Vec<f64> {
    scatter_pairs(values.len(), output, cs, |block| {
        let present = block
            .clone()
            .filter_map(|tn_idx| {
                let value = read_cell(values, input_layout, tn_idx, cs);
                (!value.is_nan()).then_some((tn_idx, value))
            })
            .collect::<Vec<_>>();
        f(present)
    })
}

fn xs_per_group(
    values: &[f64],
    values_layout: Layout,
    groups: &[f64],
    groups_layout: Layout,
    output: Layout,
    cs: &CellSet,
    f: impl Fn(Vec<(usize, f64)>) -> Vec<(usize, f64)> + Sync,
) -> Vec<f64> {
    scatter_pairs(values.len(), output, cs, |block| {
        let mut buckets: HashMap<u64, Vec<(usize, f64)>> = HashMap::new();
        for tn_idx in block.clone() {
            let value = read_cell(values, values_layout, tn_idx, cs);
            let group = read_cell(groups, groups_layout, tn_idx, cs);
            if value.is_nan() || group.is_nan() {
                continue;
            }
            buckets
                .entry(group.to_bits())
                .or_default()
                .push((tn_idx, value));
        }

        buckets.into_values().flat_map(|bucket| f(bucket)).collect()
    })
}

pub(crate) fn delta(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    par_fill_blocks(values.len(), &cs.sym_blocks, |block, out_block| {
        for (local_idx, out_cell) in out_block.iter_mut().enumerate().skip(days) {
            let idx = block.start + local_idx;
            let current = values[idx];
            let previous = values[idx - days];
            if !current.is_nan() && !previous.is_nan() {
                *out_cell = current - previous;
            }
        }
    })
}

pub(crate) fn ts_sum(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_rolling_value::<RollingSum>(values, days, cs, |window| window.iter().sum())
}

pub(crate) fn ts_mean(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_rolling_value::<RollingMean>(values, days, cs, |window| {
        window.iter().sum::<f64>() / window.len() as f64
    })
}

pub(crate) fn product(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, |window| window.iter().product())
}

pub(crate) fn ts_min(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_deque_window(values, days, cs, |back, current| back >= current)
}

pub(crate) fn ts_max(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_deque_window(values, days, cs, |back, current| back <= current)
}

pub(crate) fn ts_argmin(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    // DolphinDB `mimin`: 0-based position of the minimum within the window,
    // counted from the oldest day (0) to the current day (days - 1). The
    // earliest occurrence wins on ties (strict `<` keeps the first minimum).
    ts_window(values, days, cs, |window| {
        let mut best_idx = 0usize;
        let mut best_value = window[0];
        for (idx, &value) in window.iter().enumerate().skip(1) {
            if value < best_value {
                best_value = value;
                best_idx = idx;
            }
        }
        best_idx as f64
    })
}

pub(crate) fn ts_argmax(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    // DolphinDB `mimax`: 0-based position of the maximum within the window,
    // counted from the oldest day (0) to the current day (days - 1). The
    // earliest occurrence wins on ties (strict `>` keeps the first maximum).
    ts_window(values, days, cs, |window| {
        let mut best_idx = 0usize;
        let mut best_value = window[0];
        for (idx, &value) in window.iter().enumerate().skip(1) {
            if value > best_value {
                best_value = value;
                best_idx = idx;
            }
        }
        best_idx as f64
    })
}

pub(crate) fn ts_rank(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, rank_last)
}

pub(crate) fn ts_rank_raw(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, rank_last_raw)
}

pub(crate) fn ts_std(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_rolling_value::<RollingVar>(values, days, cs, |window| {
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

pub(crate) fn slope(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, slope_window)
}

pub(crate) fn rsquare(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, rsquare_window)
}

pub(crate) fn resi(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window(values, days, cs, resi_window)
}

pub(crate) fn quantile(values: &[f64], days: usize, q: f64, cs: &CellSet) -> Vec<f64> {
    if days == 0 || !(0.0..=1.0).contains(&q) {
        return vec![f64::NAN; values.len()];
    }

    par_fill_blocks(values.len(), &cs.sym_blocks, |block, out_block| {
        // Incremental order-statistic window: `sorted` keeps the current window's
        // non-NaN values in ascending order. Each step evicts the outgoing value
        // and inserts the incoming one via binary search (O(days) memmove), so the
        // quantile itself is a plain indexed lookup. NaNs never enter `sorted`;
        // they are tracked by `nan_count` and gate the output exactly as in
        // `ts_window`. Evicting before inserting caps `sorted` at `days` entries.
        let mut sorted: Vec<f64> = Vec::with_capacity(days);
        let mut nan_count = 0usize;
        for (local_idx, out_cell) in out_block.iter_mut().enumerate() {
            if local_idx >= days {
                let old = values[block.start + local_idx - days];
                if old.is_nan() {
                    nan_count -= 1;
                } else {
                    let pos = sorted
                        .binary_search_by(|probe| {
                            probe.partial_cmp(&old).unwrap_or(Ordering::Equal)
                        })
                        .expect("evicted value is present in the window");
                    sorted.remove(pos);
                }
            }

            let new = values[block.start + local_idx];
            if new.is_nan() {
                nan_count += 1;
            } else {
                let pos = sorted.partition_point(|&probe| probe < new);
                sorted.insert(pos, new);
            }

            if local_idx + 1 >= days && nan_count == 0 {
                *out_cell = quantile_sorted(&sorted, q);
            }
        }
    })
}

pub(crate) fn decay_linear(values: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_rolling_decay(values, days, cs)
}

fn decay_linear_window(window: &[f64]) -> f64 {
    let weighted = window
        .iter()
        .enumerate()
        .map(|(idx, value)| (idx as f64 + 1.0) * value)
        .sum::<f64>();
    weighted / (window.len() * (window.len() + 1) / 2) as f64
}

pub(crate) fn correlation(lhs: &[f64], rhs: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window2(lhs, rhs, days, cs, correlation_window)
}

pub(crate) fn covariance(lhs: &[f64], rhs: &[f64], days: usize, cs: &CellSet) -> Vec<f64> {
    ts_window2(lhs, rhs, days, cs, covariance_window)
}

pub(crate) fn rank(values: &[f64], input_layout: Layout, output: Layout, cs: &CellSet) -> Vec<f64> {
    xs_per_block(values, input_layout, output, cs, rank_pairs)
}

pub(crate) fn scale(
    values: &[f64],
    input_layout: Layout,
    output: Layout,
    scale_to: f64,
    cs: &CellSet,
) -> Vec<f64> {
    xs_per_block(values, input_layout, output, cs, |present| {
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

pub(crate) fn group_rank(
    values: &[f64],
    values_layout: Layout,
    groups: &[f64],
    groups_layout: Layout,
    output: Layout,
    cs: &CellSet,
) -> Vec<f64> {
    xs_per_group(
        values,
        values_layout,
        groups,
        groups_layout,
        output,
        cs,
        rank_pairs,
    )
}

pub(crate) fn group_neutralize(
    values: &[f64],
    values_layout: Layout,
    groups: &[f64],
    groups_layout: Layout,
    output: Layout,
    cs: &CellSet,
) -> Vec<f64> {
    xs_per_group(
        values,
        values_layout,
        groups,
        groups_layout,
        output,
        cs,
        |present| {
            let mean = present.iter().map(|(_, value)| value).sum::<f64>() / present.len() as f64;
            present
                .iter()
                .map(|(idx, value)| (*idx, value - mean))
                .collect()
        },
    )
}

fn rank_pairs(mut present: Vec<(usize, f64)>) -> Vec<(usize, f64)> {
    present.sort_unstable_by(|(_, lhs), (_, rhs)| lhs.partial_cmp(rhs).unwrap_or(Ordering::Equal));
    let count = present.len() as f64;
    let mut start = 0usize;
    while start < present.len() {
        let mut end = start + 1;
        while end < present.len() && present[end].1 == present[start].1 {
            end += 1;
        }

        let rank_avg = (start + 1 + end) as f64 / 2.0;
        let pct = rank_avg / count;
        for (_, value) in &mut present[start..end] {
            *value = pct;
        }
        start = end;
    }
    present
}

/// Percentile time-series rank (qfactors default). The current value's average
/// rank over the window, normalized to `(0, 1]` — the convention most quant
/// pipelines use (matches pandas `rank(pct=true)`), with ties averaged.
fn rank_last(window: &[f64]) -> f64 {
    let target = window[window.len() - 1];
    let mut less = 0usize;
    let mut eq = 0usize;
    for value in window {
        if *value < target {
            less += 1;
        } else if *value == target {
            eq += 1;
        }
    }
    (less + 1 + less + eq) as f64 / 2.0 / window.len() as f64
}

/// Raw time-series rank matching DolphinDB `mrank(x, true, d)`: the 0-based
/// ascending position of the current value in `[0, d - 1]`, minimum on ties
/// (i.e. the count of window values strictly smaller than it).
fn rank_last_raw(window: &[f64]) -> f64 {
    let target = window[window.len() - 1];
    window.iter().filter(|&&value| value < target).count() as f64
}

fn covariance_window(lhs: &[f64], rhs: &[f64]) -> f64 {
    if lhs.len() < 2 {
        return f64::NAN;
    }
    let mut lhs_sum = 0.0;
    let mut rhs_sum = 0.0;
    for (lhs, rhs) in lhs.iter().zip(rhs) {
        lhs_sum += *lhs;
        rhs_sum += *rhs;
    }
    let lhs_mean = lhs_sum / lhs.len() as f64;
    let rhs_mean = rhs_sum / rhs.len() as f64;
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
    let mut lhs_sum = 0.0;
    let mut rhs_sum = 0.0;
    for (lhs, rhs) in lhs.iter().zip(rhs) {
        lhs_sum += *lhs;
        rhs_sum += *rhs;
    }
    let lhs_mean = lhs_sum / lhs.len() as f64;
    let rhs_mean = rhs_sum / rhs.len() as f64;
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

fn regression_parts(window: &[f64]) -> Option<(f64, f64, f64, f64)> {
    let n = window.len();
    if n < 2 {
        return None;
    }

    let n_f = n as f64;
    let x_mean = (n_f - 1.0) / 2.0;
    let mut y_sum = 0.0;
    let mut sum_xy = 0.0;
    for (idx, value) in window.iter().enumerate() {
        y_sum += *value;
        sum_xy += idx as f64 * *value;
    }
    let y_mean = y_sum / n_f;
    let sxx = n_f * (n_f * n_f - 1.0) / 12.0;
    if sxx == 0.0 {
        return None;
    }

    let sxy = sum_xy - n_f * x_mean * y_mean;
    let syy = window
        .iter()
        .map(|value| {
            let centered = value - y_mean;
            centered * centered
        })
        .sum::<f64>();
    Some((sxy, sxx, syy, y_mean))
}

fn slope_window(window: &[f64]) -> f64 {
    let Some((sxy, sxx, _, _)) = regression_parts(window) else {
        return f64::NAN;
    };
    sxy / sxx
}

fn rsquare_window(window: &[f64]) -> f64 {
    let Some((sxy, sxx, syy, _)) = regression_parts(window) else {
        return f64::NAN;
    };
    if syy == 0.0 {
        f64::NAN
    } else {
        sxy * sxy / (sxx * syy)
    }
}

fn resi_window(window: &[f64]) -> f64 {
    let Some((sxy, sxx, _, y_mean)) = regression_parts(window) else {
        return f64::NAN;
    };
    let n = window.len() as f64;
    let x_mean = (n - 1.0) / 2.0;
    let slope = sxy / sxx;
    window[window.len() - 1] - y_mean - slope * (n - 1.0 - x_mean)
}

/// Linear-interpolated quantile of an ascending, NaN-free slice. Callers gate
/// `q` to `[0, 1]` and pass a non-empty window, so no defensive checks are needed
/// here; the arithmetic is identical to a full-sort quantile, hence bit-exact.
fn quantile_sorted(sorted: &[f64], q: f64) -> f64 {
    let pos = q * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let frac = pos - lo as f64;
        sorted[lo] + frac * (sorted[hi] - sorted[lo])
    }
}

pub(crate) fn log_value(value: f64) -> f64 {
    if value > 0.0 { value.ln() } else { f64::NAN }
}

pub(crate) fn sign(value: f64) -> f64 {
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

pub(crate) fn signed_power(value: f64, exponent: f64) -> f64 {
    if value.is_nan() || exponent.is_nan() {
        f64::NAN
    } else {
        sign(value) * value.abs().powf(exponent)
    }
}

pub(crate) fn min_value(lhs: f64, rhs: f64) -> f64 {
    if lhs.is_nan() || rhs.is_nan() {
        f64::NAN
    } else {
        lhs.min(rhs)
    }
}

pub(crate) fn max_value(lhs: f64, rhs: f64) -> f64 {
    if lhs.is_nan() || rhs.is_nan() {
        f64::NAN
    } else {
        lhs.max(rhs)
    }
}

pub(crate) fn cmp_value(op: CmpOp, lhs: f64, rhs: f64) -> f64 {
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

pub(crate) fn where_value(cond: f64, when_true: f64, when_false: f64) -> f64 {
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
    use std::sync::Arc;

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
            orig_index_tn: (0..n_cells).collect(),
            fields: fields
                .into_iter()
                .map(|(name, values)| (name, Arc::new(values)))
                .collect(),
            symbols_tn: Column::new("asset".into(), vec!["A"; n_cells]),
            times_tn: Column::new("time".into(), (0..n_cells as i64).collect::<Vec<_>>()),
            time_block_by_value: HashMap::new(),
        }
    }

    fn cells(value: Val<'_>, cs: &CellSet) -> Vec<f64> {
        to_cells(value, Layout::Nt, cs).into_owned()
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

    fn assert_vec_same_numeric(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (idx, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            if expected.is_nan() {
                assert!(actual.is_nan(), "idx {idx}: actual {actual}, expected NaN");
            } else if expected.is_infinite() {
                assert_eq!(
                    actual, expected,
                    "idx {idx}: actual {actual}, expected {expected}"
                );
            } else {
                assert!(
                    (actual - expected).abs() < 1e-12,
                    "idx {idx}: actual {actual}, expected {expected}"
                );
            }
        }
    }

    fn two_pass_std(window: &[f64]) -> f64 {
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
    }

    fn reference_quantile(
        values: &[f64],
        days: usize,
        q: f64,
        blocks: &[Range<usize>],
    ) -> Vec<f64> {
        let mut out = vec![f64::NAN; values.len()];
        if days == 0 || !(0.0..=1.0).contains(&q) {
            return out;
        }

        for block in blocks {
            for local_idx in 0..block.len() {
                if local_idx + 1 < days {
                    continue;
                }
                let idx = block.start + local_idx;
                let start = idx + 1 - days;
                let window = &values[start..=idx];
                if window.iter().any(|value| value.is_nan()) {
                    continue;
                }
                out[idx] = reference_quantile_window(window, q);
            }
        }
        out
    }

    fn reference_quantile_window(window: &[f64], q: f64) -> f64 {
        let mut sorted = window.to_vec();
        sorted.sort_by(|lhs, rhs| lhs.partial_cmp(rhs).unwrap_or(Ordering::Equal));
        let pos = q * (sorted.len() - 1) as f64;
        let lo = pos.floor() as usize;
        let hi = pos.ceil() as usize;
        if lo == hi {
            sorted[lo]
        } else {
            let frac = pos - lo as f64;
            sorted[lo] + frac * (sorted[hi] - sorted[lo])
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
            &[f64::NAN, f64::NAN, 1.0, 0.0, 0.0],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::TsArgMax(Box::new(Expr::Field("x".to_string())), 3),
                    &cs,
                )?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, 0.0, 1.0, 2.0],
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
    fn regression_and_quantile_operators_match_hand_examples() -> Result<()> {
        let cs = test_cellset(
            vec![1.0, 3.0, 5.0, 7.0, 8.0],
            one_block(0..5),
            vec![0..1, 1..2, 2..3, 3..4, 4..5],
        );

        assert_vec_close(
            &cells(
                eval(&Expr::Slope(Box::new(Expr::Field("x".to_string())), 3), &cs)?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, 2.0, 2.0, 1.5],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Rsquare(Box::new(Expr::Field("x".to_string())), 3),
                    &cs,
                )?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, 1.0, 1.0, 9.0 / (2.0 * (14.0 / 3.0))],
        );
        assert_vec_close(
            &cells(
                eval(&Expr::Resi(Box::new(Expr::Field("x".to_string())), 3), &cs)?,
                &cs,
            ),
            &[f64::NAN, f64::NAN, 0.0, 0.0, -1.0 / 6.0],
        );

        let flat_cs = test_cellset(vec![2.0, 2.0, 2.0], one_block(0..3), vec![0..1, 1..2, 2..3]);
        let flat_slope = cells(
            eval(
                &Expr::Slope(Box::new(Expr::Field("x".to_string())), 3),
                &flat_cs,
            )?,
            &flat_cs,
        );
        let flat_rsquare = cells(
            eval(
                &Expr::Rsquare(Box::new(Expr::Field("x".to_string())), 3),
                &flat_cs,
            )?,
            &flat_cs,
        );
        let flat_resi = cells(
            eval(
                &Expr::Resi(Box::new(Expr::Field("x".to_string())), 3),
                &flat_cs,
            )?,
            &flat_cs,
        );
        assert_f64_eq(flat_slope[2], 0.0);
        assert!(flat_rsquare[2].is_nan());
        assert_f64_eq(flat_resi[2], 0.0);

        let q_cs = test_cellset(
            vec![3.0, 1.0, 2.0, 2.0, 5.0],
            one_block(0..5),
            vec![0..1, 1..2, 2..3, 3..4, 4..5],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Quantile(Box::new(Expr::Field("x".to_string())), 3, 0.8),
                    &q_cs,
                )?,
                &q_cs,
            ),
            &[f64::NAN, f64::NAN, 2.6, 2.0, 3.8],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Quantile(Box::new(Expr::Field("x".to_string())), 3, 0.2),
                    &q_cs,
                )?,
                &q_cs,
            ),
            &[f64::NAN, f64::NAN, 1.4, 1.4, 2.0],
        );
        Ok(())
    }

    #[test]
    fn quantile_matches_sorted_windows_for_edge_values() {
        let values = vec![
            3.0,
            -0.0,
            0.0,
            f64::INFINITY,
            2.0,
            2.0,
            f64::NAN,
            4.0,
            f64::NEG_INFINITY,
            4.0,
            1.0,
            -0.0,
            0.0,
            5.0,
        ];
        let blocks = vec![0..7, 7..values.len()];
        let cs = test_cellset(
            values.clone(),
            blocks.clone(),
            (0..values.len()).map(|idx| idx..idx + 1).collect(),
        );

        for days in 1..=5 {
            for q in [0.0, 0.2, 0.5, 0.8, 1.0] {
                assert_vec_same_numeric(
                    &quantile(&values, days, q, &cs),
                    &reference_quantile(&values, days, q, &blocks),
                );
            }
        }
    }

    /// Differential fuzz: the incremental sorted window must agree bit-for-bit
    /// with the full-sort reference across many random panels. The palette stuffs
    /// in ties, signed zeros, and infinities to exercise binary insert/remove, and
    /// NaNs to exercise the gate that keeps them out of the sorted structure while
    /// the window still slides over them.
    #[test]
    fn quantile_incremental_matches_full_sort_reference() {
        let mut state = 0x9E3779B97F4A7C15u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let palette = [
            0.0,
            -0.0,
            1.0,
            1.0,
            2.0,
            -3.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ];

        for _ in 0..100 {
            let len = 1 + (next() % 40) as usize;
            let values: Vec<f64> = (0..len)
                .map(|_| palette[(next() % palette.len() as u64) as usize])
                .collect();

            let mut blocks = Vec::new();
            let mut start = 0;
            while start < len {
                let size = 1 + (next() % (len - start) as u64) as usize;
                blocks.push(start..start + size);
                start += size;
            }

            let cs = test_cellset(
                values.clone(),
                blocks.clone(),
                (0..len).map(|idx| idx..idx + 1).collect(),
            );

            for days in 1..=6 {
                for q in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
                    assert_vec_same_numeric(
                        &quantile(&values, days, q, &cs),
                        &reference_quantile(&values, days, q, &blocks),
                    );
                }
            }
        }
    }

    /// Locks `ts_argmax` / `ts_argmin` / `ts_rank_raw` to the DolphinDB
    /// reference (`mimax` / `mimin` / `mrank`). Each series is a single 5-day
    /// symbol with window 5, so only the final cell is defined; the value is the
    /// DolphinDB toy output for that series. The default `ts_rank` is percentile
    /// (see `single_input_ts_operators_match_hand_examples`); `ts_rank_raw` is
    /// the DolphinDB-compatible caliber.
    #[test]
    fn ts_arg_and_rank_match_dolphindb_reference() -> Result<()> {
        let last = |values: Vec<f64>, expr: Expr| -> Result<f64> {
            let cs = test_cellset(values, one_block(0..5), vec![0..1, 1..2, 2..3, 3..4, 4..5]);
            Ok(*cells(eval(&expr, &cs)?, &cs).last().unwrap())
        };
        let arg_max = |v| Expr::TsArgMax(Box::new(Expr::Field("x".to_string())), v);
        let arg_min = |v| Expr::TsArgMin(Box::new(Expr::Field("x".to_string())), v);
        let ts_rank_raw = |v| Expr::TsRankRaw(Box::new(Expr::Field("x".to_string())), v);

        // mimax: position from oldest (0), earliest max wins on ties.
        assert_eq!(last(vec![1.0, 2.0, 3.0, 4.0, 5.0], arg_max(5))?, 4.0);
        assert_eq!(last(vec![5.0, 1.0, 2.0, 5.0, 3.0], arg_max(5))?, 0.0);
        // mimin: position from oldest, earliest min wins on ties.
        assert_eq!(last(vec![5.0, 4.0, 3.0, 2.0, 1.0], arg_min(5))?, 4.0);
        // mrank: 0-based ascending rank of the current value, minimum on ties.
        assert_eq!(last(vec![1.0, 2.0, 3.0, 4.0, 5.0], ts_rank_raw(5))?, 4.0);
        assert_eq!(last(vec![1.0, 2.0, 5.0, 4.0, 5.0], ts_rank_raw(5))?, 3.0);
        Ok(())
    }

    #[test]
    fn ts_std_accurate_for_low_relative_variance() -> Result<()> {
        let days = 60;
        let values = (0..90)
            .map(|idx| 1000.0 + ((idx % 13) as f64 - 6.0) * 0.01)
            .collect::<Vec<_>>();
        let cs = test_cellset(
            values.clone(),
            one_block(0..values.len()),
            (0..values.len()).map(|idx| idx..idx + 1).collect(),
        );

        let out = cells(
            eval(
                &Expr::TsStd(Box::new(Expr::Field("x".to_string())), days),
                &cs,
            )?,
            &cs,
        );

        for local_idx in days - 1..values.len() {
            let expected = two_pass_std(&values[local_idx + 1 - days..=local_idx]);
            assert!(
                (out[local_idx] - expected).abs() <= 1e-8,
                "idx {local_idx}: actual {}, expected {expected}",
                out[local_idx]
            );
        }

        Ok(())
    }

    #[test]
    fn rolling_windows_recover_after_nan_and_infinity() -> Result<()> {
        let values = vec![
            1.0,
            f64::INFINITY,
            3.0,
            4.0,
            5.0,
            f64::NAN,
            7.0,
            8.0,
            9.0,
            f64::NEG_INFINITY,
            11.0,
            12.0,
            13.0,
        ];
        let cs = test_cellset(
            values.clone(),
            one_block(0..values.len()),
            (0..values.len()).map(|idx| idx..idx + 1).collect(),
        );

        let sum = cells(
            eval(&Expr::TsSum(Box::new(Expr::Field("x".to_string())), 3), &cs)?,
            &cs,
        );
        let mean = cells(
            eval(
                &Expr::TsMean(Box::new(Expr::Field("x".to_string())), 3),
                &cs,
            )?,
            &cs,
        );
        let std = cells(
            eval(&Expr::TsStd(Box::new(Expr::Field("x".to_string())), 3), &cs)?,
            &cs,
        );
        let decay = cells(
            eval(
                &Expr::DecayLinear(Box::new(Expr::Field("x".to_string())), 3),
                &cs,
            )?,
            &cs,
        );

        assert!(sum[2].is_infinite() && sum[2].is_sign_positive());
        assert!(mean[2].is_infinite() && mean[2].is_sign_positive());
        assert!(std[2].is_nan());
        assert!(decay[2].is_infinite() && decay[2].is_sign_positive());

        assert_f64_eq(sum[4], 12.0);
        assert_f64_eq(mean[4], 4.0);
        assert_f64_eq(std[4], 1.0);
        assert_f64_eq(decay[4], 26.0 / 6.0);

        for idx in 5..=7 {
            assert!(sum[idx].is_nan());
            assert!(mean[idx].is_nan());
            assert!(std[idx].is_nan());
            assert!(decay[idx].is_nan());
        }

        assert_f64_eq(sum[8], 24.0);
        assert_f64_eq(mean[8], 8.0);
        assert_f64_eq(std[8], 1.0);
        assert_f64_eq(decay[8], 50.0 / 6.0);

        assert!(sum[9].is_infinite() && sum[9].is_sign_negative());
        assert!(mean[9].is_infinite() && mean[9].is_sign_negative());
        assert!(std[9].is_nan());
        assert!(decay[9].is_infinite() && decay[9].is_sign_negative());

        assert_f64_eq(sum[12], 36.0);
        assert_f64_eq(mean[12], 12.0);
        assert_f64_eq(std[12], 1.0);
        assert_f64_eq(decay[12], 74.0 / 6.0);

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
                ("exp".to_string(), vec![2.0, 3.0, 0.5, 2.0]),
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
                    &Expr::SignedPower(
                        Box::new(Expr::Field("x".to_string())),
                        Box::new(Expr::Const(2.0)),
                    ),
                    &cs,
                )?,
                &cs,
            ),
            &[-4.0, 0.0, 9.0, f64::NAN],
        );
        assert_vec_close(
            &cells(
                eval(
                    &Expr::Power(
                        Box::new(Expr::Field("x".to_string())),
                        Box::new(Expr::Field("exp".to_string())),
                    ),
                    &cs,
                )?,
                &cs,
            ),
            &[4.0, 0.0, 3.0_f64.sqrt(), f64::NAN],
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
        assert_vec_close(&tie_out, &[0.25, 0.625, 0.625, 1.0]);

        let dense_tie_cs = test_cellset(
            vec![2.0, 2.0, 1.0, 1.0, 3.0, 3.0],
            vec![0..1, 1..2, 2..3, 3..4, 4..5, 5..6],
            one_block(0..6),
        );
        let dense_tie_out = to_cells(
            eval(
                &Expr::Rank(Box::new(Expr::Field("x".to_string()))),
                &dense_tie_cs,
            )?,
            Layout::Tn,
            &dense_tie_cs,
        );
        assert_vec_close(
            &dense_tie_out,
            &[7.0 / 12.0, 7.0 / 12.0, 0.25, 0.25, 11.0 / 12.0, 11.0 / 12.0],
        );

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

        let cs = test_cellset(vec![1.0, 2.0, 3.0], one_block(0..3), vec![0..1, 1..2, 2..3]);
        let out = eval(
            &Expr::Add(
                Box::new(Expr::Field("x".to_string())),
                Box::new(Expr::Const(2.5)),
            ),
            &cs,
        )?;

        assert_eq!(
            out,
            Val::Cells {
                values: Cow::Owned(vec![3.5, 4.5, 5.5]),
                layout: Layout::Nt,
            }
        );
        Ok(())
    }
}

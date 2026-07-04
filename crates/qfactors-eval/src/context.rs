use std::collections::HashMap;
use std::ops::Range;

use polars::prelude::*;
use qfactors_core::QFactorsError;
use rayon::prelude::*;

use crate::error::{EvalError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binning {
    Daily,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Demean {
    None,
    Universe,
    Group,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weighting {
    Factor,
    Quantile,
}

#[derive(Debug, Clone)]
pub struct EvalSpec {
    pub quantiles: usize,
    pub binning: Binning,
    pub demean: Demean,
    pub min_cs_count: usize,
    pub cost_bps: f64,
    pub weighting: Weighting,
}

/// Per-run market-side state, built once and shared read-only by all factors.
///
/// Everything is laid out in TN order (day blocks contiguous, symbols ascending
/// within a block) so per-day kernels are cache-friendly slices.
#[derive(Debug)]
pub struct EvalContext {
    pub blocks: Vec<Range<usize>>,
    /// Horizon (in panel bars) per label, ascending.
    pub horizons: Vec<usize>,
    /// One TN vector per horizon, demeaned according to the spec.
    pub labels: Vec<Vec<f64>>,
    /// `[horizon][day]`: in-block offsets of tradable, non-NaN label samples,
    /// sorted ascending by (demeaned) label value. Shared by every factor's
    /// rank-IC subset re-ranking.
    pub label_sorted: Vec<Vec<Vec<u32>>>,
    pub tradable: Option<Vec<bool>>,
}

impl EvalContext {
    pub fn build(
        df: &DataFrame,
        time_blocks: &[Range<usize>],
        orig_index_tn: &[usize],
        label_pairs: &[(String, usize)],
        tradable_col: Option<&str>,
        group_col: Option<&str>,
        demean: Demean,
    ) -> Result<Self> {
        let tradable = tradable_col
            .map(|name| gather_bool_tn(df, name, orig_index_tn))
            .transpose()?;
        let group = match (demean, group_col) {
            (Demean::Group, None) => return Err(EvalError::GroupColumnRequired),
            (Demean::Group, Some(name)) => Some(gather_group_tn(df, name, orig_index_tn)?),
            _ => None,
        };

        let horizons: Vec<usize> = label_pairs.iter().map(|(_, h)| *h).collect();
        let mut labels = label_pairs
            .par_iter()
            .map(|(name, _)| gather_f64_tn(df, name, orig_index_tn))
            .collect::<Result<Vec<_>>>()?;

        if demean != Demean::None {
            labels.par_iter_mut().for_each(|label| {
                demean_label(label, time_blocks, tradable.as_deref(), group.as_deref());
            });
        }

        let label_sorted = labels
            .par_iter()
            .map(|label| sort_label_days(label, time_blocks, tradable.as_deref()))
            .collect();

        Ok(Self {
            blocks: time_blocks.to_vec(),
            horizons,
            labels,
            label_sorted,
            tradable,
        })
    }
}

/// Parse a `ret_{h}` label column name into its horizon.
pub fn parse_ret_horizon(name: &str) -> Option<usize> {
    let digits = name.strip_prefix("ret_")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

pub(crate) fn gather_f64_tn(df: &DataFrame, name: &str, order: &[usize]) -> Result<Vec<f64>> {
    let column = df
        .column(name)
        .map_err(|_| EvalError::Core(QFactorsError::MissingColumn(name.to_string())))?;
    let values = column.try_f64().ok_or_else(|| EvalError::DTypeMismatch {
        column: name.to_string(),
        expected: "f64",
        actual: column.dtype().to_string(),
    })?;
    let dense: Vec<f64> = values.iter().map(|v| v.unwrap_or(f64::NAN)).collect();
    Ok(order.par_iter().map(|&orig| dense[orig]).collect())
}

fn gather_bool_tn(df: &DataFrame, name: &str, order: &[usize]) -> Result<Vec<bool>> {
    let column = df
        .column(name)
        .map_err(|_| EvalError::Core(QFactorsError::MissingColumn(name.to_string())))?;
    let values = column.try_bool().ok_or_else(|| EvalError::DTypeMismatch {
        column: name.to_string(),
        expected: "bool",
        actual: column.dtype().to_string(),
    })?;
    // Null tradability means "cannot trade".
    let dense: Vec<bool> = values.iter().map(|v| v.unwrap_or(false)).collect();
    Ok(order.par_iter().map(|&orig| dense[orig]).collect())
}

fn gather_group_tn(df: &DataFrame, name: &str, order: &[usize]) -> Result<Vec<u32>> {
    let column = df
        .column(name)
        .map_err(|_| EvalError::Core(QFactorsError::MissingColumn(name.to_string())))?;
    let values = column.try_str().ok_or_else(|| EvalError::DTypeMismatch {
        column: name.to_string(),
        expected: "str",
        actual: column.dtype().to_string(),
    })?;
    let mut codes = HashMap::new();
    let dense = values
        .iter()
        .map(|value| {
            let value = value.ok_or_else(|| EvalError::GroupNull(name.to_string()))?;
            let next = codes.len() as u32;
            Ok(*codes.entry(value.to_string()).or_insert(next))
        })
        .collect::<Result<Vec<u32>>>()?;
    Ok(order.par_iter().map(|&orig| dense[orig]).collect())
}

/// Subtract the per-day benchmark mean (universe-wide, or per group when
/// `group` is given) from every non-NaN label. The mean is taken over the
/// evaluated universe: tradable samples with a non-NaN label.
fn demean_label(
    label: &mut [f64],
    blocks: &[Range<usize>],
    tradable: Option<&[bool]>,
    group: Option<&[u32]>,
) {
    let n_groups = group
        .map(|g| g.iter().max().map_or(0, |&m| m as usize + 1))
        .unwrap_or(1);
    let mut sums = vec![0.0; n_groups];
    let mut counts = vec![0usize; n_groups];
    for block in blocks {
        sums.fill(0.0);
        counts.fill(0);
        for idx in block.clone() {
            if label[idx].is_nan() || !tradable.is_none_or(|t| t[idx]) {
                continue;
            }
            let gid = group.map_or(0, |g| g[idx] as usize);
            sums[gid] += label[idx];
            counts[gid] += 1;
        }
        for idx in block.clone() {
            if label[idx].is_nan() {
                continue;
            }
            let gid = group.map_or(0, |g| g[idx] as usize);
            // A downstream-visible sample always has count >= 1 (it contributes
            // itself); zero-count groups only hold excluded samples.
            if counts[gid] > 0 {
                label[idx] -= sums[gid] / counts[gid] as f64;
            }
        }
    }
}

fn sort_label_days(
    label: &[f64],
    blocks: &[Range<usize>],
    tradable: Option<&[bool]>,
) -> Vec<Vec<u32>> {
    blocks
        .par_iter()
        .map(|block| {
            let base = block.start;
            let mut offsets: Vec<u32> = block
                .clone()
                .filter(|&idx| !label[idx].is_nan() && tradable.is_none_or(|t| t[idx]))
                .map(|idx| (idx - base) as u32)
                .collect();
            offsets.sort_unstable_by(|&a, &b| {
                label[base + a as usize].total_cmp(&label[base + b as usize])
            });
            offsets
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use qfactors_core::PanelOptions;
    use qfactors_core::cellset::build_cellset;

    use super::*;

    fn build(
        df: &DataFrame,
        demean: Demean,
        tradable_col: Option<&str>,
        group_col: Option<&str>,
    ) -> Result<EvalContext> {
        let panel = PanelOptions {
            symbol_col: "asset".to_string(),
            time_col: "time".to_string(),
        };
        let cs = build_cellset(df, &panel, &BTreeSet::new()).unwrap();
        EvalContext::build(
            df,
            &cs.time_blocks,
            &cs.orig_index_tn,
            &[("ret_1".to_string(), 1)],
            tradable_col,
            group_col,
            demean,
        )
    }

    #[test]
    fn parse_ret_horizon_accepts_only_ret_digits() {
        assert_eq!(parse_ret_horizon("ret_1"), Some(1));
        assert_eq!(parse_ret_horizon("ret_20"), Some(20));
        assert_eq!(parse_ret_horizon("ret_"), None);
        assert_eq!(parse_ret_horizon("ret_1d"), None);
        assert_eq!(parse_ret_horizon("returns"), None);
        assert_eq!(parse_ret_horizon("xret_1"), None);
    }

    #[test]
    fn universe_demean_subtracts_tradable_mean() {
        let df = df!(
            "asset" => ["A", "B", "C", "D"],
            "time" => [1i64; 4],
            "ret_1" => [0.10, 0.20, 0.30, 0.40],
            "tradable" => [true, true, true, false],
        )
        .unwrap();

        let ctx = build(&df, Demean::Universe, Some("tradable"), None).unwrap();

        // Mean over tradable samples only: (0.1 + 0.2 + 0.3) / 3 = 0.2.
        let expected = [-0.1, 0.0, 0.1, 0.2];
        for (actual, expected) in ctx.labels[0].iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-12);
        }
        // label_sorted keeps only tradable samples, in ascending label order.
        assert_eq!(ctx.label_sorted[0][0], [0, 1, 2]);
    }

    #[test]
    fn group_demean_uses_group_means() {
        let df = df!(
            "asset" => ["A", "B", "C", "D"],
            "time" => [1i64; 4],
            "ret_1" => [0.10, 0.30, 0.20, 0.60],
            "industry" => ["x", "x", "y", "y"],
        )
        .unwrap();

        let ctx = build(&df, Demean::Group, None, Some("industry")).unwrap();

        let expected = [-0.1, 0.1, -0.2, 0.2];
        for (actual, expected) in ctx.labels[0].iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn group_demean_requires_group_col_and_rejects_nulls() {
        let df = df!(
            "asset" => ["A", "B"],
            "time" => [1i64, 1],
            "ret_1" => [0.1, 0.2],
            "industry" => [Some("x"), None],
        )
        .unwrap();

        let err = build(&df, Demean::Group, None, None).unwrap_err();
        assert!(matches!(err, EvalError::GroupColumnRequired));

        let err = build(&df, Demean::Group, None, Some("industry")).unwrap_err();
        assert!(matches!(err, EvalError::GroupNull(_)));
    }

    #[test]
    fn nan_labels_are_excluded_from_sorting() {
        let df = df!(
            "asset" => ["A", "B", "C"],
            "time" => [1i64; 3],
            "ret_1" => [Some(0.3), None, Some(0.1)],
        )
        .unwrap();

        let ctx = build(&df, Demean::None, None, None).unwrap();

        assert_eq!(ctx.label_sorted[0][0], [2, 0]);
        assert!(ctx.labels[0][1].is_nan());
    }
}

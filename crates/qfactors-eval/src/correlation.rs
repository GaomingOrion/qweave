use polars::prelude::*;
use qfactors_core::{PanelOptions, QFactorsError};
use rayon::prelude::*;

use crate::error::{EvalError, Result};
use crate::panel::build_time_index;
use crate::stats::pearson_from_sums;

/// Time-averaged daily cross-sectional rank correlation between factors.
///
/// Per day, every factor's valid samples (tradable ∧ non-NaN) get average
/// ranks; each pair's Pearson correlation over their common symbols is
/// averaged across days (days where either side has fewer than `min_cs_count`
/// valid samples are skipped for that pair). Output is a symmetric F x F wide
/// frame with a leading `factor` name column.
///
/// Memory: holds every factor column densely, so this is meant for the
/// filtered shortlist after `evaluate`, not thousands of raw factors.
pub fn factor_correlation(
    df: &DataFrame,
    panel: &PanelOptions,
    factor_cols: &[String],
    tradable_col: Option<&str>,
    min_cs_count: usize,
) -> Result<DataFrame> {
    if factor_cols.is_empty() {
        return Err(EvalError::BadFactorColumns("<empty>".to_string()));
    }
    let n_factors = factor_cols.len();
    let ti = build_time_index(df, panel)?;
    let order = &ti.orig_index_tn;

    let factors: Vec<Vec<f64>> = factor_cols
        .par_iter()
        .map(|name| crate::context::gather_f64_tn(df, name, order))
        .collect::<Result<_>>()?;
    let tradable: Option<Vec<bool>> = tradable_col
        .map(|name| -> Result<Vec<bool>> {
            let column = df
                .column(name)
                .map_err(|_| EvalError::Core(QFactorsError::MissingColumn(name.to_string())))?;
            let values = column.try_bool().ok_or_else(|| EvalError::DTypeMismatch {
                column: name.to_string(),
                expected: "bool",
                actual: column.dtype().to_string(),
            })?;
            let dense: Vec<bool> = values.iter().map(|v| v.unwrap_or(false)).collect();
            Ok(order.iter().map(|&orig| dense[orig]).collect())
        })
        .transpose()?;

    let gate = min_cs_count.max(2);
    let n_pairs = n_factors * n_factors;
    let (sum, cnt) = ti
        .blocks
        .par_iter()
        .fold(
            || (vec![0.0f64; n_pairs], vec![0u64; n_pairs]),
            |(mut sum, mut cnt), block| {
                let base = block.start;
                let m = block.len();
                // Average ranks per factor, NaN where invalid.
                let mut ranks = vec![f64::NAN; n_factors * m];
                let mut valid_counts = vec![0usize; n_factors];
                let mut offsets: Vec<u32> = Vec::with_capacity(m);
                for (f_idx, factor) in factors.iter().enumerate() {
                    offsets.clear();
                    for offset in 0..m {
                        let idx = base + offset;
                        if !factor[idx].is_nan() && tradable.as_ref().is_none_or(|t| t[idx]) {
                            offsets.push(offset as u32);
                        }
                    }
                    valid_counts[f_idx] = offsets.len();
                    if offsets.len() < gate {
                        continue;
                    }
                    offsets.sort_by(|&a, &b| {
                        factor[base + a as usize].total_cmp(&factor[base + b as usize])
                    });
                    let row = &mut ranks[f_idx * m..(f_idx + 1) * m];
                    let mut i = 0;
                    while i < offsets.len() {
                        let v = factor[base + offsets[i] as usize];
                        let mut j = i + 1;
                        while j < offsets.len() && factor[base + offsets[j] as usize] == v {
                            j += 1;
                        }
                        let avg = (i + 1 + j) as f64 / 2.0;
                        for &offset in &offsets[i..j] {
                            row[offset as usize] = avg;
                        }
                        i = j;
                    }
                }
                for a in 0..n_factors {
                    if valid_counts[a] < gate {
                        continue;
                    }
                    for b in (a + 1)..n_factors {
                        if valid_counts[b] < gate {
                            continue;
                        }
                        let ra = &ranks[a * m..(a + 1) * m];
                        let rb = &ranks[b * m..(b + 1) * m];
                        let (mut n, mut sx, mut sy, mut sxx, mut syy, mut sxy) =
                            (0usize, 0.0, 0.0, 0.0, 0.0, 0.0);
                        for offset in 0..m {
                            let x = ra[offset];
                            let y = rb[offset];
                            if x.is_nan() || y.is_nan() {
                                continue;
                            }
                            n += 1;
                            sx += x;
                            sy += y;
                            sxx += x * x;
                            syy += y * y;
                            sxy += x * y;
                        }
                        if n >= gate {
                            let corr = pearson_from_sums(n, sx, sy, sxx, syy, sxy);
                            if !corr.is_nan() {
                                sum[a * n_factors + b] += corr;
                                cnt[a * n_factors + b] += 1;
                            }
                        }
                    }
                    // Diagonal: count valid days so self-correlation is 1
                    // exactly when the factor has any usable day.
                    sum[a * n_factors + a] += 1.0;
                    cnt[a * n_factors + a] += 1;
                }
                (sum, cnt)
            },
        )
        .reduce(
            || (vec![0.0f64; n_pairs], vec![0u64; n_pairs]),
            |(mut sum_a, mut cnt_a), (sum_b, cnt_b)| {
                for i in 0..n_pairs {
                    sum_a[i] += sum_b[i];
                    cnt_a[i] += cnt_b[i];
                }
                (sum_a, cnt_a)
            },
        );

    let mut columns = Vec::with_capacity(n_factors + 1);
    columns.push(Column::new("factor".into(), factor_cols.to_vec()));
    for (b, name) in factor_cols.iter().enumerate() {
        let values: Vec<f64> = (0..n_factors)
            .map(|a| {
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                let slot = lo * n_factors + hi;
                if cnt[slot] > 0 {
                    sum[slot] / cnt[slot] as f64
                } else {
                    f64::NAN
                }
            })
            .collect();
        columns.push(Column::new(name.as_str().into(), values));
    }
    DataFrame::new_infer_height(columns).map_err(EvalError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> PanelOptions {
        PanelOptions {
            symbol_col: "asset".to_string(),
            time_col: "time".to_string(),
        }
    }

    #[test]
    fn self_correlation_is_one_and_monotone_pairs_match() -> Result<()> {
        let df = df!(
            "asset" => ["A", "B", "C", "D", "A", "B", "C", "D"],
            "time" => [1i64, 1, 1, 1, 2, 2, 2, 2],
            "f1" => [1.0, 2.0, 3.0, 4.0, 4.0, 3.0, 2.0, 1.0],
            "f2" => [2.0, 4.0, 6.0, 8.0, 8.0, 6.0, 4.0, 2.0],   // monotone in f1
            "f3" => [4.0, 3.0, 2.0, 1.0, 1.0, 2.0, 3.0, 4.0],   // reversed
        )?;

        let out = factor_correlation(
            &df,
            &panel(),
            &["f1".to_string(), "f2".to_string(), "f3".to_string()],
            None,
            2,
        )?;

        let get = |row: usize, col: &str| -> f64 {
            out.column(col)
                .unwrap()
                .try_f64()
                .unwrap()
                .get(row)
                .unwrap()
        };
        assert!((get(0, "f1") - 1.0).abs() < 1e-12);
        assert!((get(0, "f2") - 1.0).abs() < 1e-12); // rank-identical
        assert!((get(0, "f3") + 1.0).abs() < 1e-12); // rank-reversed
        assert!((get(1, "f3") + 1.0).abs() < 1e-12);
        // Symmetry.
        assert_eq!(get(0, "f2"), get(1, "f1"));
        Ok(())
    }

    #[test]
    fn nan_and_gate_days_are_skipped() -> Result<()> {
        let df = df!(
            "asset" => ["A", "B", "C", "A", "B", "C"],
            "time" => [1i64, 1, 1, 2, 2, 2],
            "f1" => [1.0, 2.0, 3.0, f64::NAN, f64::NAN, 3.0],
            "f2" => [1.0, 2.0, 3.0, 1.0, 2.0, 3.0],
        )?;

        // Day 2 has only one valid f1 sample -> skipped; day 1 corr = 1.
        let out = factor_correlation(
            &df,
            &panel(),
            &["f1".to_string(), "f2".to_string()],
            None,
            2,
        )?;

        let value = out.column("f2")?.try_f64().unwrap().get(0).unwrap();
        assert!((value - 1.0).abs() < 1e-12);
        Ok(())
    }
}

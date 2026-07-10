use std::ops::Range;

use rayon::prelude::*;

use crate::context::{Binning, EvalContext, EvalSpec};
use crate::stats::{
    kendall_tau_vs_index, mean_std, newey_west_t, pearson_from_sums, type7_cuts, win_rate,
};

/// Days per parallel work unit inside one factor. Small enough that a single
/// factor saturates the pool (framework: few factors degrade to day-level
/// parallelism), large enough to amortize scratch allocation.
const DAY_CHUNK: usize = 128;

/// Per-(factor, horizon) summary statistics.
pub struct SummaryRow {
    pub n_days: u32,
    pub ic_mean: f64,
    pub ic_std: f64,
    pub ic_ir: f64,
    pub ic_t_nw: f64,
    pub ic_win_rate: f64,
    pub rank_ic_mean: f64,
    pub rank_ic_std: f64,
    pub rank_ic_ir: f64,
    pub rank_ic_t_nw: f64,
    pub rank_ic_win_rate: f64,
    pub spread_mean: f64,
    pub spread_t_nw: f64,
    pub monotonicity: f64,
    pub avg_coverage: f64,
}

/// Everything `evaluate` needs to assemble one factor's rows in the result
/// tables. `ic`/`rank_ic` are dense `[h][day]` (h-major); quantile rows are
/// sparse (only buckets with samples on valid days), sorted by day.
pub struct FactorOutput {
    pub ic: Vec<f64>,
    pub rank_ic: Vec<f64>,
    pub q_day: Vec<u32>,
    pub q_bin: Vec<u32>,
    pub q_lo: Vec<f64>,
    pub q_hi: Vec<f64>,
    pub q_count: Vec<u32>,
    pub q_mean: Vec<Vec<f64>>,
    pub cov_valid: Vec<u32>,
    pub cov_masked: Vec<u32>,
    pub summary: Vec<SummaryRow>,
}

pub struct GlobalBins {
    pub(crate) cuts: Vec<f64>,
    pub(crate) min: f64,
    pub(crate) max: f64,
}

/// One day-chunk's partial results; `ic`/`rank_ic`/`spread` are h-major over
/// the chunk's days only.
struct ChunkOutput {
    ic: Vec<f64>,
    rank_ic: Vec<f64>,
    spread: Vec<f64>,
    q_day: Vec<u32>,
    q_bin: Vec<u32>,
    q_lo: Vec<f64>,
    q_hi: Vec<f64>,
    q_count: Vec<u32>,
    q_mean: Vec<Vec<f64>>,
    cov_valid: Vec<u32>,
    cov_masked: Vec<u32>,
    agg_sum: Vec<f64>,
    agg_cnt: Vec<u64>,
}

/// Evaluate one factor against the shared context.
///
/// Validity layers per day: factor-valid = tradable ∧ factor non-NaN (horizon
/// independent); pair-valid(h) additionally requires a non-NaN label. A day
/// with fewer than `min_cs_count` factor-valid samples is skipped entirely
/// (IC row stays NaN, no quantile rows); a horizon with fewer than
/// `min_cs_count` pair-valid samples gets NaN ic/rank_ic/spread while its
/// bucket means remain visible (sample counts tell the story).
pub fn eval_factor(ctx: &EvalContext, spec: &EvalSpec, factor: &[f64]) -> FactorOutput {
    let global = match spec.binning {
        Binning::Global => Some(global_bins(ctx, factor, spec.quantiles)),
        Binning::Daily => None,
    };
    eval_factor_with(ctx, spec, factor, global.as_ref())
}

/// `eval_factor` with precomputed global bins (shared with the flows pass so
/// the pooled sort runs once per factor).
pub(crate) fn eval_factor_with(
    ctx: &EvalContext,
    spec: &EvalSpec,
    factor: &[f64],
    global: Option<&GlobalBins>,
) -> FactorOutput {
    let t_days = ctx.blocks.len();
    let n_h = ctx.horizons.len();
    let q = spec.quantiles;

    let chunk_ranges: Vec<Range<usize>> = (0..t_days)
        .step_by(DAY_CHUNK)
        .map(|start| start..(start + DAY_CHUNK).min(t_days))
        .collect();
    let chunks: Vec<ChunkOutput> = chunk_ranges
        .par_iter()
        .map(|days| eval_day_chunk(ctx, spec, factor, days.clone(), global))
        .collect();

    // Merge: chunks are in day order, so quantile rows stay day-sorted.
    let mut ic = vec![f64::NAN; n_h * t_days];
    let mut rank_ic = vec![f64::NAN; n_h * t_days];
    let mut spread = vec![f64::NAN; n_h * t_days];
    let q_rows: usize = chunks.iter().map(|c| c.q_day.len()).sum();
    let mut q_day = Vec::with_capacity(q_rows);
    let mut q_bin = Vec::with_capacity(q_rows);
    let mut q_lo = Vec::with_capacity(q_rows);
    let mut q_hi = Vec::with_capacity(q_rows);
    let mut q_count = Vec::with_capacity(q_rows);
    let mut q_mean: Vec<Vec<f64>> = vec![Vec::with_capacity(q_rows); n_h];
    let mut cov_valid = vec![0u32; t_days];
    let mut cov_masked = vec![0u32; t_days];
    let mut agg_sum = vec![0.0f64; n_h * q];
    let mut agg_cnt = vec![0u64; n_h * q];
    for (days, chunk) in chunk_ranges.iter().zip(chunks) {
        let len = days.len();
        for h_idx in 0..n_h {
            ic[h_idx * t_days + days.start..h_idx * t_days + days.end]
                .copy_from_slice(&chunk.ic[h_idx * len..(h_idx + 1) * len]);
            rank_ic[h_idx * t_days + days.start..h_idx * t_days + days.end]
                .copy_from_slice(&chunk.rank_ic[h_idx * len..(h_idx + 1) * len]);
            spread[h_idx * t_days + days.start..h_idx * t_days + days.end]
                .copy_from_slice(&chunk.spread[h_idx * len..(h_idx + 1) * len]);
            q_mean[h_idx].extend_from_slice(&chunk.q_mean[h_idx]);
        }
        q_day.extend_from_slice(&chunk.q_day);
        q_bin.extend_from_slice(&chunk.q_bin);
        q_lo.extend_from_slice(&chunk.q_lo);
        q_hi.extend_from_slice(&chunk.q_hi);
        q_count.extend_from_slice(&chunk.q_count);
        cov_valid[days.clone()].copy_from_slice(&chunk.cov_valid);
        cov_masked[days.clone()].copy_from_slice(&chunk.cov_masked);
        for slot in 0..n_h * q {
            agg_sum[slot] += chunk.agg_sum[slot];
            agg_cnt[slot] += chunk.agg_cnt[slot];
        }
    }

    let avg_coverage = {
        let mut sum = 0.0;
        for (day, block) in ctx.blocks.iter().enumerate() {
            if !block.is_empty() {
                sum += cov_valid[day] as f64 / block.len() as f64;
            }
        }
        if t_days > 0 {
            sum / t_days as f64
        } else {
            f64::NAN
        }
    };
    let mut summary = Vec::with_capacity(n_h);
    for h_idx in 0..n_h {
        let lag = ctx.horizons[h_idx].saturating_sub(1);
        let ic_series = &ic[h_idx * t_days..(h_idx + 1) * t_days];
        let rank_series = &rank_ic[h_idx * t_days..(h_idx + 1) * t_days];
        let spread_series = &spread[h_idx * t_days..(h_idx + 1) * t_days];
        let (ic_mean, ic_std, n_days) = mean_std(ic_series);
        let (rank_ic_mean, rank_ic_std, _) = mean_std(rank_series);
        let (spread_mean, _, _) = mean_std(spread_series);
        let bin_means: Vec<f64> = (0..q)
            .map(|bucket| {
                let slot = h_idx * q + bucket;
                if agg_cnt[slot] > 0 {
                    agg_sum[slot] / agg_cnt[slot] as f64
                } else {
                    f64::NAN
                }
            })
            .collect();
        summary.push(SummaryRow {
            n_days: n_days as u32,
            ic_mean,
            ic_std,
            ic_ir: ic_mean / ic_std,
            ic_t_nw: newey_west_t(ic_series, lag),
            ic_win_rate: win_rate(ic_series),
            rank_ic_mean,
            rank_ic_std,
            rank_ic_ir: rank_ic_mean / rank_ic_std,
            rank_ic_t_nw: newey_west_t(rank_series, lag),
            rank_ic_win_rate: win_rate(rank_series),
            spread_mean,
            spread_t_nw: newey_west_t(spread_series, lag),
            monotonicity: kendall_tau_vs_index(&bin_means),
            avg_coverage,
        });
    }

    FactorOutput {
        ic,
        rank_ic,
        q_day,
        q_bin,
        q_lo,
        q_hi,
        q_count,
        q_mean,
        cov_valid,
        cov_masked,
        summary,
    }
}

fn eval_day_chunk(
    ctx: &EvalContext,
    spec: &EvalSpec,
    factor: &[f64],
    days: Range<usize>,
    global: Option<&GlobalBins>,
) -> ChunkOutput {
    let len = days.len();
    let n_h = ctx.horizons.len();
    let q = spec.quantiles;
    let max_block = ctx.blocks[days.clone()]
        .iter()
        .map(|b| b.len())
        .max()
        .unwrap_or(0);
    let day_gate = spec.min_cs_count.max(1);
    let pair_gate = spec.min_cs_count.max(2);

    let mut out = ChunkOutput {
        ic: vec![f64::NAN; n_h * len],
        rank_ic: vec![f64::NAN; n_h * len],
        spread: vec![f64::NAN; n_h * len],
        q_day: Vec::new(),
        q_bin: Vec::new(),
        q_lo: Vec::new(),
        q_hi: Vec::new(),
        q_count: Vec::new(),
        q_mean: vec![Vec::new(); n_h],
        cov_valid: vec![0u32; len],
        cov_masked: vec![0u32; len],
        agg_sum: vec![0.0f64; n_h * q],
        agg_cnt: vec![0u64; n_h * q],
    };

    let mut sorted: Vec<u32> = Vec::with_capacity(max_block);
    let mut pair: Vec<u32> = Vec::with_capacity(max_block);
    let mut label_filtered: Vec<u32> = Vec::with_capacity(max_block);
    let mut factor_rank = vec![0.0f64; max_block];
    let mut label_rank = vec![0.0f64; max_block];
    let mut bucket_of = vec![0u32; max_block];
    let mut count = vec![0u32; q];
    let mut lo = vec![f64::NAN; q];
    let mut hi = vec![f64::NAN; q];
    let mut label_sum = vec![0.0f64; q];
    let mut label_cnt = vec![0u32; q];
    let mut day_mean = vec![f64::NAN; n_h * q];

    for day in days.clone() {
        let block = &ctx.blocks[day];
        let base = block.start;
        let chunk_day = day - days.start;

        // Factor-valid samples, plus the masked count for coverage.
        sorted.clear();
        let mut masked = 0u32;
        for offset in 0..block.len() {
            let idx = base + offset;
            if factor[idx].is_nan() {
                continue;
            }
            if ctx.tradable.as_ref().is_none_or(|t| t[idx]) {
                sorted.push(offset as u32);
            } else {
                masked += 1;
            }
        }
        let m = sorted.len();
        out.cov_valid[chunk_day] = m as u32;
        out.cov_masked[chunk_day] = masked;
        if m < day_gate {
            continue;
        }

        // One stable sort by factor value: equal values keep ascending offset
        // (= symbol) order, making bucket assignment deterministic.
        sorted.sort_by(|&a, &b| factor[base + a as usize].total_cmp(&factor[base + b as usize]));

        count.fill(0);
        for (pos, &offset) in sorted.iter().enumerate() {
            let value = factor[base + offset as usize];
            let bucket = match global {
                None => pos * q / m,
                Some(bins) => bins.cuts.partition_point(|&cut| cut < value),
            };
            bucket_of[pos] = bucket as u32;
            if count[bucket] == 0 {
                lo[bucket] = value;
            }
            hi[bucket] = value;
            count[bucket] += 1;
        }
        if let Some(bins) = global {
            for bucket in 0..q {
                lo[bucket] = if bucket == 0 {
                    bins.min
                } else {
                    bins.cuts[bucket - 1]
                };
                hi[bucket] = if bucket == q - 1 {
                    bins.max
                } else {
                    bins.cuts[bucket]
                };
            }
        }

        for h_idx in 0..n_h {
            let label = &ctx.labels[h_idx];
            label_sum.fill(0.0);
            label_cnt.fill(0);
            pair.clear();
            let (mut n, mut sx, mut sy, mut sxx, mut syy, mut sxy) =
                (0usize, 0.0, 0.0, 0.0, 0.0, 0.0);
            for (pos, &offset) in sorted.iter().enumerate() {
                let idx = base + offset as usize;
                let y = label[idx];
                if y.is_nan() {
                    continue;
                }
                let bucket = bucket_of[pos] as usize;
                label_sum[bucket] += y;
                label_cnt[bucket] += 1;
                let x = factor[idx];
                n += 1;
                sx += x;
                sy += y;
                sxx += x * x;
                syy += y * y;
                sxy += x * y;
                pair.push(offset);
            }

            // Subset re-ranking: `pair` is pair-valid in factor order; the
            // filtered label_sorted list is the same set in label order. Both
            // scratch arrays are fully rewritten for exactly the offsets read
            // below, so no reset is needed.
            assign_average_ranks(&pair, |off| factor[base + off as usize], &mut factor_rank);
            label_filtered.clear();
            for &offset in &ctx.label_sorted[h_idx][day] {
                if !factor[base + offset as usize].is_nan() {
                    label_filtered.push(offset);
                }
            }
            assign_average_ranks(
                &label_filtered,
                |off| label[base + off as usize],
                &mut label_rank,
            );
            let (mut rsx, mut rsy, mut rsxx, mut rsyy, mut rsxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for &offset in &pair {
                let x = factor_rank[offset as usize];
                let y = label_rank[offset as usize];
                rsx += x;
                rsy += y;
                rsxx += x * x;
                rsyy += y * y;
                rsxy += x * y;
            }

            let slot = h_idx * len + chunk_day;
            if n >= pair_gate {
                out.ic[slot] = pearson_from_sums(n, sx, sy, sxx, syy, sxy);
                out.rank_ic[slot] = pearson_from_sums(n, rsx, rsy, rsxx, rsyy, rsxy);
                if label_cnt[0] > 0 && label_cnt[q - 1] > 0 {
                    out.spread[slot] = label_sum[q - 1] / label_cnt[q - 1] as f64
                        - label_sum[0] / label_cnt[0] as f64;
                }
            }
            for bucket in 0..q {
                let slot = h_idx * q + bucket;
                day_mean[slot] = if label_cnt[bucket] > 0 {
                    label_sum[bucket] / label_cnt[bucket] as f64
                } else {
                    f64::NAN
                };
                out.agg_sum[slot] += label_sum[bucket];
                out.agg_cnt[slot] += label_cnt[bucket] as u64;
            }
        }

        for bucket in 0..q {
            if count[bucket] == 0 {
                continue;
            }
            out.q_day.push(day as u32);
            out.q_bin.push(bucket as u32 + 1);
            out.q_lo.push(lo[bucket]);
            out.q_hi.push(hi[bucket]);
            out.q_count.push(count[bucket]);
            for h_idx in 0..n_h {
                out.q_mean[h_idx].push(day_mean[h_idx * q + bucket]);
            }
        }
    }

    out
}

/// Assign 1-based average ranks to `sorted` (ascending by `value`), writing
/// `out[offset] = rank`; tied values share the average of their positions.
fn assign_average_ranks(sorted: &[u32], value: impl Fn(u32) -> f64, out: &mut [f64]) {
    let mut i = 0;
    while i < sorted.len() {
        let v = value(sorted[i]);
        let mut j = i + 1;
        while j < sorted.len() && value(sorted[j]) == v {
            j += 1;
        }
        let avg = (i + 1 + j) as f64 / 2.0;
        for &offset in &sorted[i..j] {
            out[offset as usize] = avg;
        }
        i = j;
    }
}

pub(crate) fn global_bins(ctx: &EvalContext, factor: &[f64], q: usize) -> GlobalBins {
    let mut values: Vec<f64> = (0..factor.len())
        .filter(|&idx| !factor[idx].is_nan() && ctx.tradable.as_ref().is_none_or(|t| t[idx]))
        .map(|idx| factor[idx])
        .collect();
    values.par_sort_unstable_by(|a, b| a.total_cmp(b));
    if values.is_empty() {
        return GlobalBins {
            cuts: vec![f64::NAN; q - 1],
            min: f64::NAN,
            max: f64::NAN,
        };
    }
    GlobalBins {
        cuts: type7_cuts(&values, q),
        min: values[0],
        max: *values.last().expect("non-empty"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use polars::prelude::*;
    use qweave_core::PanelOptions;
    use qweave_core::cellset::build_cellset;

    use super::*;
    use crate::context::Demean;

    fn spec(quantiles: usize, min_cs_count: usize, binning: Binning) -> EvalSpec {
        EvalSpec {
            quantiles,
            binning,
            demean: Demean::None,
            min_cs_count,
            cost_bps: 0.0,
            weighting: crate::context::Weighting::Factor,
        }
    }

    fn context(df: &DataFrame, tradable_col: Option<&str>) -> EvalContext {
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
            None,
            Demean::None,
        )
        .unwrap()
    }

    /// 2 days x 4 assets; the factor is a noiseless monotone transform of the
    /// label, so IC and RankIC are exactly 1 and buckets order perfectly.
    fn perfect_panel() -> (EvalContext, Vec<f64>) {
        let df = df!(
            "asset" => ["A", "B", "C", "D", "A", "B", "C", "D"],
            "time" => [1i64, 1, 1, 1, 2, 2, 2, 2],
            "ret_1" => [0.01, 0.02, 0.03, 0.04, 0.04, 0.03, 0.02, 0.01],
        )
        .unwrap();
        let ctx = context(&df, None);
        // TN order matches input order here (already time-major, symbols sorted).
        let factor = vec![1.0, 2.0, 3.0, 4.0, 4.0, 3.0, 2.0, 1.0];
        (ctx, factor)
    }

    #[test]
    fn perfect_factor_has_unit_ic_and_ordered_buckets() {
        let (ctx, factor) = perfect_panel();

        let out = eval_factor(&ctx, &spec(2, 2, Binning::Daily), &factor);

        for day in 0..2 {
            assert!((out.ic[day] - 1.0).abs() < 1e-12);
            assert!((out.rank_ic[day] - 1.0).abs() < 1e-12);
        }
        // Day 1: bottom bucket {0.01, 0.02} mean 0.015, top {0.03, 0.04} 0.035.
        assert_eq!(out.q_bin, [1, 2, 1, 2]);
        assert_eq!(out.q_count, [2, 2, 2, 2]);
        let means = &out.q_mean[0];
        assert!((means[0] - 0.015).abs() < 1e-12);
        assert!((means[1] - 0.035).abs() < 1e-12);
        assert_eq!(out.cov_valid, [4, 4]);
        assert_eq!(out.cov_masked, [0, 0]);

        let row = &out.summary[0];
        assert_eq!(row.n_days, 2);
        assert!((row.ic_mean - 1.0).abs() < 1e-12);
        assert!((row.spread_mean - 0.02).abs() < 1e-12);
        assert!((row.monotonicity - 1.0).abs() < 1e-12);
        assert!((row.avg_coverage - 1.0).abs() < 1e-12);
        assert!((row.ic_win_rate - 1.0).abs() < 1e-12);
    }

    #[test]
    fn negated_factor_flips_ic_and_reverses_buckets() {
        let (ctx, factor) = perfect_panel();
        let negated: Vec<f64> = factor.iter().map(|v| -v).collect();

        let out = eval_factor(&ctx, &spec(2, 2, Binning::Daily), &factor);
        let neg = eval_factor(&ctx, &spec(2, 2, Binning::Daily), &negated);

        for day in 0..2 {
            assert!((out.ic[day] + neg.ic[day]).abs() < 1e-12);
            assert!((out.rank_ic[day] + neg.rank_ic[day]).abs() < 1e-12);
        }
        // Bucket means reverse: negated bin 1 holds the original top names.
        assert!((neg.q_mean[0][0] - out.q_mean[0][1]).abs() < 1e-12);
        assert!((neg.q_mean[0][1] - out.q_mean[0][0]).abs() < 1e-12);
        assert!((neg.summary[0].monotonicity + out.summary[0].monotonicity).abs() < 1e-12);
    }

    #[test]
    fn min_cs_count_gates_days_and_nan_factors_are_excluded() {
        let df = df!(
            "asset" => ["A", "B", "C", "D", "A", "B", "C", "D"],
            "time" => [1i64, 1, 1, 1, 2, 2, 2, 2],
            "ret_1" => [0.01, 0.02, 0.03, 0.04, 0.04, 0.03, 0.02, 0.01],
        )
        .unwrap();
        let ctx = context(&df, None);
        // Day 2 has only 3 valid samples; gate at 4 to skip it.
        let factor = vec![1.0, 2.0, 3.0, 4.0, f64::NAN, 3.0, 2.0, 1.0];

        let out = eval_factor(&ctx, &spec(2, 4, Binning::Daily), &factor);

        assert!((out.ic[0] - 1.0).abs() < 1e-12);
        assert!(out.ic[1].is_nan());
        assert_eq!(out.cov_valid, [4, 3]);
        // Quantile rows only for day 0.
        assert!(out.q_day.iter().all(|&d| d == 0));
        assert_eq!(out.summary[0].n_days, 1);
    }

    #[test]
    fn tradable_mask_removes_samples_and_counts_them() {
        let df = df!(
            "asset" => ["A", "B", "C", "D"],
            "time" => [1i64, 1, 1, 1],
            "ret_1" => [0.01, 0.02, 0.03, 0.40],
            "tradable" => [true, true, true, false],
        )
        .unwrap();
        let ctx = context(&df, Some("tradable"));
        let factor = vec![1.0, 2.0, 3.0, 100.0];

        let out = eval_factor(&ctx, &spec(3, 3, Binning::Daily), &factor);

        assert_eq!(out.cov_valid, [3]);
        assert_eq!(out.cov_masked, [1]);
        // The masked outlier (factor 100, ret 0.40) is fully absent: three
        // buckets of one sample each from the remaining names.
        assert_eq!(out.q_count, [1, 1, 1]);
        assert!((out.q_mean[0][2] - 0.03).abs() < 1e-12);
        assert!((out.ic[0] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn tied_factor_values_share_average_ranks() {
        // Factor ties across half the names; hand-computed Spearman.
        let df = df!(
            "asset" => ["A", "B", "C", "D"],
            "time" => [1i64, 1, 1, 1],
            "ret_1" => [0.01, 0.02, 0.03, 0.04],
        )
        .unwrap();
        let ctx = context(&df, None);
        let factor = vec![1.0, 1.0, 2.0, 2.0];

        let out = eval_factor(&ctx, &spec(2, 2, Binning::Daily), &factor);

        // Factor ranks: [1.5, 1.5, 3.5, 3.5]; label ranks [1, 2, 3, 4].
        // Pearson of those: sum dx*dy = 4, sum dx^2 = 4, sum dy^2 = 5.
        let expected = 4.0 / 20.0f64.sqrt();
        assert!((out.rank_ic[0] - expected).abs() < 1e-12);
    }

    #[test]
    fn global_binning_uses_pooled_cuts() {
        // Day 1 values low, day 2 values high: global median splits the days,
        // daily binning would split within each day.
        let df = df!(
            "asset" => ["A", "B", "A", "B"],
            "time" => [1i64, 1, 2, 2],
            "ret_1" => [0.01, 0.02, 0.03, 0.04],
        )
        .unwrap();
        let ctx = context(&df, None);
        let factor = vec![1.0, 2.0, 10.0, 20.0];

        let out = eval_factor(&ctx, &spec(2, 2, Binning::Global), &factor);

        // Pooled cuts: median of [1,2,10,20] = 6 -> day 1 all bin 1, day 2 all bin 2.
        assert_eq!(out.q_day, [0, 1]);
        assert_eq!(out.q_bin, [1, 2]);
        assert_eq!(out.q_count, [2, 2]);
        // Fixed bounds: bin 1 = [1, 6], bin 2 = [6, 20].
        assert_eq!(out.q_lo[0], 1.0);
        assert_eq!(out.q_hi[0], 6.0);
        assert_eq!(out.q_lo[1], 6.0);
        assert_eq!(out.q_hi[1], 20.0);
    }

    /// Many days force multiple parallel chunks; results must be identical to
    /// what the same per-day logic produces (validated via the day-0 pattern
    /// repeating and dense coverage).
    #[test]
    fn day_chunking_covers_all_days() {
        let n_days = DAY_CHUNK * 2 + 17;
        let mut asset = Vec::new();
        let mut time = Vec::new();
        let mut ret = Vec::new();
        let mut factor = Vec::new();
        for day in 0..n_days {
            for (i, name) in ["A", "B", "C"].iter().enumerate() {
                asset.push(*name);
                time.push(day as i64);
                ret.push(0.01 * (i as f64 + 1.0));
                factor.push(i as f64 + 1.0);
            }
        }
        let df = df!(
            "asset" => asset,
            "time" => time,
            "ret_1" => ret,
        )
        .unwrap();
        let ctx = context(&df, None);

        let out = eval_factor(&ctx, &spec(3, 3, Binning::Daily), &factor);

        assert_eq!(out.cov_valid.len(), n_days);
        assert!(out.cov_valid.iter().all(|&v| v == 3));
        assert_eq!(out.q_day.len(), n_days * 3);
        for day in 0..n_days {
            assert!((out.ic[day] - 1.0).abs() < 1e-12, "day {day}");
        }
        // Quantile rows are sorted by day.
        assert!(out.q_day.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(out.summary[0].n_days as usize, n_days);
    }
}

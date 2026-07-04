use std::collections::VecDeque;

use crate::context::{EvalContext, EvalSpec, Weighting};
use crate::metrics::GlobalBins;
use crate::stats::{mean_std, pearson_from_sums};

/// Trading days per year used for annualized summary statistics.
pub const ANNUALIZATION_DAYS: f64 = 252.0;

/// Rank-autocorrelation lags reported per factor.
pub const AUTOCORR_LAGS: [usize; 4] = [1, 5, 10, 20];

/// Cross-day flow metrics for one factor: quantile turnover, staggered
/// long-short portfolio, and rank autocorrelation. Dense `[h][day]` layout
/// like `FactorOutput`.
pub struct FlowsOutput {
    pub top_turnover: Vec<f64>,
    pub bottom_turnover: Vec<f64>,
    pub gross: Vec<f64>,
    pub net: Vec<f64>,
    pub turnover: Vec<f64>,
    /// `(lag, mean autocorrelation)`, one entry per AUTOCORR_LAGS lag < T.
    pub autocorr: Vec<(u32, f64)>,
    pub summary: Vec<FlowSummaryRow>,
}

/// Per-(factor, horizon) flow summary.
pub struct FlowSummaryRow {
    pub ls_gross_ann: f64,
    pub ls_net_ann: f64,
    pub ls_ir: f64,
    pub ls_turnover: f64,
    pub top_turnover: f64,
    pub bottom_turnover: f64,
}

/// One day's factor-side state, keyed by global symbol code (ascending).
struct DayState {
    valid: bool,
    /// Long-short weights (gross leverage 1) by symbol code.
    weights: Vec<(u32, f64)>,
    /// Average factor ranks by symbol code.
    ranks: Vec<(u32, f64)>,
    top: Vec<u32>,
    bottom: Vec<u32>,
}

impl DayState {
    fn invalid() -> Self {
        Self {
            valid: false,
            weights: Vec::new(),
            ranks: Vec::new(),
            top: Vec::new(),
            bottom: Vec::new(),
        }
    }
}

/// Per-horizon staggered portfolio accumulator: the rolling sum of the last
/// `h` signal days' weight vectors, plus the previous day's averaged weights.
struct PortfolioState {
    horizon: usize,
    /// Sparse weight vectors of the last <= h signal days.
    window: VecDeque<Vec<(u32, f64)>>,
    /// Dense sum over the window, indexed by symbol code.
    sum_w: Vec<f64>,
    /// Previous day's dense averaged weights (for turnover).
    prev_wbar: Option<Vec<f64>>,
}

/// Compute flow metrics for one factor. Sequential over days (cross-day
/// dependencies), so callers parallelize across factors instead.
///
/// The portfolio needs the 1-bar label: `ret1` is the TN vector of `ret_1`
/// when present; without it gross/net/turnover stay NaN. A position in a name
/// with no `ret_1` today contributes zero return (suspension approximation).
#[allow(clippy::too_many_arguments)]
pub fn eval_factor_flows(
    ctx: &EvalContext,
    spec: &EvalSpec,
    factor: &[f64],
    symbol_code_tn: &[u32],
    n_symbols: usize,
    ret1: Option<&[f64]>,
    global: Option<&GlobalBins>,
) -> FlowsOutput {
    let t_days = ctx.blocks.len();
    let n_h = ctx.horizons.len();
    let q = spec.quantiles;
    let day_gate = spec.min_cs_count.max(1);
    let max_h = ctx.horizons.iter().copied().max().unwrap_or(1);
    let max_lag = AUTOCORR_LAGS.iter().copied().max().unwrap_or(0);
    let history = max_h.max(max_lag) + 1;

    let mut out = FlowsOutput {
        top_turnover: vec![f64::NAN; n_h * t_days],
        bottom_turnover: vec![f64::NAN; n_h * t_days],
        gross: vec![f64::NAN; n_h * t_days],
        net: vec![f64::NAN; n_h * t_days],
        turnover: vec![f64::NAN; n_h * t_days],
        autocorr: Vec::new(),
        summary: Vec::new(),
    };

    // Ring buffer of recent day states, indexed day % history.
    let mut states: Vec<DayState> = (0..history).map(|_| DayState::invalid()).collect();
    let mut portfolios: Vec<PortfolioState> = ctx
        .horizons
        .iter()
        .map(|&h| PortfolioState {
            horizon: h,
            window: VecDeque::with_capacity(h + 1),
            sum_w: vec![0.0; n_symbols],
            prev_wbar: None,
        })
        .collect();
    let mut wbar = vec![0.0f64; n_symbols];
    let mut ret1_by_code = vec![f64::NAN; n_symbols];
    let mut autocorr_sum = vec![0.0f64; AUTOCORR_LAGS.len()];
    let mut autocorr_cnt = vec![0usize; AUTOCORR_LAGS.len()];
    let mut sorted: Vec<u32> = Vec::new();

    for day in 0..t_days {
        let block = &ctx.blocks[day];
        let base = block.start;

        sorted.clear();
        for offset in 0..block.len() {
            let idx = base + offset;
            if !factor[idx].is_nan() && ctx.tradable.as_ref().is_none_or(|t| t[idx]) {
                sorted.push(offset as u32);
            }
        }
        let m = sorted.len();
        let state = if m < day_gate {
            DayState::invalid()
        } else {
            sorted
                .sort_by(|&a, &b| factor[base + a as usize].total_cmp(&factor[base + b as usize]));
            build_day_state(factor, symbol_code_tn, base, &sorted, q, spec, global)
        };

        // Quantile turnover per horizon (vs the state h days ago).
        for (h_idx, &h) in ctx.horizons.iter().enumerate() {
            if day >= h {
                let past = &states[(day - h) % history];
                if state.valid && past.valid {
                    let slot = h_idx * t_days + day;
                    out.top_turnover[slot] = set_turnover(&state.top, &past.top);
                    out.bottom_turnover[slot] = set_turnover(&state.bottom, &past.bottom);
                }
            }
        }

        // Rank autocorrelation.
        for (lag_idx, &lag) in AUTOCORR_LAGS.iter().enumerate() {
            if day >= lag {
                let past = &states[(day - lag) % history];
                if state.valid && past.valid {
                    let value = rank_correlation(&state.ranks, &past.ranks);
                    if !value.is_nan() {
                        autocorr_sum[lag_idx] += value;
                        autocorr_cnt[lag_idx] += 1;
                    }
                }
            }
        }

        // Staggered long-short portfolio.
        if let Some(ret1) = ret1 {
            for (offset, &code) in symbol_code_tn[block.clone()].iter().enumerate() {
                ret1_by_code[code as usize] = ret1[base + offset];
            }
            for (h_idx, portfolio) in portfolios.iter_mut().enumerate() {
                portfolio.push_day(&state);
                let h_avail = portfolio.window.len();
                let slot = h_idx * t_days + day;
                if h_avail > 0 {
                    let scale = 1.0 / h_avail as f64;
                    let mut gross = 0.0;
                    for (code, wbar) in wbar.iter_mut().enumerate() {
                        *wbar = portfolio.sum_w[code] * scale;
                        if *wbar != 0.0 {
                            let r = ret1_by_code[code];
                            if !r.is_nan() {
                                gross += *wbar * r;
                            }
                        }
                    }
                    let turnover = portfolio.prev_wbar.as_ref().map(|prev| {
                        0.5 * wbar
                            .iter()
                            .zip(prev)
                            .map(|(now, was)| (now - was).abs())
                            .sum::<f64>()
                    });
                    out.gross[slot] = gross;
                    out.turnover[slot] = turnover.unwrap_or(f64::NAN);
                    out.net[slot] = gross - turnover.map_or(0.0, |t| t * spec.cost_bps * 1e-4);
                    match &mut portfolio.prev_wbar {
                        Some(prev) => prev.copy_from_slice(&wbar),
                        None => portfolio.prev_wbar = Some(wbar.clone()),
                    }
                }
            }
            for &code in &symbol_code_tn[block.clone()] {
                ret1_by_code[code as usize] = f64::NAN;
            }
        }

        states[day % history] = state;
    }

    for (lag_idx, &lag) in AUTOCORR_LAGS.iter().enumerate() {
        if lag < t_days {
            let mean = if autocorr_cnt[lag_idx] > 0 {
                autocorr_sum[lag_idx] / autocorr_cnt[lag_idx] as f64
            } else {
                f64::NAN
            };
            out.autocorr.push((lag as u32, mean));
        }
    }

    for h_idx in 0..n_h {
        let slice = |values: &[f64]| -> (f64, f64) {
            let series = &values[h_idx * t_days..(h_idx + 1) * t_days];
            let (mean, std, _) = mean_std(series);
            (mean, std)
        };
        let (gross_mean, _) = slice(&out.gross);
        let (net_mean, net_std) = slice(&out.net);
        let (turnover_mean, _) = slice(&out.turnover);
        let (top_mean, _) = slice(&out.top_turnover);
        let (bottom_mean, _) = slice(&out.bottom_turnover);
        out.summary.push(FlowSummaryRow {
            ls_gross_ann: gross_mean * ANNUALIZATION_DAYS,
            ls_net_ann: net_mean * ANNUALIZATION_DAYS,
            ls_ir: net_mean / net_std * ANNUALIZATION_DAYS.sqrt(),
            ls_turnover: turnover_mean,
            top_turnover: top_mean,
            bottom_turnover: bottom_mean,
        });
    }

    out
}

impl PortfolioState {
    fn push_day(&mut self, state: &DayState) {
        for &(code, weight) in &state.weights {
            self.sum_w[code as usize] += weight;
        }
        self.window.push_back(state.weights.clone());
        if self.window.len() > self.horizon {
            let expired = self.window.pop_front().expect("window non-empty");
            for (code, weight) in expired {
                self.sum_w[code as usize] -= weight;
            }
        }
    }
}

/// Build the (code-ascending) weights, ranks, and top/bottom sets for one day.
/// `sorted` holds factor-valid block offsets ascending by factor value.
fn build_day_state(
    factor: &[f64],
    symbol_code_tn: &[u32],
    base: usize,
    sorted: &[u32],
    q: usize,
    spec: &EvalSpec,
    global: Option<&GlobalBins>,
) -> DayState {
    let m = sorted.len();
    let mut top = Vec::new();
    let mut bottom = Vec::new();
    let mut top_sum = 0.0;
    let mut bottom_count = 0usize;

    // Bucket via the same rule as the metrics kernel.
    for (pos, &offset) in sorted.iter().enumerate() {
        let value = factor[base + offset as usize];
        let bucket = match global {
            None => pos * q / m,
            Some(bins) => bins.cuts.partition_point(|&cut| cut < value),
        };
        if bucket == 0 {
            bottom.push(symbol_code_tn[base + offset as usize]);
            bottom_count += 1;
        } else if bucket == q - 1 {
            top.push(symbol_code_tn[base + offset as usize]);
            top_sum += 1.0;
        }
    }
    top.sort_unstable();
    bottom.sort_unstable();

    // Ranks and weights in block-offset order = ascending code.
    let mut rank_by_offset =
        vec![0.0f64; sorted.iter().map(|&o| o as usize + 1).max().unwrap_or(0)];
    assign_ranks(
        sorted,
        |off| factor[base + off as usize],
        &mut rank_by_offset,
    );

    let mean = sorted
        .iter()
        .map(|&off| factor[base + off as usize])
        .sum::<f64>()
        / m as f64;
    let mut offsets: Vec<u32> = sorted.to_vec();
    offsets.sort_unstable();
    let mut weights = Vec::with_capacity(m);
    let mut ranks = Vec::with_capacity(m);
    let mut abs_sum = 0.0;
    for &offset in &offsets {
        let idx = base + offset as usize;
        let code = symbol_code_tn[idx];
        ranks.push((code, rank_by_offset[offset as usize]));
        let raw = match spec.weighting {
            Weighting::Factor => factor[idx] - mean,
            Weighting::Quantile => 0.0,
        };
        abs_sum += raw.abs();
        weights.push((code, raw));
    }
    match spec.weighting {
        Weighting::Factor => {
            if abs_sum > 0.0 {
                for (_, weight) in &mut weights {
                    *weight /= abs_sum;
                }
            } else {
                for (_, weight) in &mut weights {
                    *weight = 0.0;
                }
            }
        }
        Weighting::Quantile => {
            let top_w = if top_sum > 0.0 { 0.5 / top_sum } else { 0.0 };
            let bottom_w = if bottom_count > 0 {
                -0.5 / bottom_count as f64
            } else {
                0.0
            };
            for (code, weight) in &mut weights {
                *weight = if top.binary_search(code).is_ok() {
                    top_w
                } else if bottom.binary_search(code).is_ok() {
                    bottom_w
                } else {
                    0.0
                };
            }
            weights.retain(|&(_, w)| w != 0.0);
        }
    }

    DayState {
        valid: true,
        weights,
        ranks,
        top,
        bottom,
    }
}

/// Same average-rank assignment as the metrics kernel, but writing into an
/// offset-indexed scratch (only offsets present in `sorted` are read back).
fn assign_ranks(sorted: &[u32], value: impl Fn(u32) -> f64, out: &mut [f64]) {
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

/// Fraction of today's set that was not in the past set (both code-ascending).
fn set_turnover(current: &[u32], past: &[u32]) -> f64 {
    if current.is_empty() {
        return f64::NAN;
    }
    let mut common = 0usize;
    let mut i = 0;
    let mut j = 0;
    while i < current.len() && j < past.len() {
        match current[i].cmp(&past[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                common += 1;
                i += 1;
                j += 1;
            }
        }
    }
    1.0 - common as f64 / current.len() as f64
}

/// Pearson correlation of two (code, rank) lists over their common codes.
fn rank_correlation(current: &[(u32, f64)], past: &[(u32, f64)]) -> f64 {
    let (mut n, mut sx, mut sy, mut sxx, mut syy, mut sxy) = (0usize, 0.0, 0.0, 0.0, 0.0, 0.0);
    let mut i = 0;
    let mut j = 0;
    while i < current.len() && j < past.len() {
        match current[i].0.cmp(&past[j].0) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                let x = current[i].1;
                let y = past[j].1;
                n += 1;
                sx += x;
                sy += y;
                sxx += x * x;
                syy += y * y;
                sxy += x * y;
                i += 1;
                j += 1;
            }
        }
    }
    pearson_from_sums(n, sx, sy, sxx, syy, sxy)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use polars::prelude::*;
    use qfactors_core::PanelOptions;
    use qfactors_core::cellset::build_cellset;

    use super::*;
    use crate::context::{Binning, Demean, EvalContext};

    fn spec(quantiles: usize, weighting: Weighting, cost_bps: f64) -> EvalSpec {
        EvalSpec {
            quantiles,
            binning: Binning::Daily,
            demean: Demean::None,
            min_cs_count: 2,
            cost_bps,
            weighting,
        }
    }

    fn setup(df: &DataFrame) -> (EvalContext, Vec<u32>, usize) {
        let panel = PanelOptions {
            symbol_col: "asset".to_string(),
            time_col: "time".to_string(),
        };
        let cs = build_cellset(df, &panel, &BTreeSet::new()).unwrap();
        let ctx = EvalContext::build(
            df,
            &cs.time_blocks,
            &cs.orig_index_tn,
            &[("ret_1".to_string(), 1)],
            None,
            None,
            Demean::None,
        )
        .unwrap();
        // Codes by lexicographic order; panel fixtures are already TN-ordered
        // with symbols A.. ascending, so codes equal block offsets.
        let ti = crate::panel::build_time_index(df, &panel).unwrap();
        (ctx, ti.symbol_code_tn, ti.n_symbols)
    }

    /// 3 days x 4 assets, factor constant over time: after warm-up all
    /// turnover is zero and rank autocorrelation is exactly 1.
    #[test]
    fn static_factor_has_zero_turnover_and_unit_autocorr() {
        let df = df!(
            "asset" => ["A", "B", "C", "D", "A", "B", "C", "D", "A", "B", "C", "D"],
            "time" => [1i64, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3],
            "ret_1" => [0.01, 0.02, 0.03, 0.04, 0.01, 0.02, 0.03, 0.04, 0.01, 0.02, 0.03, 0.04],
        )
        .unwrap();
        let (ctx, codes, n_symbols) = setup(&df);
        let factor = vec![1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 3.0, 4.0];

        let out = eval_factor_flows(
            &ctx,
            &spec(2, Weighting::Factor, 0.0),
            &factor,
            &codes,
            n_symbols,
            Some(&ctx.labels[0].clone()),
            None,
        );

        // Day 0 has no h=1 comparison; days 1, 2 have zero turnover.
        assert!(out.top_turnover[0].is_nan());
        assert_eq!(out.top_turnover[1], 0.0);
        assert_eq!(out.bottom_turnover[2], 0.0);
        // Autocorrelation at lag 1 is exactly 1 (identical ranks).
        let lag1 = out.autocorr.iter().find(|(lag, _)| *lag == 1).unwrap();
        assert!((lag1.1 - 1.0).abs() < 1e-12);
        // Static weights: portfolio turnover 0 from day 1 on.
        assert!(out.turnover[0].is_nan());
        assert!(out.turnover[1].abs() < 1e-15);

        // Hand-check gross: weights = (f - 2.5)/sum|.| = [-1.5,-0.5,.5,1.5]/4.
        // gross = -0.375*0.01 - 0.125*0.02 + 0.125*0.03 + 0.375*0.04 = 0.0125.
        assert!((out.gross[0] - 0.0125).abs() < 1e-12);
        assert!((out.summary[0].ls_gross_ann - 0.0125 * ANNUALIZATION_DAYS).abs() < 1e-10);
    }

    /// Flipping the factor between days turns over both quantile sets fully.
    #[test]
    fn factor_flip_produces_full_turnover() {
        let df = df!(
            "asset" => ["A", "B", "C", "D", "A", "B", "C", "D"],
            "time" => [1i64, 1, 1, 1, 2, 2, 2, 2],
            "ret_1" => [0.01, 0.02, 0.03, 0.04, 0.01, 0.02, 0.03, 0.04],
        )
        .unwrap();
        let (ctx, codes, n_symbols) = setup(&df);
        let factor = vec![1.0, 2.0, 3.0, 4.0, 4.0, 3.0, 2.0, 1.0];

        let out = eval_factor_flows(
            &ctx,
            &spec(2, Weighting::Quantile, 0.0),
            &factor,
            &codes,
            n_symbols,
            None,
            None,
        );

        assert_eq!(out.top_turnover[1], 1.0);
        assert_eq!(out.bottom_turnover[1], 1.0);
        let lag1 = out.autocorr.iter().find(|(lag, _)| *lag == 1).unwrap();
        assert!((lag1.1 + 1.0).abs() < 1e-12);
        // No ret_1 passed: portfolio stays NaN.
        assert!(out.gross.iter().all(|v| v.is_nan()));
    }

    /// Quantile weighting: +-0.5/n on the extreme buckets, cost reduces net.
    #[test]
    fn quantile_weights_and_cost() {
        let df = df!(
            "asset" => ["A", "B", "C", "D", "A", "B", "C", "D"],
            "time" => [1i64, 1, 1, 1, 2, 2, 2, 2],
            "ret_1" => [0.01, 0.02, 0.03, 0.04, 0.01, 0.02, 0.03, 0.04],
        )
        .unwrap();
        let (ctx, codes, n_symbols) = setup(&df);
        // Day 2 flips: full portfolio turnover of 2 * 0.5 = ... hand-checked below.
        let factor = vec![1.0, 2.0, 3.0, 4.0, 4.0, 3.0, 2.0, 1.0];
        let ret1 = ctx.labels[0].clone();

        let out = eval_factor_flows(
            &ctx,
            &spec(2, Weighting::Quantile, 100.0),
            &factor,
            &codes,
            n_symbols,
            Some(&ret1),
            None,
        );

        // Day 1: long {C, D} at 0.25 each, short {A, B} at 0.25 each.
        let expected_gross = 0.25 * (0.03 + 0.04) - 0.25 * (0.01 + 0.02);
        assert!((out.gross[0] - expected_gross).abs() < 1e-12);
        // Day 2 weights flip sign entirely: 0.5 * sum|dw| = 0.5 * 4 * 0.5 = 1.
        assert!((out.turnover[1] - 1.0).abs() < 1e-12);
        // net = gross - 1.0 * 100bps.
        assert!((out.net[1] - (out.gross[1] - 0.01)).abs() < 1e-12);
    }

    /// h=2 staggering averages the last two days' weights.
    #[test]
    fn staggered_portfolio_averages_weights() {
        let df = df!(
            "asset" => ["A", "B", "A", "B", "A", "B"],
            "time" => [1i64, 1, 2, 2, 3, 3],
            "ret_1" => [0.01, 0.03, 0.01, 0.03, 0.01, 0.03],
            "ret_2" => [0.02, 0.06, 0.02, 0.06, 0.02, 0.06],
        )
        .unwrap();
        let panel = PanelOptions {
            symbol_col: "asset".to_string(),
            time_col: "time".to_string(),
        };
        let cs = build_cellset(&df, &panel, &BTreeSet::new()).unwrap();
        let ctx = EvalContext::build(
            &df,
            &cs.time_blocks,
            &cs.orig_index_tn,
            &[("ret_1".to_string(), 1), ("ret_2".to_string(), 2)],
            None,
            None,
            Demean::None,
        )
        .unwrap();
        let ti = crate::panel::build_time_index(&df, &panel).unwrap();
        // Day 1: A high, B low; day 2 flips; day 3 flips back.
        let factor = vec![2.0, 1.0, 1.0, 2.0, 2.0, 1.0];
        let ret1 = ctx.labels[0].clone();

        let out = eval_factor_flows(
            &ctx,
            &spec(2, Weighting::Factor, 0.0),
            &factor,
            &ti.symbol_code_tn,
            ti.n_symbols,
            Some(&ret1),
            None,
        );

        let t = 3;
        // h=1 (h_idx 0): day 2 gross = w(day2) . ret1 = (-0.5*0.01 + 0.5*0.03) flipped
        // day2 weights: A=-0.5, B=+0.5 -> gross = -0.5*0.01 + 0.5*0.03 = 0.01.
        assert!((out.gross[1] - 0.01).abs() < 1e-12);
        // h=2 (h_idx 1): day 2 wbar = mean(day1 w, day2 w) = 0 -> gross 0,
        // and day-2 turnover = 0.5*sum|wbar_2 - wbar_1| = 0.5*(|0-0.5|+|0+0.5|) = 0.5.
        assert!((out.gross[t + 1] - 0.0).abs() < 1e-12);
        assert!((out.turnover[t + 1] - 0.5).abs() < 1e-12);
    }
}

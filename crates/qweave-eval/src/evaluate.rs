use std::collections::BTreeSet;
use std::path::Path;

use polars::prelude::*;
use qweave_core::PanelOptions;
use qweave_core::compute_sink::{ComputeResult, ComputeSink};
use rayon::prelude::*;

use crate::context::{
    Binning, Demean, EvalContext, EvalSpec, Weighting, gather_f64_tn, parse_ret_horizon,
};
use crate::error::{EvalError, Result};
use crate::factor_source::{FactorSource, validate_source_factor_cols};
use crate::flows::{FlowsOutput, eval_factor_flows};
use crate::metrics::{FactorOutput, eval_factor_with, global_bins};
use crate::panel::build_time_index;
use crate::stats::civil_year_month;

/// Factors per batch: bounds transient memory (factor TN vectors + one batch of
/// output rows) in streaming mode while keeping rayon saturated.
const FACTOR_BATCH: usize = 64;

#[derive(Debug, Clone)]
pub struct EvaluateOptions {
    pub factor_cols: Vec<String>,
    pub label_cols: Option<Vec<String>>,
    pub quantiles: usize,
    pub binning: Binning,
    pub demean: Demean,
    pub min_cs_count: usize,
    pub group_col: Option<String>,
    pub tradable_col: Option<String>,
    pub cost_bps: f64,
    pub weighting: Weighting,
    /// Read factor columns from this parquet panel instead of from `df` (must
    /// cover the same (symbol, time) panel). Lets thousand-factor runs skip
    /// materializing the full wide input frame.
    pub factor_source: Option<String>,
    pub output_dir: Option<String>,
}

#[derive(Debug)]
pub enum TableData {
    Memory(DataFrame),
    File(String),
}

#[derive(Debug)]
pub struct EvalOutput {
    pub summary: DataFrame,
    pub ic: TableData,
    pub quantile_returns: TableData,
    pub coverage: TableData,
    pub turnover: TableData,
    pub portfolio: TableData,
    /// Time-mean factor rank autocorrelation per lag (small, always in memory).
    pub rank_autocorr: DataFrame,
    /// Only present when the time column is Date/Datetime.
    pub ic_monthly: Option<DataFrame>,
    pub meta_json: String,
}

pub fn evaluate(
    df: &DataFrame,
    panel: &PanelOptions,
    opts: &EvaluateOptions,
) -> Result<EvalOutput> {
    if opts.quantiles < 2 {
        return Err(EvalError::InvalidQuantiles(opts.quantiles));
    }
    let label_pairs = resolve_label_pairs(df, opts)?;
    let factor_source = match &opts.factor_source {
        None => {
            validate_factor_cols(df, opts, &label_pairs)?;
            None
        }
        Some(path) => {
            validate_source_factor_cols(&opts.factor_cols, &label_pairs)?;
            Some(FactorSource::open(path, panel, df, &opts.factor_cols)?)
        }
    };

    let ti = build_time_index(df, panel)?;
    let ctx = EvalContext::build(
        df,
        &ti.blocks,
        &ti.orig_index_tn,
        &label_pairs,
        opts.tradable_col.as_deref(),
        opts.group_col.as_deref(),
        opts.demean,
    )?;
    let spec = EvalSpec {
        quantiles: opts.quantiles,
        binning: opts.binning,
        demean: opts.demean,
        min_cs_count: opts.min_cs_count,
        cost_bps: opts.cost_bps,
        weighting: opts.weighting,
    };
    // The staggered long-short portfolio consumes the 1-bar label; without a
    // ret_1 column the portfolio table stays NaN.
    let ret1 = ctx
        .horizons
        .iter()
        .position(|&h| h == 1)
        .map(|idx| ctx.labels[idx].clone());

    let day_starts: Vec<IdxSize> = ti.blocks.iter().map(|r| r.start as IdxSize).collect();
    let dates = ti
        .times_tn
        .as_materialized_series()
        .take(&IdxCa::from_vec("idx".into(), day_starts))?
        .with_name("date".into());
    let month_keys = month_keys(&dates);
    let t_days = dates.len();

    if let Some(dir) = &opts.output_dir {
        std::fs::create_dir_all(dir)?;
    }
    let table_path = |table: &str| {
        opts.output_dir
            .as_ref()
            .map(|dir| Path::new(dir).join(format!("{table}.parquet")))
            .map(|p| p.to_string_lossy().into_owned())
    };
    let ic_path = table_path("ic");
    let quantile_path = table_path("quantile_returns");
    let coverage_path = table_path("coverage");
    let turnover_path = table_path("turnover");
    let portfolio_path = table_path("portfolio");
    let mut ic_sink = ComputeSink::for_output(ic_path.as_deref());
    let mut quantile_sink = ComputeSink::for_output(quantile_path.as_deref());
    let mut coverage_sink = ComputeSink::for_output(coverage_path.as_deref());
    let mut turnover_sink = ComputeSink::for_output(turnover_path.as_deref());
    let mut portfolio_sink = ComputeSink::for_output(portfolio_path.as_deref());

    let mut summary = SummaryColumns::default();
    let mut monthly = MonthlyColumns::default();
    let mut autocorr = AutocorrColumns::default();

    for batch in opts.factor_cols.chunks(FACTOR_BATCH) {
        let factors: Vec<Vec<f64>> = match &factor_source {
            Some(source) => source.read_batch(batch)?,
            None => batch
                .par_iter()
                .map(|name| gather_f64_tn(df, name, &ti.orig_index_tn))
                .collect::<Result<_>>()?,
        };
        let outputs: Vec<(FactorOutput, FlowsOutput)> = factors
            .par_iter()
            .map(|factor| {
                let global = match spec.binning {
                    Binning::Global => Some(global_bins(&ctx, factor, spec.quantiles)),
                    Binning::Daily => None,
                };
                let metrics = eval_factor_with(&ctx, &spec, factor, global.as_ref());
                let flows = eval_factor_flows(
                    &ctx,
                    &spec,
                    factor,
                    &ti.symbol_code_tn,
                    ti.n_symbols,
                    ret1.as_deref(),
                    global.as_ref(),
                );
                (metrics, flows)
            })
            .collect();
        drop(factors);
        let metrics: Vec<&FactorOutput> = outputs.iter().map(|(m, _)| m).collect();
        let flows: Vec<&FlowsOutput> = outputs.iter().map(|(_, f)| f).collect();

        ic_sink.write_observation(assemble_ic(&dates, batch, &ctx.horizons, &metrics)?)?;
        quantile_sink.write_observation(assemble_quantiles(
            &dates,
            batch,
            &ctx.horizons,
            &metrics,
        )?)?;
        coverage_sink.write_observation(assemble_coverage(&dates, batch, &metrics)?)?;
        turnover_sink.write_observation(assemble_turnover(
            &dates,
            batch,
            &ctx.horizons,
            &flows,
        )?)?;
        portfolio_sink.write_observation(assemble_portfolio(
            &dates,
            batch,
            &ctx.horizons,
            &flows,
        )?)?;

        for ((name, metric), flow) in batch.iter().zip(&metrics).zip(&flows) {
            summary.push_factor(name, &ctx.horizons, metric, flow);
            autocorr.push_factor(name, flow);
            if let Some(keys) = &month_keys {
                monthly.push_factor(name, &ctx.horizons, metric, keys, t_days);
            }
        }
    }

    let summary = summary.into_frame()?;
    let rank_autocorr = autocorr.into_frame()?;
    let ic_monthly = month_keys.map(|_| monthly.into_frame()).transpose()?;
    let meta_json = meta_json(panel, opts, &label_pairs, t_days, df.height());

    if let Some(dir) = &opts.output_dir {
        write_parquet(
            &mut summary.clone(),
            &Path::new(dir).join("summary.parquet"),
        )?;
        write_parquet(
            &mut rank_autocorr.clone(),
            &Path::new(dir).join("rank_autocorr.parquet"),
        )?;
        if let Some(monthly) = &ic_monthly {
            write_parquet(
                &mut monthly.clone(),
                &Path::new(dir).join("ic_monthly.parquet"),
            )?;
        }
        std::fs::write(Path::new(dir).join("meta.json"), &meta_json)?;
    }

    Ok(EvalOutput {
        summary,
        ic: finish_sink(ic_sink)?,
        quantile_returns: finish_sink(quantile_sink)?,
        coverage: finish_sink(coverage_sink)?,
        turnover: finish_sink(turnover_sink)?,
        portfolio: finish_sink(portfolio_sink)?,
        rank_autocorr,
        ic_monthly,
        meta_json,
    })
}

/// Persist an in-memory result under the `save()`/`output_dir` contract:
/// one parquet per table plus `meta.json`.
pub fn save_output(output: &EvalOutput, dir: &str) -> Result<()> {
    let tables = [
        ("ic", &output.ic),
        ("quantile_returns", &output.quantile_returns),
        ("coverage", &output.coverage),
        ("turnover", &output.turnover),
        ("portfolio", &output.portfolio),
    ];
    for (_, table) in &tables {
        if let TableData::File(path) = table {
            let streamed = Path::new(path)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string());
            return Err(EvalError::AlreadySaved(streamed));
        }
    }
    std::fs::create_dir_all(dir)?;
    let dir = Path::new(dir);
    write_parquet(&mut output.summary.clone(), &dir.join("summary.parquet"))?;
    write_parquet(
        &mut output.rank_autocorr.clone(),
        &dir.join("rank_autocorr.parquet"),
    )?;
    for (name, table) in tables {
        let TableData::Memory(df) = table else {
            unreachable!("file mode rejected above");
        };
        write_parquet(&mut df.clone(), &dir.join(format!("{name}.parquet")))?;
    }
    if let Some(monthly) = &output.ic_monthly {
        write_parquet(&mut monthly.clone(), &dir.join("ic_monthly.parquet"))?;
    }
    std::fs::write(dir.join("meta.json"), &output.meta_json)?;
    Ok(())
}

fn write_parquet(df: &mut DataFrame, path: &Path) -> Result<()> {
    let file = std::fs::File::create(path)?;
    ParquetWriter::new(std::io::BufWriter::new(file)).finish(df)?;
    Ok(())
}

fn finish_sink(sink: ComputeSink) -> Result<TableData> {
    Ok(match sink.finish()? {
        ComputeResult::Memory(df) => TableData::Memory(df),
        ComputeResult::File(summary) => TableData::File(summary.output_path),
    })
}

fn resolve_label_pairs(df: &DataFrame, opts: &EvaluateOptions) -> Result<Vec<(String, usize)>> {
    let mut pairs: Vec<(String, usize)> = match &opts.label_cols {
        Some(cols) => cols
            .iter()
            .map(|name| {
                parse_ret_horizon(name)
                    .map(|h| (name.clone(), h))
                    .ok_or_else(|| EvalError::BadLabelColumn(name.clone()))
            })
            .collect::<Result<_>>()?,
        None => df
            .get_column_names()
            .iter()
            .filter_map(|name| parse_ret_horizon(name).map(|h| (name.to_string(), h)))
            .collect(),
    };
    if pairs.is_empty() {
        return Err(EvalError::NoLabelColumns);
    }
    pairs.sort_by_key(|(_, h)| *h);
    for window in pairs.windows(2) {
        if window[0].1 == window[1].1 {
            return Err(EvalError::BadLabelColumn(window[1].0.clone()));
        }
    }
    Ok(pairs)
}

fn validate_factor_cols(
    df: &DataFrame,
    opts: &EvaluateOptions,
    label_pairs: &[(String, usize)],
) -> Result<()> {
    if opts.factor_cols.is_empty() {
        return Err(EvalError::BadFactorColumns("<empty>".to_string()));
    }
    let mut seen = BTreeSet::new();
    for name in &opts.factor_cols {
        if !seen.insert(name.as_str()) || label_pairs.iter().any(|(label, _)| label == name) {
            return Err(EvalError::BadFactorColumns(name.clone()));
        }
        let column = df
            .column(name)
            .map_err(|_| EvalError::Core(qweave_core::QWeaveError::MissingColumn(name.clone())))?;
        if column.dtype() != &DataType::Float64 {
            return Err(EvalError::DTypeMismatch {
                column: name.clone(),
                expected: "f64",
                actual: column.dtype().to_string(),
            });
        }
    }
    Ok(())
}

fn month_keys(dates: &Series) -> Option<Vec<(i32, u32)>> {
    let days: Vec<i64> = match dates.dtype() {
        DataType::Date => dates
            .to_physical_repr()
            .i32()
            .ok()?
            .into_no_null_iter()
            .map(|d| d as i64)
            .collect(),
        DataType::Datetime(unit, _) => {
            let per_day = match unit {
                TimeUnit::Milliseconds => 86_400_000i64,
                TimeUnit::Microseconds => 86_400_000_000,
                TimeUnit::Nanoseconds => 86_400_000_000_000,
            };
            dates
                .to_physical_repr()
                .i64()
                .ok()?
                .into_no_null_iter()
                .map(|ts| ts.div_euclid(per_day))
                .collect()
        }
        // Integer/string columns holding a packed `YYYYMMDD` calendar date
        // (e.g. 20240402) rather than epoch days. Parse the year/month directly;
        // a value that is not a plausible calendar date yields `None`, which
        // safely disables the heatmap instead of emitting garbage.
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => {
            return dates
                .cast(&DataType::Int64)
                .ok()?
                .i64()
                .ok()?
                .iter()
                .map(|v| v.and_then(ymd_month_from_packed))
                .collect();
        }
        DataType::String => {
            return dates
                .str()
                .ok()?
                .iter()
                .map(|v| v.and_then(ymd_month_from_str))
                .collect();
        }
        _ => return None,
    };
    Some(days.into_iter().map(civil_year_month).collect())
}

/// Year/month from a packed `YYYYMMDD` integer, or `None` when it is not a
/// plausible calendar date (so non-date integer columns disable the heatmap).
fn ymd_month_from_packed(v: i64) -> Option<(i32, u32)> {
    let year = (v / 10_000) as i32;
    let month = ((v / 100) % 100) as u32;
    ((1000..=9999).contains(&year) && (1..=12).contains(&month)).then_some((year, month))
}

/// Year/month from a `YYYYMMDD` or `YYYY-MM-DD` string (any non-digit
/// separators are ignored); `None` when it is not a plausible calendar date.
fn ymd_month_from_str(s: &str) -> Option<(i32, u32)> {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 6 {
        return None;
    }
    let year = digits[0..4].parse::<i32>().ok()?;
    let month = digits[4..6].parse::<u32>().ok()?;
    ((1000..=9999).contains(&year) && (1..=12).contains(&month)).then_some((year, month))
}

fn assemble_ic(
    dates: &Series,
    batch: &[String],
    horizons: &[usize],
    outputs: &[&FactorOutput],
) -> Result<DataFrame> {
    let t = dates.len();
    let rows = batch.len() * horizons.len() * t;
    let mut date_idx: Vec<IdxSize> = Vec::with_capacity(rows);
    let mut factor: Vec<&str> = Vec::with_capacity(rows);
    let mut horizon: Vec<u32> = Vec::with_capacity(rows);
    let mut ic: Vec<f64> = Vec::with_capacity(rows);
    let mut rank_ic: Vec<f64> = Vec::with_capacity(rows);
    for (name, output) in batch.iter().zip(outputs) {
        for (h_idx, &h) in horizons.iter().enumerate() {
            date_idx.extend(0..t as IdxSize);
            factor.extend(std::iter::repeat_n(name.as_str(), t));
            horizon.extend(std::iter::repeat_n(h as u32, t));
            ic.extend_from_slice(&output.ic[h_idx * t..(h_idx + 1) * t]);
            rank_ic.extend_from_slice(&output.rank_ic[h_idx * t..(h_idx + 1) * t]);
        }
    }
    let date = dates.take(&IdxCa::from_vec("idx".into(), date_idx))?;
    DataFrame::new_infer_height(vec![
        date.into_column(),
        Column::new("factor".into(), factor),
        Column::new("horizon".into(), horizon),
        Column::new("ic".into(), ic),
        Column::new("rank_ic".into(), rank_ic),
    ])
    .map_err(EvalError::from)
}

fn assemble_quantiles(
    dates: &Series,
    batch: &[String],
    horizons: &[usize],
    outputs: &[&FactorOutput],
) -> Result<DataFrame> {
    let rows: usize = outputs.iter().map(|o| o.q_day.len()).sum();
    let mut date_idx: Vec<IdxSize> = Vec::with_capacity(rows);
    let mut factor: Vec<&str> = Vec::with_capacity(rows);
    let mut bin: Vec<u32> = Vec::with_capacity(rows);
    let mut bin_lo: Vec<f64> = Vec::with_capacity(rows);
    let mut bin_hi: Vec<f64> = Vec::with_capacity(rows);
    let mut count: Vec<u32> = Vec::with_capacity(rows);
    let mut means: Vec<Vec<f64>> = vec![Vec::with_capacity(rows); horizons.len()];
    for (name, output) in batch.iter().zip(outputs) {
        date_idx.extend(output.q_day.iter().map(|&d| d as IdxSize));
        factor.extend(std::iter::repeat_n(name.as_str(), output.q_day.len()));
        bin.extend_from_slice(&output.q_bin);
        bin_lo.extend_from_slice(&output.q_lo);
        bin_hi.extend_from_slice(&output.q_hi);
        count.extend_from_slice(&output.q_count);
        for (h_idx, mean) in means.iter_mut().enumerate() {
            mean.extend_from_slice(&output.q_mean[h_idx]);
        }
    }
    let date = dates.take(&IdxCa::from_vec("idx".into(), date_idx))?;
    let mut columns = vec![
        date.into_column(),
        Column::new("factor".into(), factor),
        Column::new("bin".into(), bin),
        Column::new("bin_lo".into(), bin_lo),
        Column::new("bin_hi".into(), bin_hi),
        Column::new("count".into(), count),
    ];
    for (&h, mean) in horizons.iter().zip(means) {
        columns.push(Column::new(format!("mean_ret_{h}").into(), mean));
    }
    DataFrame::new_infer_height(columns).map_err(EvalError::from)
}

fn assemble_coverage(
    dates: &Series,
    batch: &[String],
    outputs: &[&FactorOutput],
) -> Result<DataFrame> {
    let t = dates.len();
    let rows = batch.len() * t;
    let mut date_idx: Vec<IdxSize> = Vec::with_capacity(rows);
    let mut factor: Vec<&str> = Vec::with_capacity(rows);
    let mut n_valid: Vec<u32> = Vec::with_capacity(rows);
    let mut n_masked: Vec<u32> = Vec::with_capacity(rows);
    for (name, output) in batch.iter().zip(outputs) {
        date_idx.extend(0..t as IdxSize);
        factor.extend(std::iter::repeat_n(name.as_str(), t));
        n_valid.extend_from_slice(&output.cov_valid);
        n_masked.extend_from_slice(&output.cov_masked);
    }
    let date = dates.take(&IdxCa::from_vec("idx".into(), date_idx))?;
    DataFrame::new_infer_height(vec![
        date.into_column(),
        Column::new("factor".into(), factor),
        Column::new("n_valid".into(), n_valid),
        Column::new("n_masked".into(), n_masked),
    ])
    .map_err(EvalError::from)
}

#[derive(Default)]
struct SummaryColumns {
    factor: Vec<String>,
    horizon: Vec<u32>,
    n_days: Vec<u32>,
    ic_mean: Vec<f64>,
    ic_std: Vec<f64>,
    ic_ir: Vec<f64>,
    ic_t_nw: Vec<f64>,
    ic_win_rate: Vec<f64>,
    rank_ic_mean: Vec<f64>,
    rank_ic_std: Vec<f64>,
    rank_ic_ir: Vec<f64>,
    rank_ic_t_nw: Vec<f64>,
    rank_ic_win_rate: Vec<f64>,
    spread_mean: Vec<f64>,
    spread_t_nw: Vec<f64>,
    monotonicity: Vec<f64>,
    avg_coverage: Vec<f64>,
    ls_gross_ann: Vec<f64>,
    ls_net_ann: Vec<f64>,
    ls_ir: Vec<f64>,
    ls_turnover: Vec<f64>,
    top_turnover: Vec<f64>,
    bottom_turnover: Vec<f64>,
}

impl SummaryColumns {
    fn push_factor(
        &mut self,
        name: &str,
        horizons: &[usize],
        output: &FactorOutput,
        flows: &FlowsOutput,
    ) {
        for ((&h, row), flow) in horizons.iter().zip(&output.summary).zip(&flows.summary) {
            self.factor.push(name.to_string());
            self.horizon.push(h as u32);
            self.n_days.push(row.n_days);
            self.ic_mean.push(row.ic_mean);
            self.ic_std.push(row.ic_std);
            self.ic_ir.push(row.ic_ir);
            self.ic_t_nw.push(row.ic_t_nw);
            self.ic_win_rate.push(row.ic_win_rate);
            self.rank_ic_mean.push(row.rank_ic_mean);
            self.rank_ic_std.push(row.rank_ic_std);
            self.rank_ic_ir.push(row.rank_ic_ir);
            self.rank_ic_t_nw.push(row.rank_ic_t_nw);
            self.rank_ic_win_rate.push(row.rank_ic_win_rate);
            self.spread_mean.push(row.spread_mean);
            self.spread_t_nw.push(row.spread_t_nw);
            self.monotonicity.push(row.monotonicity);
            self.avg_coverage.push(row.avg_coverage);
            self.ls_gross_ann.push(flow.ls_gross_ann);
            self.ls_net_ann.push(flow.ls_net_ann);
            self.ls_ir.push(flow.ls_ir);
            self.ls_turnover.push(flow.ls_turnover);
            self.top_turnover.push(flow.top_turnover);
            self.bottom_turnover.push(flow.bottom_turnover);
        }
    }

    fn into_frame(self) -> Result<DataFrame> {
        DataFrame::new_infer_height(vec![
            Column::new("factor".into(), self.factor),
            Column::new("horizon".into(), self.horizon),
            Column::new("n_days".into(), self.n_days),
            Column::new("ic_mean".into(), self.ic_mean),
            Column::new("ic_std".into(), self.ic_std),
            Column::new("ic_ir".into(), self.ic_ir),
            Column::new("ic_t_nw".into(), self.ic_t_nw),
            Column::new("ic_win_rate".into(), self.ic_win_rate),
            Column::new("rank_ic_mean".into(), self.rank_ic_mean),
            Column::new("rank_ic_std".into(), self.rank_ic_std),
            Column::new("rank_ic_ir".into(), self.rank_ic_ir),
            Column::new("rank_ic_t_nw".into(), self.rank_ic_t_nw),
            Column::new("rank_ic_win_rate".into(), self.rank_ic_win_rate),
            Column::new("spread_mean".into(), self.spread_mean),
            Column::new("spread_t_nw".into(), self.spread_t_nw),
            Column::new("monotonicity".into(), self.monotonicity),
            Column::new("avg_coverage".into(), self.avg_coverage),
            Column::new("ls_gross_ann".into(), self.ls_gross_ann),
            Column::new("ls_net_ann".into(), self.ls_net_ann),
            Column::new("ls_ir".into(), self.ls_ir),
            Column::new("ls_turnover".into(), self.ls_turnover),
            Column::new("top_turnover".into(), self.top_turnover),
            Column::new("bottom_turnover".into(), self.bottom_turnover),
        ])
        .map_err(EvalError::from)
    }
}

/// Dense per-(factor, horizon, day) tables built from the flows pass: quantile
/// turnover and the staggered long-short portfolio.
fn assemble_turnover(
    dates: &Series,
    batch: &[String],
    horizons: &[usize],
    outputs: &[&FlowsOutput],
) -> Result<DataFrame> {
    let t = dates.len();
    let rows = batch.len() * horizons.len() * t;
    let mut date_idx: Vec<IdxSize> = Vec::with_capacity(rows);
    let mut factor: Vec<&str> = Vec::with_capacity(rows);
    let mut horizon: Vec<u32> = Vec::with_capacity(rows);
    let mut top: Vec<f64> = Vec::with_capacity(rows);
    let mut bottom: Vec<f64> = Vec::with_capacity(rows);
    for (name, output) in batch.iter().zip(outputs) {
        for (h_idx, &h) in horizons.iter().enumerate() {
            date_idx.extend(0..t as IdxSize);
            factor.extend(std::iter::repeat_n(name.as_str(), t));
            horizon.extend(std::iter::repeat_n(h as u32, t));
            top.extend_from_slice(&output.top_turnover[h_idx * t..(h_idx + 1) * t]);
            bottom.extend_from_slice(&output.bottom_turnover[h_idx * t..(h_idx + 1) * t]);
        }
    }
    let date = dates.take(&IdxCa::from_vec("idx".into(), date_idx))?;
    DataFrame::new_infer_height(vec![
        date.into_column(),
        Column::new("factor".into(), factor),
        Column::new("horizon".into(), horizon),
        Column::new("top_turnover".into(), top),
        Column::new("bottom_turnover".into(), bottom),
    ])
    .map_err(EvalError::from)
}

fn assemble_portfolio(
    dates: &Series,
    batch: &[String],
    horizons: &[usize],
    outputs: &[&FlowsOutput],
) -> Result<DataFrame> {
    let t = dates.len();
    let rows = batch.len() * horizons.len() * t;
    let mut date_idx: Vec<IdxSize> = Vec::with_capacity(rows);
    let mut factor: Vec<&str> = Vec::with_capacity(rows);
    let mut horizon: Vec<u32> = Vec::with_capacity(rows);
    let mut gross: Vec<f64> = Vec::with_capacity(rows);
    let mut net: Vec<f64> = Vec::with_capacity(rows);
    let mut turnover: Vec<f64> = Vec::with_capacity(rows);
    for (name, output) in batch.iter().zip(outputs) {
        for (h_idx, &h) in horizons.iter().enumerate() {
            date_idx.extend(0..t as IdxSize);
            factor.extend(std::iter::repeat_n(name.as_str(), t));
            horizon.extend(std::iter::repeat_n(h as u32, t));
            gross.extend_from_slice(&output.gross[h_idx * t..(h_idx + 1) * t]);
            net.extend_from_slice(&output.net[h_idx * t..(h_idx + 1) * t]);
            turnover.extend_from_slice(&output.turnover[h_idx * t..(h_idx + 1) * t]);
        }
    }
    let date = dates.take(&IdxCa::from_vec("idx".into(), date_idx))?;
    DataFrame::new_infer_height(vec![
        date.into_column(),
        Column::new("factor".into(), factor),
        Column::new("horizon".into(), horizon),
        Column::new("gross".into(), gross),
        Column::new("net".into(), net),
        Column::new("turnover".into(), turnover),
    ])
    .map_err(EvalError::from)
}

#[derive(Default)]
struct AutocorrColumns {
    factor: Vec<String>,
    lag: Vec<u32>,
    autocorr: Vec<f64>,
}

impl AutocorrColumns {
    fn push_factor(&mut self, name: &str, flows: &FlowsOutput) {
        for &(lag, value) in &flows.autocorr {
            self.factor.push(name.to_string());
            self.lag.push(lag);
            self.autocorr.push(value);
        }
    }

    fn into_frame(self) -> Result<DataFrame> {
        DataFrame::new_infer_height(vec![
            Column::new("factor".into(), self.factor),
            Column::new("lag".into(), self.lag),
            Column::new("rank_autocorr".into(), self.autocorr),
        ])
        .map_err(EvalError::from)
    }
}

#[derive(Default)]
struct MonthlyColumns {
    year: Vec<i32>,
    month: Vec<u32>,
    factor: Vec<String>,
    horizon: Vec<u32>,
    ic_mean: Vec<f64>,
    rank_ic_mean: Vec<f64>,
}

impl MonthlyColumns {
    fn push_factor(
        &mut self,
        name: &str,
        horizons: &[usize],
        output: &FactorOutput,
        keys: &[(i32, u32)],
        t_days: usize,
    ) {
        for (h_idx, &h) in horizons.iter().enumerate() {
            let ic = &output.ic[h_idx * t_days..(h_idx + 1) * t_days];
            let rank_ic = &output.rank_ic[h_idx * t_days..(h_idx + 1) * t_days];
            let mut day = 0;
            while day < t_days {
                let key = keys[day];
                let mut end = day + 1;
                while end < t_days && keys[end] == key {
                    end += 1;
                }
                let mean_of = |series: &[f64]| {
                    let mut n = 0usize;
                    let mut sum = 0.0;
                    for &value in &series[day..end] {
                        if !value.is_nan() {
                            n += 1;
                            sum += value;
                        }
                    }
                    (n, sum)
                };
                let (ic_n, ic_sum) = mean_of(ic);
                let (rank_n, rank_sum) = mean_of(rank_ic);
                if ic_n > 0 || rank_n > 0 {
                    self.year.push(key.0);
                    self.month.push(key.1);
                    self.factor.push(name.to_string());
                    self.horizon.push(h as u32);
                    self.ic_mean.push(if ic_n > 0 {
                        ic_sum / ic_n as f64
                    } else {
                        f64::NAN
                    });
                    self.rank_ic_mean.push(if rank_n > 0 {
                        rank_sum / rank_n as f64
                    } else {
                        f64::NAN
                    });
                }
                day = end;
            }
        }
    }

    fn into_frame(self) -> Result<DataFrame> {
        DataFrame::new_infer_height(vec![
            Column::new("year".into(), self.year),
            Column::new("month".into(), self.month),
            Column::new("factor".into(), self.factor),
            Column::new("horizon".into(), self.horizon),
            Column::new("ic_mean".into(), self.ic_mean),
            Column::new("rank_ic_mean".into(), self.rank_ic_mean),
        ])
        .map_err(EvalError::from)
    }
}

fn meta_json(
    panel: &PanelOptions,
    opts: &EvaluateOptions,
    label_pairs: &[(String, usize)],
    n_days: usize,
    n_rows: usize,
) -> String {
    let binning = match opts.binning {
        Binning::Daily => "daily",
        Binning::Global => "global",
    };
    let demean = match opts.demean {
        Demean::None => "none",
        Demean::Universe => "universe",
        Demean::Group => "group",
    };
    let horizons = label_pairs
        .iter()
        .map(|(_, h)| h.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let label_cols = label_pairs
        .iter()
        .map(|(name, _)| json_string(name))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        concat!(
            "{{\"symbol_col\":{},\"time_col\":{},\"quantiles\":{},\"binning\":\"{}\",",
            "\"demean\":\"{}\",\"min_cs_count\":{},\"cost_bps\":{},\"weighting\":\"{}\",",
            "\"horizons\":[{}],\"label_cols\":[{}],",
            "\"factor_count\":{},\"group_col\":{},\"tradable_col\":{},\"n_days\":{},",
            "\"n_rows\":{},\"factor_source\":{},\"output_dir\":{}}}"
        ),
        json_string(&panel.symbol_col),
        json_string(&panel.time_col),
        opts.quantiles,
        binning,
        demean,
        opts.min_cs_count,
        opts.cost_bps,
        match opts.weighting {
            Weighting::Factor => "factor",
            Weighting::Quantile => "quantile",
        },
        horizons,
        label_cols,
        opts.factor_cols.len(),
        json_option(opts.group_col.as_deref()),
        json_option(opts.tradable_col.as_deref()),
        n_days,
        n_rows,
        json_option(opts.factor_source.as_deref()),
        json_option(opts.output_dir.as_deref()),
    )
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_option(value: Option<&str>) -> String {
    match value {
        Some(value) => json_string(value),
        None => "null".to_string(),
    }
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

    fn options(factor_cols: Vec<String>) -> EvaluateOptions {
        EvaluateOptions {
            factor_cols,
            label_cols: None,
            quantiles: 2,
            binning: Binning::Daily,
            demean: Demean::None,
            min_cs_count: 2,
            group_col: None,
            tradable_col: None,
            cost_bps: 0.0,
            weighting: Weighting::Factor,
            factor_source: None,
            output_dir: None,
        }
    }

    fn sample_df() -> DataFrame {
        df!(
            "asset" => ["A", "B", "C", "D", "A", "B", "C", "D"],
            "time" => [1i64, 1, 1, 1, 2, 2, 2, 2],
            "f1" => [1.0, 2.0, 3.0, 4.0, 4.0, 3.0, 2.0, 1.0],
            "ret_1" => [0.01, 0.02, 0.03, 0.04, 0.04, 0.03, 0.02, 0.01],
            "ret_2" => [0.02, 0.04, 0.06, 0.08, 0.08, 0.06, 0.04, 0.02],
        )
        .unwrap()
    }

    #[test]
    fn evaluate_end_to_end_memory_mode() -> Result<()> {
        let df = sample_df();

        let out = evaluate(&df, &panel(), &options(vec!["f1".to_string()]))?;

        assert_eq!(out.summary.height(), 2); // 1 factor x 2 horizons
        let ic_mean = out.summary.column("ic_mean").unwrap().try_f64().unwrap();
        assert!((ic_mean.get(0).unwrap() - 1.0).abs() < 1e-12);
        let TableData::Memory(ic) = &out.ic else {
            panic!("expected memory table");
        };
        assert_eq!(ic.height(), 4); // 1 factor x 2 horizons x 2 days
        assert_eq!(
            ic.get_column_names()
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
            ["date", "factor", "horizon", "ic", "rank_ic"]
        );
        let TableData::Memory(quantiles) = &out.quantile_returns else {
            panic!("expected memory table");
        };
        assert_eq!(quantiles.height(), 4); // 2 days x 2 bins
        assert!(
            quantiles
                .get_column_names()
                .iter()
                .any(|c| c.as_str() == "mean_ret_2")
        );
        // Integer time column: no monthly table.
        assert!(out.ic_monthly.is_none());
        assert!(out.meta_json.contains("\"horizons\":[1,2]"));
        Ok(())
    }

    #[test]
    fn evaluate_streaming_matches_memory() -> Result<()> {
        let df = sample_df();
        let dir = std::env::temp_dir().join(format!("qweave-eval-test-{}", std::process::id()));
        let dir_string = dir.to_string_lossy().into_owned();

        let memory = evaluate(&df, &panel(), &options(vec!["f1".to_string()]))?;
        let mut streamed_opts = options(vec!["f1".to_string()]);
        streamed_opts.output_dir = Some(dir_string.clone());
        let streamed = evaluate(&df, &panel(), &streamed_opts)?;

        assert!(streamed.summary.equals_missing(&memory.summary));
        let TableData::File(ic_path) = &streamed.ic else {
            panic!("expected file table");
        };
        let ic_file = ParquetReader::new(std::fs::File::open(ic_path).unwrap()).finish()?;
        let TableData::Memory(ic_memory) = &memory.ic else {
            panic!("expected memory table");
        };
        assert!(ic_file.equals_missing(ic_memory));
        assert!(dir.join("summary.parquet").exists());
        assert!(dir.join("meta.json").exists());

        let err = save_output(&streamed, &dir_string).unwrap_err();
        assert!(matches!(err, EvalError::AlreadySaved(_)));

        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }

    #[test]
    fn evaluate_factor_source_matches_in_frame() -> Result<()> {
        let df = sample_df();
        // Write the factor as a separate parquet panel, deliberately unsorted,
        // and drop it from the frame passed to evaluate.
        let mut factor_panel = df!(
            "time" => [2i64, 1, 1, 2, 1, 2, 1, 2],
            "asset" => ["D", "C", "A", "A", "B", "B", "D", "C"],
            "f1" => [1.0, 3.0, 1.0, 4.0, 2.0, 3.0, 4.0, 2.0],
        )?;
        let path =
            std::env::temp_dir().join(format!("qweave-eval-source-{}.parquet", std::process::id()));
        ParquetWriter::new(std::fs::File::create(&path)?).finish(&mut factor_panel)?;
        let path_string = path.to_string_lossy().into_owned();

        let in_frame = evaluate(&df, &panel(), &options(vec!["f1".to_string()]))?;
        let df_without_factor = df.drop("f1")?;
        let mut source_opts = options(vec!["f1".to_string()]);
        source_opts.factor_source = Some(path_string);
        let from_source = evaluate(&df_without_factor, &panel(), &source_opts)?;

        assert!(from_source.summary.equals_missing(&in_frame.summary));
        let (TableData::Memory(a), TableData::Memory(b)) =
            (&from_source.quantile_returns, &in_frame.quantile_returns)
        else {
            panic!("expected memory tables");
        };
        assert!(a.equals_missing(b));

        // A mismatched panel is rejected.
        let wrong_panel = df_without_factor.slice(0, 4);
        source_opts.factor_source = Some(path.to_string_lossy().into_owned());
        let err = evaluate(&wrong_panel, &panel(), &source_opts).unwrap_err();
        assert!(matches!(err, EvalError::FactorSourcePanelMismatch));

        std::fs::remove_file(&path).ok();
        Ok(())
    }

    #[test]
    fn evaluate_rejects_bad_inputs() {
        let df = sample_df();

        let err = evaluate(&df, &panel(), &options(vec![])).unwrap_err();
        assert!(matches!(err, EvalError::BadFactorColumns(_)));

        let err = evaluate(
            &df,
            &panel(),
            &options(vec!["f1".to_string(), "f1".to_string()]),
        )
        .unwrap_err();
        assert!(matches!(err, EvalError::BadFactorColumns(_)));

        let err = evaluate(&df, &panel(), &options(vec!["ret_1".to_string()])).unwrap_err();
        assert!(matches!(err, EvalError::BadFactorColumns(_)));

        let mut opts = options(vec!["f1".to_string()]);
        opts.quantiles = 1;
        let err = evaluate(&df, &panel(), &opts).unwrap_err();
        assert!(matches!(err, EvalError::InvalidQuantiles(1)));

        let mut opts = options(vec!["f1".to_string()]);
        opts.label_cols = Some(vec!["nope".to_string()]);
        let err = evaluate(&df, &panel(), &opts).unwrap_err();
        assert!(matches!(err, EvalError::BadLabelColumn(_)));

        let df_no_labels = df.drop("ret_1").unwrap().drop("ret_2").unwrap();
        let err = evaluate(&df_no_labels, &panel(), &options(vec!["f1".to_string()])).unwrap_err();
        assert!(matches!(err, EvalError::NoLabelColumns));
    }

    #[test]
    fn evaluate_dates_produce_monthly_table() -> Result<()> {
        let mut df = df!(
            "asset" => ["A", "B", "C", "A", "B", "C"],
            "time" => [20481i32, 20481, 20481, 20512, 20512, 20512],
            "f1" => [1.0, 2.0, 3.0, 3.0, 2.0, 1.0],
            "ret_1" => [0.01, 0.02, 0.03, 0.03, 0.02, 0.01],
        )?;
        let time = df.column("time")?.cast(&DataType::Date)?;
        df.with_column(time)?;

        let mut opts = options(vec!["f1".to_string()]);
        opts.min_cs_count = 3;
        opts.quantiles = 3;
        let out = evaluate(&df, &panel(), &opts)?;

        let monthly = out.ic_monthly.expect("date column yields monthly table");
        assert_eq!(monthly.height(), 2); // two distinct months
        // 20481 days = 2026-01-28; 20512 = 2026-02-28.
        let months: Vec<u32> = monthly
            .column("month")?
            .u32()?
            .into_no_null_iter()
            .collect();
        assert_eq!(months, [1, 2]);
        Ok(())
    }

    #[test]
    fn evaluate_packed_yyyymmdd_time_produces_monthly_table() -> Result<()> {
        // A-share style panels often carry the date as a packed YYYYMMDD int/string
        // rather than a Polars Date; the monthly heatmap must still populate.
        let df = df!(
            "asset" => ["A", "B", "C", "A", "B", "C"],
            "time" => [20240115i64, 20240115, 20240115, 20240220, 20240220, 20240220],
            "f1" => [1.0, 2.0, 3.0, 3.0, 2.0, 1.0],
            "ret_1" => [0.01, 0.02, 0.03, 0.03, 0.02, 0.01],
        )?;

        let mut opts = options(vec!["f1".to_string()]);
        opts.min_cs_count = 3;
        opts.quantiles = 3;
        let out = evaluate(&df, &panel(), &opts)?;

        let monthly = out
            .ic_monthly
            .expect("packed YYYYMMDD column yields monthly table");
        let months: Vec<u32> = monthly
            .column("month")?
            .u32()?
            .into_no_null_iter()
            .collect();
        assert_eq!(months, [1, 2]);
        let years: Vec<i32> = monthly.column("year")?.i32()?.into_no_null_iter().collect();
        assert_eq!(years, [2024, 2024]);
        Ok(())
    }

    #[test]
    fn month_keys_helpers_reject_non_dates() {
        assert_eq!(ymd_month_from_packed(20240402), Some((2024, 4)));
        assert_eq!(ymd_month_from_packed(1), None); // plain sequence index
        assert_eq!(ymd_month_from_packed(20241301), None); // month 13
        assert_eq!(ymd_month_from_str("2024-04-02"), Some((2024, 4)));
        assert_eq!(ymd_month_from_str("20240402"), Some((2024, 4)));
        assert_eq!(ymd_month_from_str("abc"), None);
    }
}

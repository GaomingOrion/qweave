use std::collections::BTreeMap;

use polars::prelude::*;

use crate::error::{EvalError, Result};
use crate::evaluate::{EvalOutput, TableData};

/// Write a self-contained HTML report: a sortable multi-factor summary table
/// and a per-factor drill-down (quantile returns, monthly IC) drawn with inline
/// SVG. No external assets, no server — it opens straight from disk.
///
/// Memory mode only; a streamed result already has its tables on disk. `detail`
/// is capped at `max_detail_factors` factors (by summary order) to bound file
/// size on large runs — the summary table always covers every factor.
pub fn to_html(output: &EvalOutput, path: &str, max_detail_factors: usize) -> Result<()> {
    let TableData::Memory(quantiles) = &output.quantile_returns else {
        return Err(EvalError::AlreadySaved(
            "to_html requires an in-memory result".to_string(),
        ));
    };

    let summary_json = rows_to_json(&output.summary)?;
    let detail_json = detail_json(
        &output.summary,
        quantiles,
        output.ic_monthly.as_ref(),
        max_detail_factors,
    )?;
    let html = HTML_TEMPLATE
        .replace("__SUMMARY__", &summary_json)
        .replace("__DETAIL__", &detail_json);
    std::fs::write(path, html)?;
    Ok(())
}

/// Per-factor detail: weighted-mean quantile returns by (bin, horizon) and the
/// monthly IC series when present.
fn detail_json(
    summary: &DataFrame,
    quantiles: &DataFrame,
    ic_monthly: Option<&DataFrame>,
    max_detail_factors: usize,
) -> Result<String> {
    let factor_order: Vec<String> = str_col(summary, "factor")?
        .into_iter()
        .take_while_distinct(max_detail_factors);

    // Aggregate quantile returns: weighted mean of each mean_ret_h by count,
    // per (factor, bin). Horizons are the mean_ret_* columns in schema order.
    let horizons: Vec<String> = quantiles
        .get_column_names()
        .iter()
        .filter(|name| name.starts_with("mean_ret_"))
        .map(|name| name.to_string())
        .collect();
    let factors = str_col(quantiles, "factor")?;
    let bins = u32_col(quantiles, "bin")?;
    let counts = u32_col(quantiles, "count")?;
    let horizon_values: Vec<Vec<f64>> = horizons
        .iter()
        .map(|name| f64_col(quantiles, name))
        .collect::<Result<_>>()?;

    // (factor, bin) -> per-horizon (weighted sum, weight).
    type Acc = BTreeMap<(String, u32), Vec<(f64, f64)>>;
    let mut acc: Acc = BTreeMap::new();
    for row in 0..factors.len() {
        let entry = acc
            .entry((factors[row].clone(), bins[row]))
            .or_insert_with(|| vec![(0.0, 0.0); horizons.len()]);
        let weight = counts[row] as f64;
        for (h, values) in horizon_values.iter().enumerate() {
            let value = values[row];
            if !value.is_nan() && weight > 0.0 {
                entry[h].0 += value * weight;
                entry[h].1 += weight;
            }
        }
    }

    // factor -> (bins, per-horizon per-bin mean returns).
    type FactorDetail = (Vec<u32>, Vec<Vec<f64>>);
    let mut per_factor: BTreeMap<String, FactorDetail> = BTreeMap::new();
    for ((factor, bin), horizon_acc) in &acc {
        let entry = per_factor
            .entry(factor.clone())
            .or_insert_with(|| (Vec::new(), vec![Vec::new(); horizons.len()]));
        entry.0.push(*bin);
        for (h, (sum, weight)) in horizon_acc.iter().enumerate() {
            entry.1[h].push(if *weight > 0.0 {
                sum / weight
            } else {
                f64::NAN
            });
        }
    }

    let monthly = ic_monthly.map(monthly_by_factor).transpose()?;

    let mut out = String::from("{");
    for (i, factor) in factor_order.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&json_string(factor));
        out.push_str(":{\"horizons\":[");
        out.push_str(
            &horizons
                .iter()
                .map(|name| json_string(name.trim_start_matches("mean_ret_")))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push_str("],\"bins\":");
        let (bins, q_ret) = per_factor
            .get(factor)
            .cloned()
            .unwrap_or_else(|| (Vec::new(), vec![Vec::new(); horizons.len()]));
        out.push_str(&json_u32_array(&bins));
        out.push_str(",\"q_ret\":[");
        for (h, series) in q_ret.iter().enumerate() {
            if h > 0 {
                out.push(',');
            }
            out.push_str(&json_f64_array(series));
        }
        out.push(']');
        if let Some(monthly) = &monthly {
            out.push_str(",\"monthly\":");
            out.push_str(monthly.get(factor).map(String::as_str).unwrap_or("null"));
        }
        out.push('}');
    }
    out.push('}');
    Ok(out)
}

/// factor -> JSON `{labels:[...], ic:[...], rank_ic:[...]}` for the monthly table.
fn monthly_by_factor(monthly: &DataFrame) -> Result<BTreeMap<String, String>> {
    let factors = str_col(monthly, "factor")?;
    let years = i32_col(monthly, "year")?;
    let months = u32_col(monthly, "month")?;
    let ic = f64_col(monthly, "ic_mean")?;
    let rank_ic = f64_col(monthly, "rank_ic_mean")?;

    type MonthlyAcc = (Vec<String>, Vec<f64>, Vec<f64>);
    let mut per: BTreeMap<String, MonthlyAcc> = BTreeMap::new();
    for row in 0..factors.len() {
        let entry = per
            .entry(factors[row].clone())
            .or_insert_with(|| (Vec::new(), Vec::new(), Vec::new()));
        entry.0.push(format!("{}-{:02}", years[row], months[row]));
        entry.1.push(ic[row]);
        entry.2.push(rank_ic[row]);
    }
    Ok(per
        .into_iter()
        .map(|(factor, (labels, ic, rank_ic))| {
            let labels_json = labels
                .iter()
                .map(|l| json_string(l))
                .collect::<Vec<_>>()
                .join(",");
            (
                factor,
                format!(
                    "{{\"labels\":[{}],\"ic\":{},\"rank_ic\":{}}}",
                    labels_json,
                    json_f64_array(&ic),
                    json_f64_array(&rank_ic)
                ),
            )
        })
        .collect())
}

// --- small JSON serializers (no serde dependency) ---------------------------

fn rows_to_json(df: &DataFrame) -> Result<String> {
    let names: Vec<String> = df
        .get_column_names()
        .iter()
        .map(|n| n.to_string())
        .collect();
    let mut out = String::from("[");
    for row in 0..df.height() {
        if row > 0 {
            out.push(',');
        }
        out.push('{');
        for (col, name) in names.iter().enumerate() {
            if col > 0 {
                out.push(',');
            }
            out.push_str(&json_string(name));
            out.push(':');
            out.push_str(&any_value_json(df.column(name)?.get(row)?));
        }
        out.push('}');
    }
    out.push(']');
    Ok(out)
}

fn any_value_json(value: AnyValue<'_>) -> String {
    match value {
        AnyValue::Null => "null".to_string(),
        AnyValue::Float64(v) => json_number(v),
        AnyValue::Float32(v) => json_number(v as f64),
        AnyValue::Int64(v) => v.to_string(),
        AnyValue::Int32(v) => v.to_string(),
        AnyValue::UInt32(v) => v.to_string(),
        AnyValue::UInt64(v) => v.to_string(),
        AnyValue::String(v) => json_string(v),
        AnyValue::StringOwned(v) => json_string(&v),
        other => json_string(&other.to_string()),
    }
}

fn json_number(v: f64) -> String {
    if v.is_finite() {
        format!("{v}")
    } else {
        "null".to_string()
    }
}

fn json_f64_array(values: &[f64]) -> String {
    let body = values
        .iter()
        .map(|v| json_number(*v))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
}

fn json_u32_array(values: &[u32]) -> String {
    let body = values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
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
            '<' => out.push_str("\\u003c"), // keep the JSON safe inside <script>
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// --- typed column extraction ------------------------------------------------

fn str_col(df: &DataFrame, name: &str) -> Result<Vec<String>> {
    Ok(df
        .column(name)?
        .str()?
        .iter()
        .map(|v| v.unwrap_or("").to_string())
        .collect())
}

fn f64_col(df: &DataFrame, name: &str) -> Result<Vec<f64>> {
    Ok(df
        .column(name)?
        .f64()?
        .iter()
        .map(|v| v.unwrap_or(f64::NAN))
        .collect())
}

fn u32_col(df: &DataFrame, name: &str) -> Result<Vec<u32>> {
    Ok(df
        .column(name)?
        .u32()?
        .iter()
        .map(|v| v.unwrap_or(0))
        .collect())
}

fn i32_col(df: &DataFrame, name: &str) -> Result<Vec<i32>> {
    Ok(df
        .column(name)?
        .i32()?
        .iter()
        .map(|v| v.unwrap_or(0))
        .collect())
}

/// Iterator adapter: the first `n` distinct values, preserving order.
trait TakeDistinct: Iterator<Item = String> + Sized {
    fn take_while_distinct(self, n: usize) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for value in self {
            if seen.insert(value.clone()) {
                out.push(value);
                if out.len() == n {
                    break;
                }
            }
        }
        out
    }
}
impl<I: Iterator<Item = String>> TakeDistinct for I {}

const HTML_TEMPLATE: &str = include_str!("report_template.html");

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use qfactors_core::PanelOptions;

    use super::*;
    use crate::evaluate::{EvaluateOptions, evaluate};
    use crate::{Binning, Demean, Weighting};

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

    #[test]
    fn to_html_writes_self_contained_file() -> Result<()> {
        let df = df!(
            "asset" => ["A", "B", "C", "D", "A", "B", "C", "D"],
            "time" => [1i64, 1, 1, 1, 2, 2, 2, 2],
            "f1" => [1.0, 2.0, 3.0, 4.0, 4.0, 3.0, 2.0, 1.0],
            "ret_1" => [0.01, 0.02, 0.03, 0.04, 0.04, 0.03, 0.02, 0.01],
        )?;
        let _ = BTreeSet::<String>::new();
        let panel = PanelOptions {
            symbol_col: "asset".to_string(),
            time_col: "time".to_string(),
        };
        let out = evaluate(&df, &panel, &options(vec!["f1".to_string()]))?;

        let path = std::env::temp_dir().join(format!("qf-report-{}.html", std::process::id()));
        let path_string = path.to_string_lossy().into_owned();
        to_html(&out, &path_string, 200)?;

        let html = std::fs::read_to_string(&path)?;
        assert!(html.contains("<!doctype html>") || html.contains("<!DOCTYPE html>"));
        assert!(html.contains("\"f1\""));
        // Data is inlined; no external resources are fetched (the only URLs are
        // the SVG/XML namespace URIs browsers never request).
        assert!(!html.contains("src=\"http"));
        assert!(!html.contains("<link"));
        assert!(!html.contains("cdn"));
        std::fs::remove_file(&path).ok();
        Ok(())
    }
}

//! In-memory report tables and their JSON shaping. The data is held as polars
//! DataFrames (passed in from an evaluation result, or read once from a saved
//! `output_dir`) and filtered per factor in memory — the interactive report
//! targets a shortlist, not thousand-factor runs.

use std::collections::HashSet;
use std::fs::File;
use std::path::Path;

use polars::prelude::*;
use serde_json::{Map, Value, json};

/// Everything the report API serves, already in memory.
pub struct ReportData {
    summary: DataFrame,
    ic: DataFrame,
    quantiles: DataFrame,
    portfolio: DataFrame,
    monthly: Option<DataFrame>,
    meta_json: String,
    factors: Vec<String>,
}

#[derive(Debug)]
pub enum DataError {
    Missing(String),
    Polars(PolarsError),
    Io(std::io::Error),
}

impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataError::Missing(what) => write!(f, "{what}"),
            DataError::Polars(err) => write!(f, "polars: {err}"),
            DataError::Io(err) => write!(f, "io: {err}"),
        }
    }
}

impl std::error::Error for DataError {}

impl From<PolarsError> for DataError {
    fn from(err: PolarsError) -> Self {
        DataError::Polars(err)
    }
}
impl From<std::io::Error> for DataError {
    fn from(err: std::io::Error) -> Self {
        DataError::Io(err)
    }
}

type Result<T> = std::result::Result<T, DataError>;

impl ReportData {
    /// Build from the five evaluation tables plus the `meta.json` snapshot. Date
    /// columns are cast to ISO strings once here so per-factor requests are cheap.
    pub fn new(
        summary: DataFrame,
        ic: DataFrame,
        quantiles: DataFrame,
        portfolio: DataFrame,
        monthly: Option<DataFrame>,
        meta_json: String,
    ) -> Result<Self> {
        let factors = distinct_factors(&summary)?;
        Ok(Self {
            summary,
            ic: stringify_dates(ic)?,
            quantiles: stringify_dates(quantiles)?,
            portfolio: stringify_dates(portfolio)?,
            monthly,
            meta_json,
            factors,
        })
    }

    /// Load the tables from a saved `output_dir` (one parquet per table plus
    /// `meta.json`).
    pub fn from_dir(dir: &Path) -> Result<Self> {
        let summary_path = dir.join("summary.parquet");
        if !summary_path.is_file() {
            return Err(DataError::Missing(format!(
                "{} is not an evaluation output dir (no summary.parquet)",
                dir.display()
            )));
        }
        let read = |name: &str| read_parquet(&dir.join(format!("{name}.parquet")));
        let monthly_path = dir.join("ic_monthly.parquet");
        let monthly = if monthly_path.is_file() {
            Some(read_parquet(&monthly_path)?)
        } else {
            None
        };
        let meta_path = dir.join("meta.json");
        let meta_json = if meta_path.is_file() {
            std::fs::read_to_string(meta_path)?
        } else {
            "{}".to_string()
        };
        Self::new(
            read("summary")?,
            read("ic")?,
            read("quantile_returns")?,
            read("portfolio")?,
            monthly,
            meta_json,
        )
    }

    /// The `meta.json` snapshot with the factor list added.
    pub fn meta_value(&self) -> Value {
        let mut value: Value = serde_json::from_str(&self.meta_json).unwrap_or_else(|_| json!({}));
        if let Value::Object(map) = &mut value {
            map.insert("factors".into(), json!(self.factors));
        }
        value
    }

    /// The full summary table as an array of row objects (drives the grid).
    pub fn summary_records(&self) -> Value {
        Value::Array(df_to_records(&self.summary))
    }

    /// Every per-factor series the tearsheet needs, in one payload. All horizons
    /// are included; the frontend filters client-side. `monthly` is null when
    /// there is no `ic_monthly` table.
    pub fn factor_bundle(&self, name: &str) -> Result<Value> {
        if !self.factors.iter().any(|f| f == name) {
            return Err(DataError::Missing(format!("unknown factor {name:?}")));
        }
        let mut obj = Map::new();
        obj.insert("factor".into(), Value::String(name.to_string()));
        obj.insert("ic".into(), df_columns(&filter_factor(&self.ic, name)?)?);
        obj.insert(
            "quantiles".into(),
            df_columns(&filter_factor(&self.quantiles, name)?)?,
        );
        obj.insert(
            "portfolio".into(),
            df_columns(&filter_factor(&self.portfolio, name)?)?,
        );
        obj.insert(
            "monthly".into(),
            match &self.monthly {
                Some(df) => df_columns(&filter_factor(df, name)?)?,
                None => Value::Null,
            },
        );
        Ok(Value::Object(obj))
    }
}

fn read_parquet(path: &Path) -> Result<DataFrame> {
    Ok(ParquetReader::new(File::open(path)?).finish()?)
}

fn filter_factor(df: &DataFrame, name: &str) -> Result<DataFrame> {
    let mask = df.column("factor")?.str()?.equal(name);
    Ok(df.filter(&mask)?)
}

fn distinct_factors(summary: &DataFrame) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for value in summary.column("factor")?.str()?.iter().flatten() {
        if seen.insert(value.to_string()) {
            out.push(value.to_string());
        }
    }
    Ok(out)
}

/// Cast a `date` column (Date/Datetime) to ISO strings so the JSON carries axis
/// labels directly. No-op when the column is absent or already textual.
fn stringify_dates(df: DataFrame) -> PolarsResult<DataFrame> {
    if !df.get_column_names().iter().any(|c| c.as_str() == "date") {
        return Ok(df);
    }
    match df.column("date")?.dtype() {
        DataType::Date | DataType::Datetime(_, _) => {
            let mut df = df;
            let as_str = df.column("date")?.cast(&DataType::String)?;
            df.with_column(as_str)?;
            Ok(df)
        }
        _ => Ok(df),
    }
}

/// `{ colName: [values...] }` — compact column-oriented JSON for time series.
fn df_columns(df: &DataFrame) -> Result<Value> {
    let names: Vec<String> = df
        .get_column_names()
        .iter()
        .map(|n| n.to_string())
        .collect();
    let mut obj = Map::new();
    for name in &names {
        let column = df.column(name)?;
        let mut values = Vec::with_capacity(df.height());
        for row in 0..df.height() {
            values.push(any_value_json(column.get(row)?));
        }
        obj.insert(name.clone(), Value::Array(values));
    }
    Ok(Value::Object(obj))
}

/// `[ { col: value, ... }, ... ]` — row-oriented JSON for the summary grid.
fn df_to_records(df: &DataFrame) -> Vec<Value> {
    let names: Vec<String> = df
        .get_column_names()
        .iter()
        .map(|n| n.to_string())
        .collect();
    let mut rows = Vec::with_capacity(df.height());
    for row in 0..df.height() {
        let mut obj = Map::new();
        for name in &names {
            let value = df
                .column(name)
                .and_then(|c| c.get(row))
                .map(any_value_json)
                .unwrap_or(Value::Null);
            obj.insert(name.clone(), value);
        }
        rows.push(Value::Object(obj));
    }
    rows
}

fn any_value_json(value: AnyValue<'_>) -> Value {
    match value {
        AnyValue::Null => Value::Null,
        AnyValue::Boolean(v) => Value::Bool(v),
        AnyValue::Float64(v) => finite_json(v),
        AnyValue::Float32(v) => finite_json(v as f64),
        AnyValue::Int64(v) => Value::from(v),
        AnyValue::Int32(v) => Value::from(v),
        AnyValue::UInt64(v) => Value::from(v),
        AnyValue::UInt32(v) => Value::from(v),
        AnyValue::String(v) => Value::String(v.to_string()),
        AnyValue::StringOwned(v) => Value::String(v.to_string()),
        other => Value::String(other.to_string()),
    }
}

/// JSON has no NaN/Inf; map them to null so the frontend renders gaps.
fn finite_json(v: f64) -> Value {
    if v.is_finite() {
        serde_json::Number::from_f64(v)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    }
}

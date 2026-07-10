use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};
use std::ops::Range;

use polars::prelude::*;
use qweave_core::PanelOptions;
use qweave_core::cellset::{CellSet, build_cellset};
use rayon::prelude::*;

use crate::error::{EvalError, Result};

pub const TRADABLE_ENTRY: &str = "tradable_entry";

#[derive(Debug, Clone)]
pub struct LabelOptions {
    pub horizons: Vec<usize>,
    pub entry_lag: usize,
    pub entry_col: String,
    pub exit_col: String,
    pub tradable_col: Option<String>,
}

#[derive(Debug)]
pub struct LabelsOutput {
    pub df: DataFrame,
    /// Calendar days inside the panel's time range with no panel row at all
    /// (only populated when an explicit calendar is provided).
    pub missing_days: Vec<String>,
}

/// Append forward-return label columns `ret_{h}` (and `tradable_entry` when a
/// tradable column is given) to `df`, preserving the original row order.
///
/// `ret_h(t) = exit(t + entry_lag + h) / entry(t + entry_lag) - 1`, where bar
/// offsets are taken on the panel-wide time grid (the union of all panel dates,
/// or the provided calendar restricted to the panel's range), so a symbol's
/// missing days yield NaN instead of silently compressing the holding period.
pub fn with_labels(
    df: DataFrame,
    panel: &PanelOptions,
    opts: &LabelOptions,
    calendar: Option<&Series>,
) -> Result<LabelsOutput> {
    validate_horizons(&opts.horizons)?;
    validate_output_columns(&df, opts)?;

    let fields = BTreeSet::from([opts.entry_col.clone(), opts.exit_col.clone()]);
    let cs = build_cellset(&df, panel, &fields)?;
    let (grid_len, grid_map, missing_days) = build_grid(&cs, calendar)?;

    // Per NT row: its slot on the time grid and its original row index.
    let n = cs.n_cells;
    let mut grid_pos_nt = vec![0usize; n];
    let mut orig_of_nt = vec![0usize; n];
    for (block, range) in cs.time_blocks.iter().enumerate() {
        for tn in range.clone() {
            let nt = cs.tn_order[tn];
            grid_pos_nt[nt] = grid_map[block];
            orig_of_nt[nt] = cs.orig_index_tn[tn];
        }
    }

    let tradable_nt = opts
        .tradable_col
        .as_deref()
        .map(|name| tradable_in_nt_order(&df, name, &orig_of_nt))
        .transpose()?;

    let entry_nt = cs.fields[&opts.entry_col].clone();
    let exit_nt = cs.fields[&opts.exit_col].clone();
    let per_symbol: Vec<SymbolLabels> = cs
        .sym_blocks
        .par_iter()
        .map(|range| {
            symbol_labels(
                range.clone(),
                &entry_nt,
                &exit_nt,
                tradable_nt.as_deref(),
                &grid_pos_nt,
                grid_len,
                opts,
            )
        })
        .collect();

    // Scatter the per-symbol (NT-contiguous) outputs back to original row order.
    let mut ret_cols: Vec<Vec<f64>> = opts.horizons.iter().map(|_| vec![f64::NAN; n]).collect();
    let mut tradable_out = tradable_nt.as_ref().map(|_| vec![false; n]);
    for (range, symbol) in cs.sym_blocks.iter().zip(per_symbol) {
        for (offset, nt) in range.clone().enumerate() {
            let orig = orig_of_nt[nt];
            for (values, rets) in ret_cols.iter_mut().zip(&symbol.rets) {
                values[orig] = rets[offset];
            }
            if let (Some(out), Some(tradable)) = (&mut tradable_out, &symbol.tradable_entry) {
                out[orig] = tradable[offset];
            }
        }
    }

    let mut columns = Vec::with_capacity(ret_cols.len() + 1);
    for (&h, values) in opts.horizons.iter().zip(ret_cols) {
        columns.push(Column::new(format!("ret_{h}").into(), values));
    }
    if let Some(values) = tradable_out {
        columns.push(Column::new(TRADABLE_ENTRY.into(), values));
    }

    let mut out = df;
    out.hstack_mut(&columns)?;
    Ok(LabelsOutput {
        df: out,
        missing_days,
    })
}

struct SymbolLabels {
    rets: Vec<Vec<f64>>,
    tradable_entry: Option<Vec<bool>>,
}

fn symbol_labels(
    range: Range<usize>,
    entry_nt: &[f64],
    exit_nt: &[f64],
    tradable_nt: Option<&[bool]>,
    grid_pos_nt: &[usize],
    grid_len: usize,
    opts: &LabelOptions,
) -> SymbolLabels {
    let mut dense_entry = vec![f64::NAN; grid_len];
    let mut dense_exit = vec![f64::NAN; grid_len];
    let mut dense_tradable = tradable_nt.map(|_| vec![false; grid_len]);
    for nt in range.clone() {
        let slot = grid_pos_nt[nt];
        dense_entry[slot] = entry_nt[nt];
        dense_exit[slot] = exit_nt[nt];
        if let (Some(dense), Some(tradable)) = (&mut dense_tradable, tradable_nt) {
            dense[slot] = tradable[nt];
        }
    }

    let rows = range.len();
    let mut rets: Vec<Vec<f64>> = opts
        .horizons
        .iter()
        .map(|_| Vec::with_capacity(rows))
        .collect();
    let mut tradable_entry = tradable_nt.map(|_| Vec::with_capacity(rows));
    for nt in range {
        let entry_slot = grid_pos_nt[nt] + opts.entry_lag;
        for (rets, &h) in rets.iter_mut().zip(&opts.horizons) {
            let exit_slot = entry_slot + h;
            let value = if exit_slot < grid_len {
                dense_exit[exit_slot] / dense_entry[entry_slot] - 1.0
            } else {
                f64::NAN
            };
            rets.push(value);
        }
        if let (Some(out), Some(dense)) = (&mut tradable_entry, &dense_tradable) {
            out.push(entry_slot < grid_len && dense[entry_slot]);
        }
    }
    SymbolLabels {
        rets,
        tradable_entry,
    }
}

fn validate_horizons(horizons: &[usize]) -> Result<()> {
    let unique: BTreeSet<usize> = horizons.iter().copied().collect();
    if horizons.is_empty() || unique.contains(&0) || unique.len() != horizons.len() {
        return Err(EvalError::InvalidHorizons(format!("{horizons:?}")));
    }
    Ok(())
}

fn validate_output_columns(df: &DataFrame, opts: &LabelOptions) -> Result<()> {
    let mut names: Vec<String> = opts.horizons.iter().map(|h| format!("ret_{h}")).collect();
    if opts.tradable_col.is_some() {
        names.push(TRADABLE_ENTRY.to_string());
    }
    for name in names {
        if df.column(&name).is_ok() {
            return Err(EvalError::OutputColumnConflict(name));
        }
    }
    Ok(())
}

fn tradable_in_nt_order(df: &DataFrame, name: &str, orig_of_nt: &[usize]) -> Result<Vec<bool>> {
    let column = df
        .column(name)
        .map_err(|_| EvalError::Core(qweave_core::QWeaveError::MissingColumn(name.into())))?;
    let values = column.try_bool().ok_or_else(|| EvalError::DTypeMismatch {
        column: name.to_string(),
        expected: "bool",
        actual: column.dtype().to_string(),
    })?;
    // Null tradability means "cannot trade": missing information must not let a
    // sample into the evaluated universe.
    Ok(orig_of_nt
        .iter()
        .map(|&orig| values.get(orig).unwrap_or(false))
        .collect())
}

type Grid = (usize, Vec<usize>, Vec<String>);

/// Return (grid length, per-time-block grid slot, missing calendar days).
///
/// Without a calendar the grid is the panel's own date union, so the slot map is
/// the identity. With a calendar, panel dates must be a subset of it; the grid is
/// the calendar restricted to the panel's [first, last] range and calendar days
/// with no panel rows are reported back for a caller-side warning.
#[allow(clippy::mutable_key_type)]
fn build_grid(cs: &CellSet, calendar: Option<&Series>) -> Result<Grid> {
    let n_blocks = cs.time_blocks.len();
    let Some(calendar) = calendar else {
        return Ok((n_blocks, (0..n_blocks).collect(), Vec::new()));
    };

    let times = cs.times_tn.as_materialized_series();
    if calendar.dtype() != times.dtype() {
        return Err(EvalError::CalendarDTypeMismatch {
            calendar: calendar.dtype().to_string(),
            time: times.dtype().to_string(),
        });
    }
    for i in 1..calendar.len() {
        let prev = calendar.get(i - 1)?;
        let current = calendar.get(i)?;
        if prev.partial_cmp(&current) != Some(Ordering::Less) {
            return Err(EvalError::CalendarNotSorted);
        }
    }
    if n_blocks == 0 {
        return Ok((0, Vec::new(), Vec::new()));
    }

    let mut pos_by_value: HashMap<AnyValue<'static>, usize> =
        HashMap::with_capacity(calendar.len());
    for i in 0..calendar.len() {
        pos_by_value.insert(calendar.get(i)?.into_static(), i);
    }
    let mut block_pos = Vec::with_capacity(n_blocks);
    for range in &cs.time_blocks {
        let value = times.get(range.start)?;
        let position = pos_by_value
            .get(&value.clone().into_static())
            .copied()
            .ok_or_else(|| EvalError::TimeNotInCalendar(value.to_string()))?;
        block_pos.push(position);
    }

    // Panel blocks ascend in time and the calendar is strictly increasing, so
    // the first/last block bound the grid.
    let first = block_pos[0];
    let last = *block_pos.last().expect("n_blocks > 0");
    let grid_len = last - first + 1;
    let grid_map: Vec<usize> = block_pos.iter().map(|position| position - first).collect();

    let mut covered = vec![false; grid_len];
    for &slot in &grid_map {
        covered[slot] = true;
    }
    let mut missing_days = Vec::new();
    for (slot, covered) in covered.into_iter().enumerate() {
        if !covered {
            missing_days.push(calendar.get(first + slot)?.to_string());
        }
    }

    Ok((grid_len, grid_map, missing_days))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> PanelOptions {
        PanelOptions {
            symbol_col: "asset".to_string(),
            time_col: "time".to_string(),
        }
    }

    fn label_options(horizons: Vec<usize>) -> LabelOptions {
        LabelOptions {
            horizons,
            entry_lag: 1,
            entry_col: "close".to_string(),
            exit_col: "close".to_string(),
            tradable_col: None,
        }
    }

    fn ret_column(df: &DataFrame, name: &str) -> Vec<f64> {
        df.column(name)
            .unwrap()
            .try_f64()
            .expect("ret column is f64")
            .into_no_null_iter()
            .collect()
    }

    fn assert_close(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (index, (a, e)) in actual.iter().zip(expected).enumerate() {
            if e.is_nan() {
                assert!(a.is_nan(), "row {index}: expected NaN, got {a}");
            } else {
                assert!((a - e).abs() < 1e-12, "row {index}: got {a}, expected {e}");
            }
        }
    }

    #[test]
    fn close_to_close_lag_one() -> Result<()> {
        let df = df!(
            "asset" => ["A"; 5],
            "time" => [1i64, 2, 3, 4, 5],
            "close" => [10.0, 11.0, 12.0, 13.0, 14.0],
        )?;

        let out = with_labels(df, &options(), &label_options(vec![1, 2]), None)?;

        assert!(out.missing_days.is_empty());
        assert_close(
            &ret_column(&out.df, "ret_1"),
            &[
                12.0 / 11.0 - 1.0,
                13.0 / 12.0 - 1.0,
                14.0 / 13.0 - 1.0,
                f64::NAN,
                f64::NAN,
            ],
        );
        assert_close(
            &ret_column(&out.df, "ret_2"),
            &[
                13.0 / 11.0 - 1.0,
                14.0 / 12.0 - 1.0,
                f64::NAN,
                f64::NAN,
                f64::NAN,
            ],
        );
        Ok(())
    }

    #[test]
    fn entry_exit_price_combinations() -> Result<()> {
        let df = df!(
            "asset" => ["A"; 4],
            "time" => [1i64, 2, 3, 4],
            "open" => [1.0, 2.0, 3.0, 4.0],
            "close" => [10.0, 20.0, 30.0, 40.0],
        )?;

        let combos = [
            ("close", "close", [30.0 / 20.0, 40.0 / 30.0]),
            ("open", "close", [30.0 / 2.0, 40.0 / 3.0]),
            ("close", "open", [3.0 / 20.0, 4.0 / 30.0]),
            ("open", "open", [3.0 / 2.0, 4.0 / 3.0]),
        ];
        for (entry_col, exit_col, [first, second]) in combos {
            let opts = LabelOptions {
                horizons: vec![1],
                entry_lag: 1,
                entry_col: entry_col.to_string(),
                exit_col: exit_col.to_string(),
                tradable_col: None,
            };
            let out = with_labels(df.clone(), &options(), &opts, None)?;
            assert_close(
                &ret_column(&out.df, "ret_1"),
                &[first - 1.0, second - 1.0, f64::NAN, f64::NAN],
            );
        }
        Ok(())
    }

    #[test]
    fn entry_lag_zero() -> Result<()> {
        let df = df!(
            "asset" => ["A"; 3],
            "time" => [1i64, 2, 3],
            "close" => [10.0, 11.0, 12.1],
        )?;
        let opts = LabelOptions {
            entry_lag: 0,
            ..label_options(vec![1])
        };

        let out = with_labels(df, &options(), &opts, None)?;

        assert_close(
            &ret_column(&out.df, "ret_1"),
            &[11.0 / 10.0 - 1.0, 12.1 / 11.0 - 1.0, f64::NAN],
        );
        Ok(())
    }

    #[test]
    fn missing_symbol_day_yields_nan_on_panel_grid() -> Result<()> {
        // Grid is the union {1,2,3,4}; B has no row on day 3.
        let df = df!(
            "asset" => ["A", "A", "A", "A", "B", "B", "B"],
            "time" => [1i64, 2, 3, 4, 1, 2, 4],
            "close" => [10.0, 11.0, 12.0, 13.0, 100.0, 110.0, 130.0],
        )?;

        let out = with_labels(df, &options(), &label_options(vec![1]), None)?;

        let ret = ret_column(&out.df, "ret_1");
        // A rows: normal chain.
        assert_close(
            &ret[..4],
            &[12.0 / 11.0 - 1.0, 13.0 / 12.0 - 1.0, f64::NAN, f64::NAN],
        );
        // B@1: entry day 2 exists, exit day 3 missing -> NaN.
        // B@2: entry day 3 missing -> NaN. B@4: beyond grid -> NaN.
        assert_close(&ret[4..], &[f64::NAN, f64::NAN, f64::NAN]);
        Ok(())
    }

    #[test]
    fn calendar_inserts_missing_day_into_grid() -> Result<()> {
        let df = df!(
            "asset" => ["A", "A", "A"],
            "time" => [1i64, 2, 4],
            "close" => [10.0, 11.0, 13.0],
        )?;

        // Without a calendar day 4 is the bar right after day 2.
        let out = with_labels(df.clone(), &options(), &label_options(vec![1]), None)?;
        assert_close(
            &ret_column(&out.df, "ret_1"),
            &[13.0 / 11.0 - 1.0, f64::NAN, f64::NAN],
        );

        // With calendar [1..=5], day 3 occupies a grid slot: A@1 exits on the
        // (missing) day 3 -> NaN, and the missing day is reported.
        let calendar = Series::new("calendar".into(), [1i64, 2, 3, 4, 5]);
        let out = with_labels(df, &options(), &label_options(vec![1]), Some(&calendar))?;
        assert_close(
            &ret_column(&out.df, "ret_1"),
            &[f64::NAN, f64::NAN, f64::NAN],
        );
        assert_eq!(out.missing_days, ["3"]);
        Ok(())
    }

    #[test]
    fn calendar_violations_are_rejected() -> Result<()> {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 6],
            "close" => [10.0, 11.0],
        )?;

        let short = Series::new("calendar".into(), [1i64, 2, 3]);
        let err = with_labels(
            df.clone(),
            &options(),
            &label_options(vec![1]),
            Some(&short),
        )
        .unwrap_err();
        assert!(matches!(err, EvalError::TimeNotInCalendar(_)));

        let unsorted = Series::new("calendar".into(), [1i64, 6, 2]);
        let err = with_labels(
            df.clone(),
            &options(),
            &label_options(vec![1]),
            Some(&unsorted),
        )
        .unwrap_err();
        assert!(matches!(err, EvalError::CalendarNotSorted));

        let wrong_dtype = Series::new("calendar".into(), [1.0f64, 2.0]);
        let err =
            with_labels(df, &options(), &label_options(vec![1]), Some(&wrong_dtype)).unwrap_err();
        assert!(matches!(err, EvalError::CalendarDTypeMismatch { .. }));
        Ok(())
    }

    #[test]
    fn tradable_entry_is_shifted_to_the_entry_day() -> Result<()> {
        // Day 2 is not tradable for A; B is missing its day-2 row entirely.
        let df = df!(
            "asset" => ["A", "A", "A", "B", "B"],
            "time" => [1i64, 2, 3, 1, 3],
            "close" => [10.0, 11.0, 12.0, 100.0, 120.0],
            "tradable" => [Some(true), Some(false), Some(true), Some(true), None],
        )?;
        let opts = LabelOptions {
            tradable_col: Some("tradable".to_string()),
            ..label_options(vec![1])
        };

        let out = with_labels(df, &options(), &opts, None)?;

        let tradable: Vec<bool> = out
            .df
            .column(TRADABLE_ENTRY)?
            .try_bool()
            .expect("tradable_entry is bool")
            .iter()
            .map(|value| value.expect("tradable_entry has no nulls"))
            .collect();
        // A@1 -> entry day 2 not tradable; A@2 -> entry day 3 tradable;
        // A@3 -> entry day off-grid; B@1 -> entry day 2 has no row;
        // B@3 -> entry day off-grid.
        assert_eq!(tradable, [false, true, false, false, false]);
        Ok(())
    }

    #[test]
    fn original_row_order_is_preserved() -> Result<()> {
        let df = df!(
            "asset" => ["B", "A", "B", "A"],
            "time" => [2i64, 1, 1, 2],
            "close" => [22.0, 10.0, 20.0, 11.0],
        )?;
        let opts = LabelOptions {
            entry_lag: 0,
            ..label_options(vec![1])
        };

        let out = with_labels(df, &options(), &opts, None)?;

        assert_eq!(
            out.df
                .column("time")?
                .try_i64()
                .expect("time is i64")
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [2, 1, 1, 2]
        );
        // Row order matches input: B@2, A@1, B@1, A@2.
        assert_close(
            &ret_column(&out.df, "ret_1"),
            &[f64::NAN, 11.0 / 10.0 - 1.0, 22.0 / 20.0 - 1.0, f64::NAN],
        );
        Ok(())
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        let df = df!(
            "asset" => ["A"],
            "time" => [1i64],
            "close" => [10.0],
            "ret_1" => [0.0],
        )
        .unwrap();

        let err = with_labels(df.clone(), &options(), &label_options(vec![1]), None).unwrap_err();
        assert!(matches!(err, EvalError::OutputColumnConflict(name) if name == "ret_1"));

        for horizons in [vec![], vec![0], vec![1, 1]] {
            let err =
                with_labels(df.clone(), &options(), &label_options(horizons), None).unwrap_err();
            assert!(matches!(err, EvalError::InvalidHorizons(_)));
        }

        let no_bool = with_labels(
            df.clone(),
            &options(),
            &LabelOptions {
                tradable_col: Some("close".to_string()),
                ..label_options(vec![2])
            },
            None,
        )
        .unwrap_err();
        assert!(matches!(no_bool, EvalError::DTypeMismatch { .. }));
    }
}

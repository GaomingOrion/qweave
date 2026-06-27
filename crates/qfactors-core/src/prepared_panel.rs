use std::collections::HashMap;

use polars::prelude::*;

use crate::error::{QFactorsError, Result};
use crate::group::GroupInfo;

pub const GROUP_ID_COL: &str = "__qfactors_group_id";
pub const TIME_ORD_COL: &str = "__qfactors_time_ord";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullPolicy {
    Error,
    FloatNullToNan,
}

impl NullPolicy {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "error" => Ok(Self::Error),
            "float_null_to_nan" => Ok(Self::FloatNullToNan),
            other => Err(QFactorsError::UnsupportedNullPolicy(other.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparePanelOptions {
    pub group_col: String,
    pub time_col: String,
    pub column_aliases: HashMap<String, String>,
    pub sort: bool,
    pub rechunk: bool,
    pub null_policy: NullPolicy,
    pub output_group_id: bool,
}

#[derive(Debug, Clone)]
pub struct PreparedPanel {
    df: DataFrame,
    group_col: String,
    time_col: String,
    column_aliases: HashMap<String, String>,
    groups: Vec<GroupInfo>,
    output_group_id: bool,
}

impl PreparedPanel {
    pub fn new(mut df: DataFrame, options: PreparePanelOptions) -> Result<Self> {
        ensure_no_internal_column_conflicts(&df)?;
        ensure_column_exists(&df, &options.group_col)?;
        ensure_column_exists(&df, &options.time_col)?;
        apply_null_policy(&mut df, &options)?;

        if options.sort {
            df = sort_panel(&df, &options.group_col, &options.time_col)?;
        } else {
            ensure_panel_sorted(&df, &options.group_col, &options.time_col)?;
        }

        if options.rechunk {
            df.rechunk_mut();
        }

        let group_ids = build_groups(&df, &options.group_col, &options.time_col)?;
        let time_ordinals = build_time_ordinals(&df, &options.time_col)?;
        let groups = group_ids.groups;

        df.with_column(Column::new(GROUP_ID_COL.into(), group_ids.values))?;
        df.with_column(Column::new(TIME_ORD_COL.into(), time_ordinals))?;

        Ok(Self {
            df,
            group_col: options.group_col,
            time_col: options.time_col,
            column_aliases: options.column_aliases,
            groups,
            output_group_id: options.output_group_id,
        })
    }

    pub fn dataframe(&self) -> &DataFrame {
        &self.df
    }

    pub fn group_col(&self) -> &str {
        &self.group_col
    }

    pub fn time_col(&self) -> &str {
        &self.time_col
    }

    pub fn column_aliases(&self) -> &HashMap<String, String> {
        &self.column_aliases
    }

    pub fn groups(&self) -> &[GroupInfo] {
        &self.groups
    }

    pub fn output_group_id(&self) -> bool {
        self.output_group_id
    }
}

#[derive(Debug)]
struct GroupBuildResult {
    values: Vec<u32>,
    groups: Vec<GroupInfo>,
}

fn ensure_no_internal_column_conflicts(df: &DataFrame) -> Result<()> {
    for name in [GROUP_ID_COL, TIME_ORD_COL] {
        if df.get_column_index(name).is_some() {
            return Err(QFactorsError::InternalColumnConflict(name));
        }
    }
    Ok(())
}

fn ensure_column_exists(df: &DataFrame, name: &str) -> Result<()> {
    df.column(name)
        .map(|_| ())
        .map_err(|_| QFactorsError::MissingColumn(name.to_string()))
}

fn apply_null_policy(df: &mut DataFrame, options: &PreparePanelOptions) -> Result<()> {
    reject_nulls_in_required_column(df, &options.group_col, true)?;
    reject_nulls_in_required_column(df, &options.time_col, false)?;

    match options.null_policy {
        NullPolicy::Error => {
            for column in df.columns() {
                if column.null_count() > 0 {
                    return Err(QFactorsError::NullNotAllowed {
                        column: column.name().to_string(),
                    });
                }
            }
        }
        NullPolicy::FloatNullToNan => {
            let names = df.get_column_names_owned();
            for name in names {
                let index = df
                    .get_column_index(name.as_str())
                    .expect("name came from this DataFrame");
                let column = df.column(name.as_str())?;
                if column.null_count() == 0 {
                    continue;
                }

                if column.dtype() != &DataType::Float64 {
                    return Err(QFactorsError::FloatNullToNanTypeMismatch {
                        column: name.to_string(),
                        dtype: format!("{:?}", column.dtype()),
                    });
                }

                let values: Vec<f64> = column
                    .try_f64()
                    .expect("dtype checked above")
                    .iter()
                    .map(|value| value.unwrap_or(f64::NAN))
                    .collect();
                df.replace_column(index, Column::new(name, values))?;
            }
        }
    }

    Ok(())
}

fn reject_nulls_in_required_column(df: &DataFrame, name: &str, is_group: bool) -> Result<()> {
    let column = df.column(name)?;
    if column.null_count() == 0 {
        return Ok(());
    }

    if is_group {
        Err(QFactorsError::GroupNull(name.to_string()))
    } else {
        Err(QFactorsError::TimeNull(name.to_string()))
    }
}

fn sort_panel(df: &DataFrame, group_col: &str, time_col: &str) -> Result<DataFrame> {
    Ok(df.sort([group_col, time_col], SortMultipleOptions::default())?)
}

fn ensure_panel_sorted(df: &DataFrame, group_col: &str, time_col: &str) -> Result<()> {
    let sorted = sort_panel(df, group_col, time_col)?;
    if same_panel_order(df, &sorted, group_col, time_col)? {
        Ok(())
    } else {
        Err(QFactorsError::SortOrder {
            group_col: group_col.to_string(),
            time_col: time_col.to_string(),
        })
    }
}

fn same_panel_order(
    left: &DataFrame,
    right: &DataFrame,
    group_col: &str,
    time_col: &str,
) -> Result<bool> {
    if left.height() != right.height() {
        return Ok(false);
    }

    let left_group = left.column(group_col)?;
    let left_time = left.column(time_col)?;
    let right_group = right.column(group_col)?;
    let right_time = right.column(time_col)?;

    for row in 0..left.height() {
        if value_key(left_group, row)? != value_key(right_group, row)? {
            return Ok(false);
        }
        if value_key(left_time, row)? != value_key(right_time, row)? {
            return Ok(false);
        }
    }

    Ok(true)
}

fn build_groups(df: &DataFrame, group_col: &str, time_col: &str) -> Result<GroupBuildResult> {
    let group = df.column(group_col)?;
    let time = df.column(time_col)?;
    let mut values = Vec::with_capacity(df.height());
    let mut groups = Vec::new();

    let mut current_group: Option<String> = None;
    let mut previous_time: Option<String> = None;
    let mut current_start = 0usize;
    let mut current_id = 0u32;

    for row in 0..df.height() {
        let group_key = value_key(group, row)?;
        let time_key = value_key(time, row)?;

        match &current_group {
            None => {
                current_group = Some(group_key.clone());
                current_start = row;
            }
            Some(existing) if existing != &group_key => {
                groups.push(GroupInfo {
                    id: current_id,
                    label_key: existing.clone(),
                    start: current_start,
                    end: row,
                });
                current_id += 1;
                current_group = Some(group_key.clone());
                current_start = row;
            }
            Some(_) => {
                if previous_time.as_ref() == Some(&time_key) {
                    return Err(QFactorsError::DuplicateGroupTime {
                        group_col: group_col.to_string(),
                        time_col: time_col.to_string(),
                    });
                }
            }
        }

        values.push(current_id);
        previous_time = Some(time_key);
    }

    if let Some(label_key) = current_group {
        groups.push(GroupInfo {
            id: current_id,
            label_key,
            start: current_start,
            end: df.height(),
        });
    }

    Ok(GroupBuildResult { values, groups })
}

fn build_time_ordinals(df: &DataFrame, time_col: &str) -> Result<Vec<u32>> {
    let time = df.column(time_col)?;
    let mut time_only = DataFrame::new_infer_height(vec![time.clone()])?;
    time_only = time_only.sort([time_col], SortMultipleOptions::default())?;

    let sorted_time = time_only.column(time_col)?;
    let mut next_ord = 0u32;
    let mut previous_key: Option<String> = None;
    let mut ord_by_key = HashMap::new();

    for row in 0..time_only.height() {
        let key = value_key(sorted_time, row)?;
        if previous_key.as_ref() != Some(&key) {
            ord_by_key.insert(key.clone(), next_ord);
            next_ord += 1;
            previous_key = Some(key);
        }
    }

    let mut ordinals = Vec::with_capacity(df.height());
    for row in 0..df.height() {
        let key = value_key(time, row)?;
        ordinals.push(
            *ord_by_key
                .get(&key)
                .expect("all row time values came from the unique time map"),
        );
    }

    Ok(ordinals)
}

fn value_key(column: &Column, row: usize) -> Result<String> {
    let value = column.get(row)?;
    Ok(format!("{:?}:{:?}", column.dtype(), value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_options() -> PreparePanelOptions {
        PreparePanelOptions {
            group_col: "asset".to_string(),
            time_col: "time".to_string(),
            column_aliases: HashMap::new(),
            sort: true,
            rechunk: true,
            null_policy: NullPolicy::Error,
            output_group_id: false,
        }
    }

    #[test]
    fn prepare_panel_sorts_and_encodes_groups() -> Result<()> {
        let df = df!(
            "asset" => ["B", "A", "A"],
            "time" => [1i64, 2, 1],
            "close" => [20.0, 11.0, 10.0],
        )?;

        let panel = PreparedPanel::new(df, default_options())?;

        assert_eq!(panel.groups().len(), 2);
        assert_eq!(panel.groups()[0].start, 0);
        assert_eq!(panel.groups()[0].end, 2);
        assert_eq!(
            panel
                .dataframe()
                .column("asset")?
                .try_str()
                .expect("asset is string")
                .iter()
                .map(|value| value.expect("asset has no nulls"))
                .collect::<Vec<_>>(),
            ["A", "A", "B"]
        );
        assert!(panel.dataframe().column(GROUP_ID_COL).is_ok());
        assert!(panel.dataframe().column(TIME_ORD_COL).is_ok());

        Ok(())
    }

    #[test]
    fn sort_false_rejects_unsorted_input() {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [2i64, 1],
            "close" => [11.0, 10.0],
        )
        .unwrap();
        let mut options = default_options();
        options.sort = false;

        let err = PreparedPanel::new(df, options).unwrap_err();
        assert!(matches!(err, QFactorsError::SortOrder { .. }));
    }

    #[test]
    fn duplicate_group_time_is_rejected() {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 1],
            "close" => [10.0, 11.0],
        )
        .unwrap();

        let err = PreparedPanel::new(df, default_options()).unwrap_err();
        assert!(matches!(err, QFactorsError::DuplicateGroupTime { .. }));
    }

    #[test]
    fn time_null_is_rejected() {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [Some(1i64), None],
            "close" => [10.0, 11.0],
        )
        .unwrap();

        let err = PreparedPanel::new(df, default_options()).unwrap_err();
        assert!(matches!(err, QFactorsError::TimeNull(_)));
    }

    #[test]
    fn float_null_to_nan_replaces_float_nulls() -> Result<()> {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 2],
            "close" => [Some(10.0), None],
        )?;
        let mut options = default_options();
        options.null_policy = NullPolicy::FloatNullToNan;

        let panel = PreparedPanel::new(df, options)?;
        let values = panel
            .dataframe()
            .column("close")?
            .try_f64()
            .expect("close is f64")
            .into_no_null_iter()
            .collect::<Vec<_>>();
        assert!(values[1].is_nan());

        Ok(())
    }
}

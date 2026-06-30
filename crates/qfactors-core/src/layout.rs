use rayon::prelude::*;

use crate::cellset::CellSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Nt,
    Tn,
}

pub fn nt_to_tn(values: &[f64], cs: &CellSet) -> Vec<f64> {
    debug_assert_eq!(values.len(), cs.n_cells);
    // Pure gather over a permutation: embarrassingly parallel. Nested inside the
    // DAG's per-level par_iter, rayon's work-stealing only spreads this across
    // cores when the enclosing level is too narrow to keep them busy.
    cs.tn_order.par_iter().map(|&nt_idx| values[nt_idx]).collect()
}

pub fn tn_to_nt(values: &[f64], cs: &CellSet) -> Vec<f64> {
    debug_assert_eq!(values.len(), cs.n_cells);
    let mut out = vec![f64::NAN; cs.n_cells];
    for (tn_idx, &nt_idx) in cs.tn_order.iter().enumerate() {
        out[nt_idx] = values[tn_idx];
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use polars::prelude::*;

    use super::*;

    fn cs() -> CellSet {
        CellSet {
            n_cells: 3,
            sym_blocks: vec![0..1, 1..3],
            time_blocks: vec![0..1, 1..3],
            tn_order: vec![1, 0, 2],
            fields: HashMap::new(),
            symbols_tn: Column::new("asset".into(), ["B", "A", "B"]),
            times_tn: Column::new("time".into(), [1i64, 2, 2]),
            time_block_by_value: HashMap::new(),
        }
    }

    #[test]
    fn converts_between_nt_and_tn() {
        let cs = cs();

        let tn = nt_to_tn(&[10.0, 20.0, 30.0], &cs);
        assert_eq!(tn, [20.0, 10.0, 30.0]);
        assert_eq!(tn_to_nt(&tn, &cs), [10.0, 20.0, 30.0]);
    }
}

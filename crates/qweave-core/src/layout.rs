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
    cs.tn_order
        .par_iter()
        .map(|&nt_idx| values[nt_idx])
        .collect()
}

pub fn tn_to_nt(values: &[f64], cs: &CellSet) -> Vec<f64> {
    debug_assert_eq!(values.len(), cs.n_cells);
    // Parallel scatter mirroring the `scatter_pairs` Nt branch. `tn_order` is a
    // permutation of `0..n_cells`, so every `nt_idx` is written exactly once and
    // the chunks touch disjoint cells. The base address is shared as a `usize`
    // because a raw pointer is not `Sync`; `out` is not resized while the
    // scatter runs, and `f64` is `Copy`, so there is nothing to drop.
    const CHUNK: usize = 8192;
    let mut out = vec![f64::NAN; cs.n_cells];
    let base = out.as_mut_ptr() as usize;
    cs.tn_order
        .par_chunks(CHUNK)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let start = chunk_idx * CHUNK;
            for (offset, &nt_idx) in chunk.iter().enumerate() {
                debug_assert!(nt_idx < cs.n_cells);
                // SAFETY: `nt_idx < n_cells` and, because `tn_order` is a
                // permutation, is the target of exactly one write across all
                // chunks, so this is a race-free store into the live `out`.
                unsafe { *(base as *mut f64).add(nt_idx) = values[start + offset] };
            }
        });
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
            orig_index_tn: vec![1, 0, 2],
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

    /// A CellSet carrying only the fields `tn_to_nt` reads (`n_cells`,
    /// `tn_order`); the rest are sized placeholders.
    #[allow(clippy::single_range_in_vec_init)]
    fn cs_with_order(order: Vec<usize>) -> CellSet {
        let n = order.len();
        CellSet {
            n_cells: n,
            sym_blocks: vec![0..n],
            time_blocks: vec![0..n],
            tn_order: order,
            orig_index_tn: (0..n).collect(),
            fields: HashMap::new(),
            symbols_tn: Column::new("asset".into(), vec!["A"; n]),
            times_tn: Column::new("time".into(), vec![0i64; n]),
            time_block_by_value: HashMap::new(),
        }
    }

    /// The parallel scatter must reproduce the serial gather bit-for-bit on
    /// arbitrary permutations, including sizes far larger than one chunk so the
    /// per-chunk `start` offset and cross-chunk disjointness are exercised.
    #[test]
    fn tn_to_nt_parallel_matches_serial_on_random_permutations() {
        let mut state = 0xD1B54A32D192ED03u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..50 {
            let n = 1 + (next() % 20_000) as usize;
            // Fisher-Yates shuffle of 0..n.
            let mut perm: Vec<usize> = (0..n).collect();
            for i in (1..n).rev() {
                let j = (next() % (i as u64 + 1)) as usize;
                perm.swap(i, j);
            }
            let values: Vec<f64> = (0..n).map(|i| i as f64).collect();
            let mut expected = vec![f64::NAN; n];
            for (tn_idx, &nt_idx) in perm.iter().enumerate() {
                expected[nt_idx] = values[tn_idx];
            }
            let cs = cs_with_order(perm);
            assert_eq!(tn_to_nt(&values, &cs), expected);
        }
    }
}

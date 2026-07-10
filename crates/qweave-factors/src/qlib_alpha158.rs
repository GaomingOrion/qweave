//! Qlib `Alpha158` feature set: 9 kbar + 4 price + 29 rolling groups × 5 windows
//! = 158 named expressions.
//!
//! Formulas follow Microsoft Qlib `Alpha158` (see `docs/qlib_alpha158.md`). All
//! factors are per-symbol time-series / element-wise, so they stay in the `Nt`
//! layout with no cross-section. Caliber notes that differ from qlib:
//! - Rolling ops require a full, NaN-free window (qlib uses `min_periods=1`), so
//!   qweave emits `NaN` over each symbol's first `N-1` warmup rows.
//! - `IMAX`/`IMIN` add `+1` because qweave `ts_argmax`/`ts_argmin` are 0-based
//!   while qlib `IdxMax`/`IdxMin` are 1-based; the offset cancels in `IMXD`.

use qweave_core::Expr;
use qweave_core::alpha::{
    abs, close, correlation, delay, gt, high, lit, log, low, lt, max, min, open, quantile, resi,
    rsquare, slope, ts_argmax, ts_argmin, ts_max, ts_mean, ts_min, ts_rank, ts_std, ts_sum, volume,
    vwap,
};

/// Zero-denominator guard used throughout Alpha158.
fn eps() -> Expr {
    lit(1e-12)
}

/// The five rolling windows Alpha158 expands every group over.
const WINDOWS: [usize; 5] = [5, 10, 20, 30, 60];

/// Builds one rolling group's expression for a given window length.
type RollingBuilder = fn(usize) -> Expr;

/// Build all 158 `(name, expr)` pairs in qlib feature order (kbar, price, then
/// each rolling group expanded over `WINDOWS`).
pub fn qlib_alpha158() -> Vec<(String, Expr)> {
    let mut out: Vec<(String, Expr)> = Vec::with_capacity(158);

    // Kbar (9): candlestick shape.
    out.push(("KMID".to_string(), (close() - open()) / open()));
    out.push(("KLEN".to_string(), (high() - low()) / open()));
    out.push((
        "KMID2".to_string(),
        (close() - open()) / (high() - low() + eps()),
    ));
    out.push(("KUP".to_string(), (high() - max(open(), close())) / open()));
    out.push((
        "KUP2".to_string(),
        (high() - max(open(), close())) / (high() - low() + eps()),
    ));
    out.push(("KLOW".to_string(), (min(open(), close()) - low()) / open()));
    out.push((
        "KLOW2".to_string(),
        (min(open(), close()) - low()) / (high() - low() + eps()),
    ));
    out.push((
        "KSFT".to_string(),
        (2.0 * close() - high() - low()) / open(),
    ));
    out.push((
        "KSFT2".to_string(),
        (2.0 * close() - high() - low()) / (high() - low() + eps()),
    ));

    // Price (4): current price normalized by close.
    out.push(("OPEN0".to_string(), open() / close()));
    out.push(("HIGH0".to_string(), high() / close()));
    out.push(("LOW0".to_string(), low() / close()));
    out.push(("VWAP0".to_string(), vwap() / close()));

    // Rolling (29 groups × 5 windows = 145).
    let groups: &[(&str, RollingBuilder)] = &[
        ("ROC", |d| delay(close(), d) / close()),
        ("MA", |d| ts_mean(close(), d) / close()),
        ("STD", |d| ts_std(close(), d) / close()),
        ("BETA", |d| slope(close(), d) / close()),
        ("RSQR", |d| rsquare(close(), d)),
        ("RESI", |d| resi(close(), d) / close()),
        ("MAX", |d| ts_max(high(), d) / close()),
        ("MIN", |d| ts_min(low(), d) / close()),
        ("QTLU", |d| quantile(close(), d, 0.8) / close()),
        ("QTLD", |d| quantile(close(), d, 0.2) / close()),
        ("RANK", |d| ts_rank(close(), d)),
        ("RSV", |d| {
            (close() - ts_min(low(), d)) / (ts_max(high(), d) - ts_min(low(), d) + eps())
        }),
        ("IMAX", |d| (ts_argmax(high(), d) + 1.0) / d as f64),
        ("IMIN", |d| (ts_argmin(low(), d) + 1.0) / d as f64),
        ("IMXD", |d| {
            (ts_argmax(high(), d) - ts_argmin(low(), d)) / d as f64
        }),
        ("CORR", |d| correlation(close(), log(volume() + 1.0), d)),
        ("CORD", |d| {
            correlation(
                close() / delay(close(), 1),
                log(volume() / delay(volume(), 1) + 1.0),
                d,
            )
        }),
        ("CNTP", |d| ts_mean(gt(close(), delay(close(), 1)), d)),
        ("CNTN", |d| ts_mean(lt(close(), delay(close(), 1)), d)),
        ("CNTD", |d| {
            ts_mean(gt(close(), delay(close(), 1)), d) - ts_mean(lt(close(), delay(close(), 1)), d)
        }),
        ("SUMP", |d| {
            ts_sum(max(close() - delay(close(), 1), lit(0.0)), d)
                / (ts_sum(abs(close() - delay(close(), 1)), d) + eps())
        }),
        ("SUMN", |d| {
            ts_sum(max(delay(close(), 1) - close(), lit(0.0)), d)
                / (ts_sum(abs(close() - delay(close(), 1)), d) + eps())
        }),
        ("SUMD", |d| {
            (ts_sum(max(close() - delay(close(), 1), lit(0.0)), d)
                - ts_sum(max(delay(close(), 1) - close(), lit(0.0)), d))
                / (ts_sum(abs(close() - delay(close(), 1)), d) + eps())
        }),
        ("VMA", |d| ts_mean(volume(), d) / (volume() + eps())),
        ("VSTD", |d| ts_std(volume(), d) / (volume() + eps())),
        ("WVMA", |d| {
            ts_std(abs(close() / delay(close(), 1) - 1.0) * volume(), d)
                / (ts_mean(abs(close() / delay(close(), 1) - 1.0) * volume(), d) + eps())
        }),
        ("VSUMP", |d| {
            ts_sum(max(volume() - delay(volume(), 1), lit(0.0)), d)
                / (ts_sum(abs(volume() - delay(volume(), 1)), d) + eps())
        }),
        ("VSUMN", |d| {
            ts_sum(max(delay(volume(), 1) - volume(), lit(0.0)), d)
                / (ts_sum(abs(volume() - delay(volume(), 1)), d) + eps())
        }),
        ("VSUMD", |d| {
            (ts_sum(max(volume() - delay(volume(), 1), lit(0.0)), d)
                - ts_sum(max(delay(volume(), 1) - volume(), lit(0.0)), d))
                / (ts_sum(abs(volume() - delay(volume(), 1)), d) + eps())
        }),
    ];

    for (prefix, build) in groups {
        for &d in &WINDOWS {
            out.push((format!("{prefix}{d}"), build(d)));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use qweave_core::expr::collect_fields;

    use super::*;

    #[test]
    fn builds_158_uniquely_named_factors() {
        let alphas = qlib_alpha158();
        assert_eq!(alphas.len(), 158);

        let names = alphas
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), 158, "factor names must be unique");

        // Spot-check one factor per structural tier.
        for expected in ["KMID", "VWAP0", "ROC5", "MA60", "IMXD20", "VSUMD60"] {
            assert!(names.contains(expected), "{expected} is present");
        }
    }

    #[test]
    fn factors_only_reference_ohlcv_and_vwap() {
        let allowed = ["open", "high", "low", "close", "volume", "vwap"]
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>();

        for (name, expr) in qlib_alpha158() {
            let mut fields = BTreeSet::new();
            collect_fields(&expr, &mut fields);
            assert!(
                fields.is_subset(&allowed),
                "{name} references unexpected fields: {fields:?}"
            );
        }
    }

    #[test]
    fn imax_encodes_one_based_offset() {
        let imax5 = qlib_alpha158()
            .into_iter()
            .find(|(name, _)| name == "IMAX5")
            .map(|(_, expr)| expr.to_string())
            .expect("IMAX5 is present");
        // (ts_arg_max(high, 5) + 1) / 5
        assert!(
            imax5.contains("ts_arg_max(col(high), 5)") && imax5.contains(", 1)"),
            "IMAX5 keeps the +1 offset: {imax5}"
        );
    }
}

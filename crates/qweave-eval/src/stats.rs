//! Small statistical primitives shared by the evaluation kernels.

/// Pearson correlation from accumulated raw sums; NaN when degenerate.
pub fn pearson_from_sums(n: usize, sx: f64, sy: f64, sxx: f64, syy: f64, sxy: f64) -> f64 {
    if n < 2 {
        return f64::NAN;
    }
    let n = n as f64;
    let cov = sxy - sx * sy / n;
    let var_x = sxx - sx * sx / n;
    let var_y = syy - sy * sy / n;
    if var_x <= 0.0 || var_y <= 0.0 {
        return f64::NAN;
    }
    cov / (var_x * var_y).sqrt()
}

/// (mean, sample std, count) over the non-NaN entries of `series`.
pub fn mean_std(series: &[f64]) -> (f64, f64, usize) {
    let mut n = 0usize;
    let mut sum = 0.0;
    for &value in series {
        if !value.is_nan() {
            n += 1;
            sum += value;
        }
    }
    if n == 0 {
        return (f64::NAN, f64::NAN, 0);
    }
    let mean = sum / n as f64;
    if n < 2 {
        return (mean, f64::NAN, n);
    }
    let mut ss = 0.0;
    for &value in series {
        if !value.is_nan() {
            let d = value - mean;
            ss += d * d;
        }
    }
    (mean, (ss / (n - 1) as f64).sqrt(), n)
}

/// Fraction of non-NaN entries that are strictly positive.
pub fn win_rate(series: &[f64]) -> f64 {
    let mut n = 0usize;
    let mut wins = 0usize;
    for &value in series {
        if !value.is_nan() {
            n += 1;
            if value > 0.0 {
                wins += 1;
            }
        }
    }
    if n == 0 {
        f64::NAN
    } else {
        wins as f64 / n as f64
    }
}

/// t-statistic of the mean with a Newey-West (Bartlett kernel) standard error.
///
/// NaN entries are dropped and the remainder treated as contiguous. `lag` is the
/// maximum autocovariance lag (0 recovers the plain t-statistic).
pub fn newey_west_t(series: &[f64], lag: usize) -> f64 {
    let clean: Vec<f64> = series.iter().copied().filter(|v| !v.is_nan()).collect();
    let n = clean.len();
    if n < 2 {
        return f64::NAN;
    }
    let n_f = n as f64;
    let mean = clean.iter().sum::<f64>() / n_f;
    let auto_cov = |l: usize| -> f64 {
        let mut sum = 0.0;
        for t in l..n {
            sum += (clean[t] - mean) * (clean[t - l] - mean);
        }
        sum / n_f
    };
    let mut variance = auto_cov(0);
    let max_lag = lag.min(n - 1);
    for l in 1..=max_lag {
        let weight = 1.0 - l as f64 / (max_lag as f64 + 1.0);
        variance += 2.0 * weight * auto_cov(l);
    }
    if variance <= 0.0 {
        return f64::NAN;
    }
    mean / (variance / n_f).sqrt()
}

/// Kendall tau-a between the index order of `values` and the values themselves,
/// skipping NaN entries; NaN when fewer than two valid entries.
pub fn kendall_tau_vs_index(values: &[f64]) -> f64 {
    let valid: Vec<f64> = values.iter().copied().filter(|v| !v.is_nan()).collect();
    let n = valid.len();
    if n < 2 {
        return f64::NAN;
    }
    let mut concordant = 0i64;
    let mut discordant = 0i64;
    for i in 0..n {
        for j in (i + 1)..n {
            if valid[j] > valid[i] {
                concordant += 1;
            } else if valid[j] < valid[i] {
                discordant += 1;
            }
        }
    }
    let pairs = (n * (n - 1) / 2) as f64;
    (concordant - discordant) as f64 / pairs
}

/// Type-7 (linear interpolation) quantile cut points splitting `sorted` into
/// `q` buckets; `sorted` must be ascending and non-empty. Returns q-1 cuts.
pub fn type7_cuts(sorted: &[f64], q: usize) -> Vec<f64> {
    let m = sorted.len();
    let mut cuts = Vec::with_capacity(q.saturating_sub(1));
    for k in 1..q {
        let pos = (k as f64 / q as f64) * (m as f64 - 1.0);
        let lo = pos.floor() as usize;
        let hi = pos.ceil() as usize;
        let value = if lo == hi {
            sorted[lo]
        } else {
            sorted[lo] + (pos - lo as f64) * (sorted[hi] - sorted[lo])
        };
        cuts.push(value);
    }
    cuts
}

/// Convert days since the Unix epoch to a (year, month) pair (proleptic
/// Gregorian civil calendar).
pub fn civil_year_month(days_since_epoch: i64) -> (i32, u32) {
    // Howard Hinnant's `civil_from_days` algorithm, month/year part only.
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    (year as i32, month as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pearson(xs: &[f64], ys: &[f64]) -> f64 {
        let n = xs.len();
        let sx = xs.iter().sum();
        let sy = ys.iter().sum();
        let sxx = xs.iter().map(|x| x * x).sum();
        let syy = ys.iter().map(|y| y * y).sum();
        let sxy = xs.iter().zip(ys).map(|(x, y)| x * y).sum();
        pearson_from_sums(n, sx, sy, sxx, syy, sxy)
    }

    #[test]
    fn pearson_matches_hand_computation() {
        let xs = [1.0, 2.0, 3.0, 4.0];
        let ys = [1.5, 2.5, 2.0, 4.5];
        // Hand-computed: cov = 4.25, var_x = 5, var_y = 32.75 - 27.5625 = 5.1875.
        let expected = 4.25 / (5.0f64 * 5.1875).sqrt();
        assert!((pearson(&xs, &ys) - expected).abs() < 1e-12);
        assert!((pearson(&xs, &xs) - 1.0).abs() < 1e-12);
        assert!(pearson(&[1.0, 1.0], &[2.0, 3.0]).is_nan());
    }

    #[test]
    fn mean_std_and_win_rate_skip_nan() {
        let series = [1.0, f64::NAN, 3.0, -2.0];
        let (mean, std, n) = mean_std(&series);
        assert_eq!(n, 3);
        assert!((mean - 2.0 / 3.0).abs() < 1e-12);
        // Sample variance of [1, 3, -2]: mean 2/3, ss = 1/9 + 49/9 + 64/9 = 114/9.
        assert!((std - (114.0 / 9.0 / 2.0f64).sqrt()).abs() < 1e-12);
        assert!((win_rate(&series) - 2.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn newey_west_lag_zero_is_plain_t() {
        let series = [0.5, 1.5, 0.0, 2.0, 1.0];
        let (mean, std, n) = mean_std(&series);
        // Plain t uses the sample std; NW with lag 0 uses the population std.
        let population = std * ((n as f64 - 1.0) / n as f64).sqrt();
        let expected = mean / (population / (n as f64).sqrt());
        assert!((newey_west_t(&series, 0) - expected).abs() < 1e-12);
    }

    #[test]
    fn newey_west_matches_hand_computation_lag_one() {
        let series = [1.0, 2.0, 3.0];
        // mean 2, deviations [-1, 0, 1]; gamma0 = 2/3, gamma1 = 0/3 = 0 ... but
        // gamma1 = ((0)*(-1) + (1)*(0)) / 3 = 0, weight = 1 - 1/2 = 0.5.
        let variance: f64 = 2.0 / 3.0;
        let expected = 2.0 / (variance / 3.0).sqrt();
        assert!((newey_west_t(&series, 1) - expected).abs() < 1e-12);
    }

    #[test]
    fn kendall_tau_vs_index_basics() {
        assert!((kendall_tau_vs_index(&[1.0, 2.0, 3.0]) - 1.0).abs() < 1e-12);
        assert!((kendall_tau_vs_index(&[3.0, 2.0, 1.0]) + 1.0).abs() < 1e-12);
        assert!((kendall_tau_vs_index(&[1.0, f64::NAN, 3.0, 2.0]) - (1.0 / 3.0)).abs() < 1e-12);
        assert!(kendall_tau_vs_index(&[1.0]).is_nan());
    }

    #[test]
    fn type7_cuts_match_numpy_quantile() {
        let sorted = [1.0, 2.0, 3.0, 4.0, 5.0];
        // numpy.quantile(x, [0.25, 0.5, 0.75]) with linear interpolation: 2, 3, 4.
        assert_eq!(type7_cuts(&sorted, 4), vec![2.0, 3.0, 4.0]);
        // Median of an even-length array interpolates.
        assert_eq!(type7_cuts(&[1.0, 2.0, 3.0, 4.0], 2), vec![2.5]);
    }

    #[test]
    fn civil_year_month_known_dates() {
        assert_eq!(civil_year_month(0), (1970, 1));
        assert_eq!(civil_year_month(31), (1970, 2));
        assert_eq!(civil_year_month(-1), (1969, 12));
        // 2026-07-04 is 20638 days after the epoch.
        assert_eq!(civil_year_month(20_638), (2026, 7));
        // Leap-year boundary: 2024-02-29 = 19782, 2024-03-01 = 19783.
        assert_eq!(civil_year_month(19_782), (2024, 2));
        assert_eq!(civil_year_month(19_783), (2024, 3));
    }
}

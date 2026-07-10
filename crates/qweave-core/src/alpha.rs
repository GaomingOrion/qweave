use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::expr::{CmpOp, Expr};

pub trait IntoExpr {
    fn into_expr(self) -> Expr;
}

impl IntoExpr for Expr {
    fn into_expr(self) -> Expr {
        self
    }
}

impl IntoExpr for f64 {
    fn into_expr(self) -> Expr {
        Expr::Const(self)
    }
}

pub fn lit(value: f64) -> Expr {
    Expr::Const(value)
}

pub fn col(name: &str) -> Expr {
    Expr::Field(name.to_string())
}

pub fn open() -> Expr {
    col("open")
}

pub fn close() -> Expr {
    col("close")
}

pub fn high() -> Expr {
    col("high")
}

pub fn low() -> Expr {
    col("low")
}

pub fn volume() -> Expr {
    col("volume")
}

pub fn vwap() -> Expr {
    col("vwap")
}

pub fn cap() -> Expr {
    col("cap")
}

pub fn industry() -> Expr {
    col("industry")
}

pub fn returns() -> Expr {
    close() / delay(close(), 1) - 1.0
}

pub fn rank(x: Expr) -> Expr {
    Expr::Rank(Box::new(x))
}

pub fn delay(x: Expr, d: usize) -> Expr {
    Expr::Delay(Box::new(x), d)
}

pub fn ts_sum(x: Expr, d: usize) -> Expr {
    Expr::TsSum(Box::new(x), d)
}

pub fn delta(x: Expr, d: usize) -> Expr {
    Expr::Delta(Box::new(x), d)
}

pub fn ts_mean(x: Expr, d: usize) -> Expr {
    Expr::TsMean(Box::new(x), d)
}

/// Average daily volume over the past `d` days.
///
/// Uses **share** volume (`ts_mean(volume, d)`) by design. Note that WorldQuant's
/// `adv{d}` is average daily **dollar** volume (volume * price); only when bit-exact
/// WQ reproduction is required, use `ts_mean(volume() * vwap(), d)` or supply an
/// `adv{d}` column instead.
pub fn adv(d: usize) -> Expr {
    ts_mean(volume(), d)
}

pub fn product(x: Expr, d: usize) -> Expr {
    Expr::Product(Box::new(x), d)
}

pub fn ts_min(x: Expr, d: usize) -> Expr {
    Expr::TsMin(Box::new(x), d)
}

pub fn ts_max(x: Expr, d: usize) -> Expr {
    Expr::TsMax(Box::new(x), d)
}

pub fn ts_argmin(x: Expr, d: usize) -> Expr {
    Expr::TsArgMin(Box::new(x), d)
}

pub fn ts_argmax(x: Expr, d: usize) -> Expr {
    Expr::TsArgMax(Box::new(x), d)
}

pub fn ts_rank(x: Expr, d: usize) -> Expr {
    Expr::TsRank(Box::new(x), d)
}

/// DolphinDB-compatible raw time-series rank (`mrank`): 0-based, minimum on
/// ties. The default [`ts_rank`] returns the percentile caliber instead.
pub fn ts_rank_raw(x: Expr, d: usize) -> Expr {
    Expr::TsRankRaw(Box::new(x), d)
}

pub fn ts_std(x: Expr, d: usize) -> Expr {
    Expr::TsStd(Box::new(x), d)
}

pub fn slope(x: Expr, d: usize) -> Expr {
    Expr::Slope(Box::new(x), d)
}

pub fn rsquare(x: Expr, d: usize) -> Expr {
    Expr::Rsquare(Box::new(x), d)
}

pub fn resi(x: Expr, d: usize) -> Expr {
    Expr::Resi(Box::new(x), d)
}

pub fn quantile(x: Expr, d: usize, q: f64) -> Expr {
    Expr::Quantile(Box::new(x), d, q)
}

pub fn decay_linear(x: Expr, d: usize) -> Expr {
    Expr::DecayLinear(Box::new(x), d)
}

pub fn correlation(x: Expr, y: Expr, d: usize) -> Expr {
    Expr::Correlation(Box::new(x), Box::new(y), d)
}

pub fn covariance(x: Expr, y: Expr, d: usize) -> Expr {
    Expr::Covariance(Box::new(x), Box::new(y), d)
}

pub fn scale(x: Expr, a: f64) -> Expr {
    Expr::Scale(Box::new(x), a)
}

pub fn group_rank(x: Expr, g: Expr) -> Expr {
    Expr::GroupRank(Box::new(x), Box::new(g))
}

pub fn group_neutralize(x: Expr, g: Expr) -> Expr {
    Expr::GroupNeutralize(Box::new(x), Box::new(g))
}

pub fn abs(x: Expr) -> Expr {
    Expr::Abs(Box::new(x))
}

pub fn log(x: Expr) -> Expr {
    Expr::Log(Box::new(x))
}

pub fn sign(x: Expr) -> Expr {
    Expr::Sign(Box::new(x))
}

pub fn signed_power(x: Expr, a: impl IntoExpr) -> Expr {
    Expr::SignedPower(Box::new(x), Box::new(a.into_expr()))
}

pub fn power(x: Expr, a: impl IntoExpr) -> Expr {
    Expr::Power(Box::new(x), Box::new(a.into_expr()))
}

pub fn min(x: Expr, y: Expr) -> Expr {
    Expr::Min(Box::new(x), Box::new(y))
}

pub fn max(x: Expr, y: Expr) -> Expr {
    Expr::Max(Box::new(x), Box::new(y))
}

pub fn lt(x: Expr, y: Expr) -> Expr {
    cmp(CmpOp::Lt, x, y)
}

pub fn gt(x: Expr, y: Expr) -> Expr {
    cmp(CmpOp::Gt, x, y)
}

pub fn le(x: Expr, y: Expr) -> Expr {
    cmp(CmpOp::Le, x, y)
}

pub fn ge(x: Expr, y: Expr) -> Expr {
    cmp(CmpOp::Ge, x, y)
}

pub fn eq(x: Expr, y: Expr) -> Expr {
    cmp(CmpOp::Eq, x, y)
}

pub fn where_(c: Expr, a: Expr, b: Expr) -> Expr {
    Expr::Where(Box::new(c), Box::new(a), Box::new(b))
}

fn cmp(op: CmpOp, x: Expr, y: Expr) -> Expr {
    Expr::Cmp(op, Box::new(x), Box::new(y))
}

impl Add for Expr {
    type Output = Expr;

    fn add(self, rhs: Self) -> Self::Output {
        Expr::Add(Box::new(self), Box::new(rhs))
    }
}

impl Sub for Expr {
    type Output = Expr;

    fn sub(self, rhs: Self) -> Self::Output {
        Expr::Sub(Box::new(self), Box::new(rhs))
    }
}

impl Mul for Expr {
    type Output = Expr;

    fn mul(self, rhs: Self) -> Self::Output {
        Expr::Mul(Box::new(self), Box::new(rhs))
    }
}

impl Div for Expr {
    type Output = Expr;

    fn div(self, rhs: Self) -> Self::Output {
        Expr::Div(Box::new(self), Box::new(rhs))
    }
}

impl Add<f64> for Expr {
    type Output = Expr;

    fn add(self, rhs: f64) -> Self::Output {
        Expr::Add(Box::new(self), Box::new(Expr::Const(rhs)))
    }
}

impl Sub<f64> for Expr {
    type Output = Expr;

    fn sub(self, rhs: f64) -> Self::Output {
        Expr::Sub(Box::new(self), Box::new(Expr::Const(rhs)))
    }
}

impl Mul<f64> for Expr {
    type Output = Expr;

    fn mul(self, rhs: f64) -> Self::Output {
        Expr::Mul(Box::new(self), Box::new(Expr::Const(rhs)))
    }
}

impl Div<f64> for Expr {
    type Output = Expr;

    fn div(self, rhs: f64) -> Self::Output {
        Expr::Div(Box::new(self), Box::new(Expr::Const(rhs)))
    }
}

impl Mul<Expr> for f64 {
    type Output = Expr;

    fn mul(self, rhs: Expr) -> Self::Output {
        Expr::Mul(Box::new(Expr::Const(self)), Box::new(rhs))
    }
}

impl Neg for Expr {
    type Output = Expr;

    fn neg(self) -> Self::Output {
        Expr::Neg(Box::new(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_expands_to_close_div_delay_minus_one() {
        assert_eq!(
            returns(),
            Expr::Sub(
                Box::new(Expr::Div(
                    Box::new(Expr::Field("close".to_string())),
                    Box::new(Expr::Delay(Box::new(Expr::Field("close".to_string())), 1)),
                )),
                Box::new(Expr::Const(1.0)),
            )
        );
    }

    #[test]
    fn dsl_sugar_expands_to_expected_expr_nodes() {
        assert_eq!(vwap(), Expr::Field("vwap".to_string()));
        assert_eq!(cap(), Expr::Field("cap".to_string()));
        assert_eq!(
            adv(20),
            Expr::TsMean(Box::new(Expr::Field("volume".to_string())), 20)
        );
        assert_eq!(
            group_neutralize(close(), industry()),
            Expr::GroupNeutralize(
                Box::new(Expr::Field("close".to_string())),
                Box::new(Expr::Field("industry".to_string())),
            )
        );
        assert_eq!(
            where_(gt(close(), open()), high(), low()),
            Expr::Where(
                Box::new(Expr::Cmp(
                    CmpOp::Gt,
                    Box::new(Expr::Field("close".to_string())),
                    Box::new(Expr::Field("open".to_string())),
                )),
                Box::new(Expr::Field("high".to_string())),
                Box::new(Expr::Field("low".to_string())),
            )
        );
        assert_eq!(lit(2.0), Expr::Const(2.0));
        assert_eq!(
            power(close(), 2.0),
            Expr::Power(
                Box::new(Expr::Field("close".to_string())),
                Box::new(Expr::Const(2.0)),
            )
        );
        assert_eq!(
            signed_power(close(), delta(volume(), 1)),
            Expr::SignedPower(
                Box::new(Expr::Field("close".to_string())),
                Box::new(Expr::Delta(Box::new(Expr::Field("volume".to_string())), 1)),
            )
        );
        assert_eq!(
            quantile(close(), 5, 0.8),
            Expr::Quantile(Box::new(Expr::Field("close".to_string())), 5, 0.8)
        );
    }

    #[test]
    fn alpha8_expression_can_be_built_with_clone() {
        let inner = ts_sum(open(), 5) * ts_sum(returns(), 5);
        let expr = -1.0 * rank(inner.clone() - delay(inner, 10));

        assert!(matches!(expr, Expr::Mul(_, _)));
    }
}

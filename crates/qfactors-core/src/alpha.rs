use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::expr::{CmpOp, Expr};

#[derive(Debug, Clone, PartialEq)]
pub struct A(Expr);

impl A {
    pub fn into_expr(self) -> Expr {
        self.0
    }
}

pub trait IntoAlphaExpr {
    fn into_alpha_expr(self) -> Expr;
}

impl IntoAlphaExpr for A {
    fn into_alpha_expr(self) -> Expr {
        self.into_expr()
    }
}

impl IntoAlphaExpr for f64 {
    fn into_alpha_expr(self) -> Expr {
        Expr::Const(self)
    }
}

pub fn constant(value: f64) -> A {
    A(Expr::Const(value))
}

pub fn field(name: &str) -> A {
    A(Expr::Field(name.to_string()))
}

pub fn open() -> A {
    field("open")
}

pub fn close() -> A {
    field("close")
}

pub fn high() -> A {
    field("high")
}

pub fn low() -> A {
    field("low")
}

pub fn volume() -> A {
    field("volume")
}

pub fn vwap() -> A {
    field("vwap")
}

pub fn cap() -> A {
    field("cap")
}

pub fn industry() -> A {
    field("industry")
}

pub fn returns() -> A {
    close() / delay(close(), 1) - 1.0
}

pub fn rank(x: A) -> A {
    A(Expr::Rank(Box::new(x.into_expr())))
}

pub fn delay(x: A, d: usize) -> A {
    A(Expr::Delay(Box::new(x.into_expr()), d))
}

pub fn sum(x: A, d: usize) -> A {
    A(Expr::TsSum(Box::new(x.into_expr()), d))
}

pub fn delta(x: A, d: usize) -> A {
    A(Expr::Delta(Box::new(x.into_expr()), d))
}

pub fn ts_mean(x: A, d: usize) -> A {
    A(Expr::TsMean(Box::new(x.into_expr()), d))
}

/// Average daily volume over the past `d` days.
///
/// Uses **share** volume (`ts_mean(volume, d)`) by design. Note that WorldQuant's
/// `adv{d}` is average daily **dollar** volume (volume * price); only when bit-exact
/// WQ reproduction is required, use `ts_mean(volume() * vwap(), d)` or supply an
/// `adv{d}` column instead.
pub fn adv(d: usize) -> A {
    ts_mean(volume(), d)
}

pub fn product(x: A, d: usize) -> A {
    A(Expr::Product(Box::new(x.into_expr()), d))
}

pub fn ts_min(x: A, d: usize) -> A {
    A(Expr::TsMin(Box::new(x.into_expr()), d))
}

pub fn ts_max(x: A, d: usize) -> A {
    A(Expr::TsMax(Box::new(x.into_expr()), d))
}

pub fn ts_argmin(x: A, d: usize) -> A {
    A(Expr::TsArgMin(Box::new(x.into_expr()), d))
}

pub fn ts_argmax(x: A, d: usize) -> A {
    A(Expr::TsArgMax(Box::new(x.into_expr()), d))
}

pub fn ts_rank(x: A, d: usize) -> A {
    A(Expr::TsRank(Box::new(x.into_expr()), d))
}

pub fn stddev(x: A, d: usize) -> A {
    A(Expr::TsStd(Box::new(x.into_expr()), d))
}

pub fn decay_linear(x: A, d: usize) -> A {
    A(Expr::DecayLinear(Box::new(x.into_expr()), d))
}

pub fn correlation(x: A, y: A, d: usize) -> A {
    A(Expr::Correlation(
        Box::new(x.into_expr()),
        Box::new(y.into_expr()),
        d,
    ))
}

pub fn covariance(x: A, y: A, d: usize) -> A {
    A(Expr::Covariance(
        Box::new(x.into_expr()),
        Box::new(y.into_expr()),
        d,
    ))
}

pub fn scale(x: A, a: f64) -> A {
    A(Expr::Scale(Box::new(x.into_expr()), a))
}

pub fn group_rank(x: A, g: A) -> A {
    A(Expr::GroupRank(
        Box::new(x.into_expr()),
        Box::new(g.into_expr()),
    ))
}

pub fn group_neutralize(x: A, g: A) -> A {
    A(Expr::GroupNeutralize(
        Box::new(x.into_expr()),
        Box::new(g.into_expr()),
    ))
}

pub fn indneutralize(x: A, g: A) -> A {
    group_neutralize(x, g)
}

pub fn abs(x: A) -> A {
    A(Expr::Abs(Box::new(x.into_expr())))
}

pub fn log(x: A) -> A {
    A(Expr::Log(Box::new(x.into_expr())))
}

pub fn sign(x: A) -> A {
    A(Expr::Sign(Box::new(x.into_expr())))
}

pub fn signedpower(x: A, a: impl IntoAlphaExpr) -> A {
    A(Expr::SignedPower(
        Box::new(x.into_expr()),
        Box::new(a.into_alpha_expr()),
    ))
}

pub fn power(x: A, a: impl IntoAlphaExpr) -> A {
    A(Expr::Power(
        Box::new(x.into_expr()),
        Box::new(a.into_alpha_expr()),
    ))
}

pub fn min(x: A, y: A) -> A {
    A(Expr::Min(Box::new(x.into_expr()), Box::new(y.into_expr())))
}

pub fn max(x: A, y: A) -> A {
    A(Expr::Max(Box::new(x.into_expr()), Box::new(y.into_expr())))
}

pub fn lt(x: A, y: A) -> A {
    cmp(CmpOp::Lt, x, y)
}

pub fn gt(x: A, y: A) -> A {
    cmp(CmpOp::Gt, x, y)
}

pub fn le(x: A, y: A) -> A {
    cmp(CmpOp::Le, x, y)
}

pub fn ge(x: A, y: A) -> A {
    cmp(CmpOp::Ge, x, y)
}

pub fn eq(x: A, y: A) -> A {
    cmp(CmpOp::Eq, x, y)
}

pub fn where_(c: A, a: A, b: A) -> A {
    A(Expr::Where(
        Box::new(c.into_expr()),
        Box::new(a.into_expr()),
        Box::new(b.into_expr()),
    ))
}

fn cmp(op: CmpOp, x: A, y: A) -> A {
    A(Expr::Cmp(
        op,
        Box::new(x.into_expr()),
        Box::new(y.into_expr()),
    ))
}

impl Add for A {
    type Output = A;

    fn add(self, rhs: Self) -> Self::Output {
        A(Expr::Add(
            Box::new(self.into_expr()),
            Box::new(rhs.into_expr()),
        ))
    }
}

impl Sub for A {
    type Output = A;

    fn sub(self, rhs: Self) -> Self::Output {
        A(Expr::Sub(
            Box::new(self.into_expr()),
            Box::new(rhs.into_expr()),
        ))
    }
}

impl Mul for A {
    type Output = A;

    fn mul(self, rhs: Self) -> Self::Output {
        A(Expr::Mul(
            Box::new(self.into_expr()),
            Box::new(rhs.into_expr()),
        ))
    }
}

impl Div for A {
    type Output = A;

    fn div(self, rhs: Self) -> Self::Output {
        A(Expr::Div(
            Box::new(self.into_expr()),
            Box::new(rhs.into_expr()),
        ))
    }
}

impl Add<f64> for A {
    type Output = A;

    fn add(self, rhs: f64) -> Self::Output {
        A(Expr::Add(
            Box::new(self.into_expr()),
            Box::new(Expr::Const(rhs)),
        ))
    }
}

impl Sub<f64> for A {
    type Output = A;

    fn sub(self, rhs: f64) -> Self::Output {
        A(Expr::Sub(
            Box::new(self.into_expr()),
            Box::new(Expr::Const(rhs)),
        ))
    }
}

impl Mul<f64> for A {
    type Output = A;

    fn mul(self, rhs: f64) -> Self::Output {
        A(Expr::Mul(
            Box::new(self.into_expr()),
            Box::new(Expr::Const(rhs)),
        ))
    }
}

impl Div<f64> for A {
    type Output = A;

    fn div(self, rhs: f64) -> Self::Output {
        A(Expr::Div(
            Box::new(self.into_expr()),
            Box::new(Expr::Const(rhs)),
        ))
    }
}

impl Mul<A> for f64 {
    type Output = A;

    fn mul(self, rhs: A) -> Self::Output {
        A(Expr::Mul(
            Box::new(Expr::Const(self)),
            Box::new(rhs.into_expr()),
        ))
    }
}

impl Neg for A {
    type Output = A;

    fn neg(self) -> Self::Output {
        A(Expr::Neg(Box::new(self.into_expr())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_expands_to_close_div_delay_minus_one() {
        assert_eq!(
            returns().into_expr(),
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
    fn phase_b_sugar_expands_to_expected_expr_nodes() {
        assert_eq!(vwap().into_expr(), Expr::Field("vwap".to_string()));
        assert_eq!(cap().into_expr(), Expr::Field("cap".to_string()));
        assert_eq!(
            adv(20).into_expr(),
            Expr::TsMean(Box::new(Expr::Field("volume".to_string())), 20)
        );
        assert_eq!(
            indneutralize(close(), industry()).into_expr(),
            Expr::GroupNeutralize(
                Box::new(Expr::Field("close".to_string())),
                Box::new(Expr::Field("industry".to_string())),
            )
        );
        assert_eq!(
            where_(gt(close(), open()), high(), low()).into_expr(),
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
        assert_eq!(constant(2.0).into_expr(), Expr::Const(2.0));
        assert_eq!(
            power(close(), 2.0).into_expr(),
            Expr::Power(
                Box::new(Expr::Field("close".to_string())),
                Box::new(Expr::Const(2.0)),
            )
        );
        assert_eq!(
            signedpower(close(), delta(volume(), 1)).into_expr(),
            Expr::SignedPower(
                Box::new(Expr::Field("close".to_string())),
                Box::new(Expr::Delta(Box::new(Expr::Field("volume".to_string())), 1)),
            )
        );
    }

    #[test]
    fn alpha8_expression_can_be_built_with_clone() {
        let inner = sum(open(), 5) * sum(returns(), 5);
        let expr = (-1.0 * rank(inner.clone() - delay(inner, 10))).into_expr();

        assert!(matches!(expr, Expr::Mul(_, _)));
    }
}

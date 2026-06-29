use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CmpOp {
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Field(String),
    Const(f64),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
    Delay(Box<Expr>, usize),
    Delta(Box<Expr>, usize),
    TsSum(Box<Expr>, usize),
    TsMean(Box<Expr>, usize),
    Product(Box<Expr>, usize),
    TsMin(Box<Expr>, usize),
    TsMax(Box<Expr>, usize),
    TsArgMin(Box<Expr>, usize),
    TsArgMax(Box<Expr>, usize),
    TsRank(Box<Expr>, usize),
    TsStd(Box<Expr>, usize),
    DecayLinear(Box<Expr>, usize),
    Correlation(Box<Expr>, Box<Expr>, usize),
    Covariance(Box<Expr>, Box<Expr>, usize),
    Rank(Box<Expr>),
    Scale(Box<Expr>, f64),
    GroupRank(Box<Expr>, Box<Expr>),
    GroupNeutralize(Box<Expr>, Box<Expr>),
    Abs(Box<Expr>),
    Log(Box<Expr>),
    Sign(Box<Expr>),
    SignedPower(Box<Expr>, Box<Expr>),
    Power(Box<Expr>, Box<Expr>),
    Min(Box<Expr>, Box<Expr>),
    Max(Box<Expr>, Box<Expr>),
    Cmp(CmpOp, Box<Expr>, Box<Expr>),
    Where(Box<Expr>, Box<Expr>, Box<Expr>),
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Field(name) => write!(f, "field({name})"),
            Expr::Const(value) => write!(f, "{value}"),
            Expr::Add(lhs, rhs) => write!(f, "add({lhs}, {rhs})"),
            Expr::Sub(lhs, rhs) => write!(f, "sub({lhs}, {rhs})"),
            Expr::Mul(lhs, rhs) => write!(f, "mul({lhs}, {rhs})"),
            Expr::Div(lhs, rhs) => write!(f, "div({lhs}, {rhs})"),
            Expr::Neg(inner) => write!(f, "neg({inner})"),
            Expr::Delay(inner, days) => write!(f, "delay({inner}, {days})"),
            Expr::Delta(inner, days) => write!(f, "delta({inner}, {days})"),
            Expr::TsSum(inner, days) => write!(f, "ts_sum({inner}, {days})"),
            Expr::TsMean(inner, days) => write!(f, "ts_mean({inner}, {days})"),
            Expr::Product(inner, days) => write!(f, "product({inner}, {days})"),
            Expr::TsMin(inner, days) => write!(f, "ts_min({inner}, {days})"),
            Expr::TsMax(inner, days) => write!(f, "ts_max({inner}, {days})"),
            Expr::TsArgMin(inner, days) => write!(f, "ts_arg_min({inner}, {days})"),
            Expr::TsArgMax(inner, days) => write!(f, "ts_arg_max({inner}, {days})"),
            Expr::TsRank(inner, days) => write!(f, "ts_rank({inner}, {days})"),
            Expr::TsStd(inner, days) => write!(f, "ts_std({inner}, {days})"),
            Expr::DecayLinear(inner, days) => write!(f, "decay_linear({inner}, {days})"),
            Expr::Correlation(lhs, rhs, days) => write!(f, "correlation({lhs}, {rhs}, {days})"),
            Expr::Covariance(lhs, rhs, days) => write!(f, "covariance({lhs}, {rhs}, {days})"),
            Expr::Rank(inner) => write!(f, "rank({inner})"),
            Expr::Scale(inner, scale_to) => write!(f, "scale({inner}, {scale_to})"),
            Expr::GroupRank(values, groups) => write!(f, "group_rank({values}, {groups})"),
            Expr::GroupNeutralize(values, groups) => {
                write!(f, "group_neutralize({values}, {groups})")
            }
            Expr::Abs(inner) => write!(f, "abs({inner})"),
            Expr::Log(inner) => write!(f, "log({inner})"),
            Expr::Sign(inner) => write!(f, "sign({inner})"),
            Expr::SignedPower(inner, exponent) => write!(f, "signed_power({inner}, {exponent})"),
            Expr::Power(inner, exponent) => write!(f, "power({inner}, {exponent})"),
            Expr::Min(lhs, rhs) => write!(f, "min({lhs}, {rhs})"),
            Expr::Max(lhs, rhs) => write!(f, "max({lhs}, {rhs})"),
            Expr::Cmp(op, lhs, rhs) => write!(f, "{}({lhs}, {rhs})", cmp_name(*op)),
            Expr::Where(cond, when_true, when_false) => {
                write!(f, "where({cond}, {when_true}, {when_false})")
            }
        }
    }
}

fn cmp_name(op: CmpOp) -> &'static str {
    match op {
        CmpOp::Lt => "lt",
        CmpOp::Gt => "gt",
        CmpOp::Le => "le",
        CmpOp::Ge => "ge",
        CmpOp::Eq => "eq",
    }
}

pub fn collect_fields(expr: &Expr, out: &mut BTreeSet<String>) {
    match expr {
        Expr::Field(name) => {
            out.insert(name.clone());
        }
        Expr::Const(_) => {}
        Expr::Add(lhs, rhs)
        | Expr::Sub(lhs, rhs)
        | Expr::Mul(lhs, rhs)
        | Expr::Div(lhs, rhs)
        | Expr::Min(lhs, rhs)
        | Expr::Max(lhs, rhs)
        | Expr::Cmp(_, lhs, rhs)
        | Expr::GroupRank(lhs, rhs)
        | Expr::GroupNeutralize(lhs, rhs)
        | Expr::Correlation(lhs, rhs, _)
        | Expr::Covariance(lhs, rhs, _)
        | Expr::SignedPower(lhs, rhs)
        | Expr::Power(lhs, rhs) => {
            collect_fields(lhs, out);
            collect_fields(rhs, out);
        }
        Expr::Where(cond, when_true, when_false) => {
            collect_fields(cond, out);
            collect_fields(when_true, out);
            collect_fields(when_false, out);
        }
        Expr::Neg(inner)
        | Expr::Delay(inner, _)
        | Expr::Delta(inner, _)
        | Expr::TsSum(inner, _)
        | Expr::TsMean(inner, _)
        | Expr::Product(inner, _)
        | Expr::TsMin(inner, _)
        | Expr::TsMax(inner, _)
        | Expr::TsArgMin(inner, _)
        | Expr::TsArgMax(inner, _)
        | Expr::TsRank(inner, _)
        | Expr::TsStd(inner, _)
        | Expr::DecayLinear(inner, _)
        | Expr::Rank(inner)
        | Expr::Scale(inner, _)
        | Expr::Abs(inner)
        | Expr::Log(inner)
        | Expr::Sign(inner) => {
            collect_fields(inner, out);
        }
    }
}

pub(crate) fn lookback_depth(expr: &Expr) -> usize {
    match expr {
        Expr::Field(_) | Expr::Const(_) => 0,
        Expr::Delay(inner, days) | Expr::Delta(inner, days) => lookback_depth(inner) + days,
        Expr::TsSum(inner, days)
        | Expr::TsMean(inner, days)
        | Expr::Product(inner, days)
        | Expr::TsMin(inner, days)
        | Expr::TsMax(inner, days)
        | Expr::TsArgMin(inner, days)
        | Expr::TsArgMax(inner, days)
        | Expr::TsRank(inner, days)
        | Expr::TsStd(inner, days)
        | Expr::DecayLinear(inner, days) => lookback_depth(inner) + days.saturating_sub(1),
        Expr::Correlation(lhs, rhs, days) | Expr::Covariance(lhs, rhs, days) => {
            lookback_depth(lhs).max(lookback_depth(rhs)) + days.saturating_sub(1)
        }
        Expr::Add(lhs, rhs)
        | Expr::Sub(lhs, rhs)
        | Expr::Mul(lhs, rhs)
        | Expr::Div(lhs, rhs)
        | Expr::Min(lhs, rhs)
        | Expr::Max(lhs, rhs)
        | Expr::Cmp(_, lhs, rhs)
        | Expr::GroupRank(lhs, rhs)
        | Expr::GroupNeutralize(lhs, rhs)
        | Expr::SignedPower(lhs, rhs)
        | Expr::Power(lhs, rhs) => lookback_depth(lhs).max(lookback_depth(rhs)),
        Expr::Where(cond, when_true, when_false) => lookback_depth(cond)
            .max(lookback_depth(when_true))
            .max(lookback_depth(when_false)),
        Expr::Neg(inner)
        | Expr::Rank(inner)
        | Expr::Scale(inner, _)
        | Expr::Abs(inner)
        | Expr::Log(inner)
        | Expr::Sign(inner) => lookback_depth(inner),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_fields_deduplicates_and_sorts() {
        let expr = Expr::Add(
            Box::new(Expr::Field("close".to_string())),
            Box::new(Expr::Delay(Box::new(Expr::Field("open".to_string())), 1)),
        );
        let mut fields = BTreeSet::new();

        collect_fields(&expr, &mut fields);

        assert_eq!(
            fields.into_iter().collect::<Vec<_>>(),
            ["close".to_string(), "open".to_string()]
        );
    }

    #[test]
    fn collect_fields_recurses_new_multi_input_variants() {
        let expr = Expr::Where(
            Box::new(Expr::Cmp(
                CmpOp::Gt,
                Box::new(Expr::Correlation(
                    Box::new(Expr::Field("close".to_string())),
                    Box::new(Expr::Field("volume".to_string())),
                    5,
                )),
                Box::new(Expr::Const(0.0)),
            )),
            Box::new(Expr::GroupNeutralize(
                Box::new(Expr::Field("open".to_string())),
                Box::new(Expr::Field("industry".to_string())),
            )),
            Box::new(Expr::Scale(Box::new(Expr::Field("low".to_string())), 1.0)),
        );
        let mut fields = BTreeSet::new();

        collect_fields(&expr, &mut fields);

        assert_eq!(
            fields.into_iter().collect::<Vec<_>>(),
            [
                "close".to_string(),
                "industry".to_string(),
                "low".to_string(),
                "open".to_string(),
                "volume".to_string()
            ]
        );
    }

    #[test]
    fn expr_display_uses_prefix_functions() {
        let expr = Expr::Mul(
            Box::new(Expr::Const(-1.0)),
            Box::new(Expr::Rank(Box::new(Expr::Sub(
                Box::new(Expr::Field("close".to_string())),
                Box::new(Expr::Delay(Box::new(Expr::Field("open".to_string())), 10)),
            )))),
        );

        assert_eq!(
            expr.to_string(),
            "mul(-1, rank(sub(field(close), delay(field(open), 10))))"
        );
    }

    #[test]
    fn lookback_depth_accounts_for_time_windows_only() {
        let expr = Expr::Rank(Box::new(Expr::Covariance(
            Box::new(Expr::Rank(Box::new(Expr::Field("close".to_string())))),
            Box::new(Expr::Delta(Box::new(Expr::Field("volume".to_string())), 2)),
            5,
        )));

        assert_eq!(lookback_depth(&expr), 6);
    }
}

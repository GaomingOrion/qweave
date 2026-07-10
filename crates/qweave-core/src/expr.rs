use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    TsRankRaw(Box<Expr>, usize),
    TsStd(Box<Expr>, usize),
    Slope(Box<Expr>, usize),
    Rsquare(Box<Expr>, usize),
    Resi(Box<Expr>, usize),
    Quantile(Box<Expr>, usize, f64),
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
            Expr::Field(name) => write!(f, "col({name})"),
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
            Expr::TsRankRaw(inner, days) => write!(f, "ts_rank_raw({inner}, {days})"),
            Expr::TsStd(inner, days) => write!(f, "ts_std({inner}, {days})"),
            Expr::Slope(inner, days) => write!(f, "slope({inner}, {days})"),
            Expr::Rsquare(inner, days) => write!(f, "rsquare({inner}, {days})"),
            Expr::Resi(inner, days) => write!(f, "resi({inner}, {days})"),
            Expr::Quantile(inner, days, q) => write!(f, "quantile({inner}, {days}, {q})"),
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
    visit_fields(expr, &mut |name| {
        out.insert(name.to_string());
    });
}

pub fn visit_fields(expr: &Expr, visit: &mut impl FnMut(&str)) {
    match expr {
        Expr::Field(name) => {
            visit(name);
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
            visit_fields(lhs, visit);
            visit_fields(rhs, visit);
        }
        Expr::Where(cond, when_true, when_false) => {
            visit_fields(cond, visit);
            visit_fields(when_true, visit);
            visit_fields(when_false, visit);
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
        | Expr::TsRankRaw(inner, _)
        | Expr::TsStd(inner, _)
        | Expr::Slope(inner, _)
        | Expr::Rsquare(inner, _)
        | Expr::Resi(inner, _)
        | Expr::Quantile(inner, _, _)
        | Expr::DecayLinear(inner, _)
        | Expr::Rank(inner)
        | Expr::Scale(inner, _)
        | Expr::Abs(inner)
        | Expr::Log(inner)
        | Expr::Sign(inner) => {
            visit_fields(inner, visit);
        }
    }
}

pub fn rename_fields(expr: &Expr, names: &BTreeMap<String, String>) -> Expr {
    match expr {
        Expr::Field(name) => Expr::Field(names.get(name).cloned().unwrap_or_else(|| name.clone())),
        Expr::Const(value) => Expr::Const(*value),
        Expr::Add(lhs, rhs) => binary(lhs, rhs, names, Expr::Add),
        Expr::Sub(lhs, rhs) => binary(lhs, rhs, names, Expr::Sub),
        Expr::Mul(lhs, rhs) => binary(lhs, rhs, names, Expr::Mul),
        Expr::Div(lhs, rhs) => binary(lhs, rhs, names, Expr::Div),
        Expr::Neg(inner) => unary(inner, names, Expr::Neg),
        Expr::Delay(inner, days) => unary_window(inner, *days, names, Expr::Delay),
        Expr::Delta(inner, days) => unary_window(inner, *days, names, Expr::Delta),
        Expr::TsSum(inner, days) => unary_window(inner, *days, names, Expr::TsSum),
        Expr::TsMean(inner, days) => unary_window(inner, *days, names, Expr::TsMean),
        Expr::Product(inner, days) => unary_window(inner, *days, names, Expr::Product),
        Expr::TsMin(inner, days) => unary_window(inner, *days, names, Expr::TsMin),
        Expr::TsMax(inner, days) => unary_window(inner, *days, names, Expr::TsMax),
        Expr::TsArgMin(inner, days) => unary_window(inner, *days, names, Expr::TsArgMin),
        Expr::TsArgMax(inner, days) => unary_window(inner, *days, names, Expr::TsArgMax),
        Expr::TsRank(inner, days) => unary_window(inner, *days, names, Expr::TsRank),
        Expr::TsRankRaw(inner, days) => unary_window(inner, *days, names, Expr::TsRankRaw),
        Expr::TsStd(inner, days) => unary_window(inner, *days, names, Expr::TsStd),
        Expr::Slope(inner, days) => unary_window(inner, *days, names, Expr::Slope),
        Expr::Rsquare(inner, days) => unary_window(inner, *days, names, Expr::Rsquare),
        Expr::Resi(inner, days) => unary_window(inner, *days, names, Expr::Resi),
        Expr::Quantile(inner, days, q) => {
            Expr::Quantile(Box::new(rename_fields(inner, names)), *days, *q)
        }
        Expr::DecayLinear(inner, days) => unary_window(inner, *days, names, Expr::DecayLinear),
        Expr::Correlation(lhs, rhs, days) => {
            binary_window(lhs, rhs, *days, names, Expr::Correlation)
        }
        Expr::Covariance(lhs, rhs, days) => binary_window(lhs, rhs, *days, names, Expr::Covariance),
        Expr::Rank(inner) => unary(inner, names, Expr::Rank),
        Expr::Scale(inner, scale_to) => {
            Expr::Scale(Box::new(rename_fields(inner, names)), *scale_to)
        }
        Expr::GroupRank(lhs, rhs) => binary(lhs, rhs, names, Expr::GroupRank),
        Expr::GroupNeutralize(lhs, rhs) => binary(lhs, rhs, names, Expr::GroupNeutralize),
        Expr::Abs(inner) => unary(inner, names, Expr::Abs),
        Expr::Log(inner) => unary(inner, names, Expr::Log),
        Expr::Sign(inner) => unary(inner, names, Expr::Sign),
        Expr::SignedPower(lhs, rhs) => binary(lhs, rhs, names, Expr::SignedPower),
        Expr::Power(lhs, rhs) => binary(lhs, rhs, names, Expr::Power),
        Expr::Min(lhs, rhs) => binary(lhs, rhs, names, Expr::Min),
        Expr::Max(lhs, rhs) => binary(lhs, rhs, names, Expr::Max),
        Expr::Cmp(op, lhs, rhs) => Expr::Cmp(
            *op,
            Box::new(rename_fields(lhs, names)),
            Box::new(rename_fields(rhs, names)),
        ),
        Expr::Where(cond, when_true, when_false) => Expr::Where(
            Box::new(rename_fields(cond, names)),
            Box::new(rename_fields(when_true, names)),
            Box::new(rename_fields(when_false, names)),
        ),
    }
}

fn unary(
    inner: &Expr,
    names: &BTreeMap<String, String>,
    build: impl FnOnce(Box<Expr>) -> Expr,
) -> Expr {
    build(Box::new(rename_fields(inner, names)))
}

fn unary_window(
    inner: &Expr,
    days: usize,
    names: &BTreeMap<String, String>,
    build: impl FnOnce(Box<Expr>, usize) -> Expr,
) -> Expr {
    build(Box::new(rename_fields(inner, names)), days)
}

fn binary(
    lhs: &Expr,
    rhs: &Expr,
    names: &BTreeMap<String, String>,
    build: impl FnOnce(Box<Expr>, Box<Expr>) -> Expr,
) -> Expr {
    build(
        Box::new(rename_fields(lhs, names)),
        Box::new(rename_fields(rhs, names)),
    )
}

fn binary_window(
    lhs: &Expr,
    rhs: &Expr,
    days: usize,
    names: &BTreeMap<String, String>,
    build: impl FnOnce(Box<Expr>, Box<Expr>, usize) -> Expr,
) -> Expr {
    build(
        Box::new(rename_fields(lhs, names)),
        Box::new(rename_fields(rhs, names)),
        days,
    )
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
    fn rename_fields_recurses_and_leaves_unmapped_names() {
        let expr = Expr::Where(
            Box::new(Expr::Cmp(
                CmpOp::Le,
                Box::new(Expr::Correlation(
                    Box::new(Expr::Field("close".to_string())),
                    Box::new(Expr::Field("volume".to_string())),
                    3,
                )),
                Box::new(Expr::Const(0.0)),
            )),
            Box::new(Expr::GroupRank(
                Box::new(Expr::Field("open".to_string())),
                Box::new(Expr::Field("industry".to_string())),
            )),
            Box::new(Expr::Scale(Box::new(Expr::Field("low".to_string())), 1.0)),
        );
        let renamed = rename_fields(
            &expr,
            &BTreeMap::from([
                ("close".to_string(), "adj_close".to_string()),
                ("open".to_string(), "adj_open".to_string()),
            ]),
        );

        let mut fields = BTreeSet::new();
        collect_fields(&renamed, &mut fields);

        assert_eq!(
            fields.into_iter().collect::<Vec<_>>(),
            [
                "adj_close".to_string(),
                "adj_open".to_string(),
                "industry".to_string(),
                "low".to_string(),
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
            "mul(-1, rank(sub(col(close), delay(col(open), 10))))"
        );
    }
}

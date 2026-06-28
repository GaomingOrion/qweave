use std::collections::BTreeSet;

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
    SignedPower(Box<Expr>, f64),
    Power(Box<Expr>, f64),
    Min(Box<Expr>, Box<Expr>),
    Max(Box<Expr>, Box<Expr>),
    Cmp(CmpOp, Box<Expr>, Box<Expr>),
    Where(Box<Expr>, Box<Expr>, Box<Expr>),
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
        | Expr::Covariance(lhs, rhs, _) => {
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
        | Expr::Sign(inner)
        | Expr::SignedPower(inner, _)
        | Expr::Power(inner, _) => {
            collect_fields(inner, out);
        }
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
}

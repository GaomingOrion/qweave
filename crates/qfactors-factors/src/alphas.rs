pub use qfactors_core::A;
pub use qfactors_core::alpha::{
    close, correlation, covariance, delay, delta, field, group_rank, high, indneutralize, industry,
    low, open, rank, returns, sign, sum, volume,
};
use qfactors_macros::alpha;

#[alpha]
pub fn alpha6() -> A {
    -1.0 * correlation(open(), volume(), 10)
}

#[alpha]
pub fn alpha8() -> A {
    let inner = sum(open(), 5) * sum(returns(), 5);
    -1.0 * rank(inner.clone() - delay(inner, 10))
}

#[alpha]
pub fn alpha12() -> A {
    sign(delta(volume(), 1)) * (-1.0 * delta(close(), 1))
}

#[alpha]
pub fn alpha13() -> A {
    -1.0 * rank(covariance(rank(close()), rank(volume()), 5))
}

#[alpha]
pub fn alpha101() -> A {
    (close() - open()) / (high() - low() + 0.001)
}

#[alpha(name = "group_returns_rank")]
pub fn group_returns_rank_alpha() -> A {
    group_rank(returns(), industry())
}

#[alpha(name = "industry_neutral_close")]
pub fn industry_neutral_close_alpha() -> A {
    indneutralize(close(), industry())
}

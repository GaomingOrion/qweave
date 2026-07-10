use qweave_core::Expr;
use qweave_core::alpha::{
    abs, adv, cap, close, col, correlation, covariance, decay_linear, delay, delta, ge,
    group_neutralize, high, industry, le, lit, log, low, lt, max, min, open, power, product, rank,
    returns, scale, sign, signed_power, ts_argmax, ts_argmin, ts_max, ts_min, ts_rank, ts_std,
    ts_sum, volume, vwap, where_,
};

fn c(value: f64) -> Expr {
    lit(value)
}

fn scale1(x: Expr) -> Expr {
    scale(x, 1.0)
}

fn sector() -> Expr {
    col("sector")
}

fn subindustry() -> Expr {
    col("subindustry")
}

fn mean(x: Expr, d: usize) -> Expr {
    ts_sum(x, d) / d as f64
}

fn blend(lhs: Expr, rhs: Expr, lhs_weight: f64) -> Expr {
    lhs * lhs_weight + rhs * (1.0 - lhs_weight)
}

fn trend_20_10_0() -> Expr {
    ((delay(close(), 20) - delay(close(), 10)) / 10.0) - ((delay(close(), 10) - close()) / 10.0)
}

pub fn alpha1() -> Expr {
    rank(ts_argmax(
        signed_power(
            where_(lt(returns(), c(0.0)), ts_std(returns(), 20), close()),
            2.0,
        ),
        5,
    )) - 0.5
}

pub fn alpha2() -> Expr {
    -1.0 * correlation(
        rank(delta(log(volume()), 2)),
        rank((close() - open()) / open()),
        6,
    )
}

pub fn alpha3() -> Expr {
    -1.0 * correlation(rank(open()), rank(volume()), 10)
}

pub fn alpha4() -> Expr {
    -1.0 * ts_rank(rank(low()), 9)
}

pub fn alpha5() -> Expr {
    rank(open() - mean(vwap(), 10)) * (-1.0 * abs(rank(close() - vwap())))
}

pub fn alpha6() -> Expr {
    -1.0 * correlation(open(), volume(), 10)
}

pub fn alpha7() -> Expr {
    where_(
        lt(adv(20), volume()),
        (-1.0 * ts_rank(abs(delta(close(), 7)), 60)) * sign(delta(close(), 7)),
        c(-1.0),
    )
}

pub fn alpha8() -> Expr {
    let inner = ts_sum(open(), 5) * ts_sum(returns(), 5);
    -1.0 * rank(inner.clone() - delay(inner, 10))
}

pub fn alpha9() -> Expr {
    where_(
        lt(c(0.0), ts_min(delta(close(), 1), 5)),
        delta(close(), 1),
        where_(
            lt(ts_max(delta(close(), 1), 5), c(0.0)),
            delta(close(), 1),
            -1.0 * delta(close(), 1),
        ),
    )
}

pub fn alpha10() -> Expr {
    rank(where_(
        lt(c(0.0), ts_min(delta(close(), 1), 4)),
        delta(close(), 1),
        where_(
            lt(ts_max(delta(close(), 1), 4), c(0.0)),
            delta(close(), 1),
            -1.0 * delta(close(), 1),
        ),
    ))
}

pub fn alpha11() -> Expr {
    (rank(ts_max(vwap() - close(), 3)) + rank(ts_min(vwap() - close(), 3)))
        * rank(delta(volume(), 3))
}

pub fn alpha12() -> Expr {
    sign(delta(volume(), 1)) * (-1.0 * delta(close(), 1))
}

pub fn alpha13() -> Expr {
    -1.0 * rank(covariance(rank(close()), rank(volume()), 5))
}

pub fn alpha14() -> Expr {
    (-1.0 * rank(delta(returns(), 3))) * correlation(open(), volume(), 10)
}

pub fn alpha15() -> Expr {
    -1.0 * ts_sum(rank(correlation(rank(high()), rank(volume()), 3)), 3)
}

pub fn alpha16() -> Expr {
    -1.0 * rank(covariance(rank(high()), rank(volume()), 5))
}

pub fn alpha17() -> Expr {
    ((-1.0 * rank(ts_rank(close(), 10))) * rank(delta(delta(close(), 1), 1)))
        * rank(ts_rank(volume() / adv(20), 5))
}

pub fn alpha18() -> Expr {
    -1.0 * rank(
        ts_std(abs(close() - open()), 5) + (close() - open()) + correlation(close(), open(), 10),
    )
}

pub fn alpha19() -> Expr {
    (-1.0 * sign((close() - delay(close(), 7)) + delta(close(), 7)))
        * (rank(ts_sum(returns(), 250) + 1.0) + 1.0)
}

pub fn alpha20() -> Expr {
    ((-1.0 * rank(open() - delay(high(), 1))) * rank(open() - delay(close(), 1)))
        * rank(open() - delay(low(), 1))
}

pub fn alpha21() -> Expr {
    let close_mean8 = mean(close(), 8);
    let close_mean2 = mean(close(), 2);
    let close_std8 = ts_std(close(), 8);
    where_(
        lt(
            close_mean8.clone() + close_std8.clone(),
            close_mean2.clone(),
        ),
        c(-1.0),
        where_(
            lt(close_mean2, close_mean8 - close_std8),
            c(1.0),
            where_(ge(volume() / adv(20), c(1.0)), c(1.0), c(-1.0)),
        ),
    )
}

pub fn alpha22() -> Expr {
    -1.0 * (delta(correlation(high(), volume(), 5), 5) * rank(ts_std(close(), 20)))
}

pub fn alpha23() -> Expr {
    where_(
        lt(mean(high(), 20), high()),
        -1.0 * delta(high(), 2),
        c(0.0),
    )
}

pub fn alpha24() -> Expr {
    let close_mean100 = mean(close(), 100);
    where_(
        le(delta(close_mean100, 100) / delay(close(), 100), c(0.05)),
        -1.0 * (close() - ts_min(close(), 100)),
        -1.0 * delta(close(), 3),
    )
}

pub fn alpha25() -> Expr {
    rank(((-1.0 * returns()) * adv(20)) * vwap() * (high() - close()))
}

pub fn alpha26() -> Expr {
    -1.0 * ts_max(correlation(ts_rank(volume(), 5), ts_rank(high(), 5), 5), 3)
}

pub fn alpha27() -> Expr {
    where_(
        lt(
            c(0.5),
            rank(mean(correlation(rank(volume()), rank(vwap()), 6), 2)),
        ),
        c(-1.0),
        c(1.0),
    )
}

pub fn alpha28() -> Expr {
    scale1(correlation(adv(20), low(), 5) + ((high() + low()) / 2.0) - close())
}

pub fn alpha29() -> Expr {
    let inner = rank(rank(-1.0 * rank(delta(close() - 1.0, 5))));
    ts_min(
        product(rank(rank(scale1(log(ts_sum(ts_min(inner, 2), 1))))), 1),
        5,
    ) + ts_rank(delay(-1.0 * returns(), 6), 5)
}

pub fn alpha30() -> Expr {
    let signs = sign(close() - delay(close(), 1))
        + sign(delay(close(), 1) - delay(close(), 2))
        + sign(delay(close(), 2) - delay(close(), 3));
    ((c(1.0) - rank(signs)) * ts_sum(volume(), 5)) / ts_sum(volume(), 20)
}

pub fn alpha31() -> Expr {
    rank(rank(rank(decay_linear(
        -1.0 * rank(rank(delta(close(), 10))),
        10,
    )))) + rank(-1.0 * delta(close(), 3))
        + sign(scale1(correlation(adv(20), low(), 12)))
}

pub fn alpha32() -> Expr {
    scale1(mean(close(), 7) - close()) + 20.0 * scale1(correlation(vwap(), delay(close(), 5), 230))
}

pub fn alpha33() -> Expr {
    rank(-1.0 * power(c(1.0) - (open() / close()), 1.0))
}

pub fn alpha34() -> Expr {
    rank(
        (c(1.0) - rank(ts_std(returns(), 2) / ts_std(returns(), 5)))
            + (c(1.0) - rank(delta(close(), 1))),
    )
}

pub fn alpha35() -> Expr {
    (ts_rank(volume(), 32) * (c(1.0) - ts_rank((close() + high()) - low(), 16)))
        * (c(1.0) - ts_rank(returns(), 32))
}

pub fn alpha36() -> Expr {
    2.21 * rank(correlation(close() - open(), delay(volume(), 1), 15))
        + 0.7 * rank(open() - close())
        + 0.73 * rank(ts_rank(delay(-1.0 * returns(), 6), 5))
        + rank(abs(correlation(vwap(), adv(20), 6)))
        + 0.6 * rank((mean(close(), 200) - open()) * (close() - open()))
}

pub fn alpha37() -> Expr {
    rank(correlation(delay(open() - close(), 1), close(), 200)) + rank(open() - close())
}

pub fn alpha38() -> Expr {
    (-1.0 * rank(ts_rank(close(), 10))) * rank(close() / open())
}

pub fn alpha39() -> Expr {
    (-1.0 * rank(delta(close(), 7) * (c(1.0) - rank(decay_linear(volume() / adv(20), 9)))))
        * (c(1.0) + rank(ts_sum(returns(), 250)))
}

pub fn alpha40() -> Expr {
    (-1.0 * rank(ts_std(high(), 10))) * correlation(high(), volume(), 10)
}

pub fn alpha41() -> Expr {
    power(high() * low(), 0.5) - vwap()
}

pub fn alpha42() -> Expr {
    rank(vwap() - close()) / rank(vwap() + close())
}

pub fn alpha43() -> Expr {
    ts_rank(volume() / adv(20), 20) * ts_rank(-1.0 * delta(close(), 7), 8)
}

pub fn alpha44() -> Expr {
    -1.0 * correlation(high(), rank(volume()), 5)
}

pub fn alpha45() -> Expr {
    -1.0 * ((rank(mean(delay(close(), 5), 20)) * correlation(close(), volume(), 2))
        * rank(correlation(ts_sum(close(), 5), ts_sum(close(), 20), 2)))
}

pub fn alpha46() -> Expr {
    where_(
        lt(c(0.25), trend_20_10_0()),
        c(-1.0),
        where_(
            lt(trend_20_10_0(), c(0.0)),
            c(1.0),
            -1.0 * (close() - delay(close(), 1)),
        ),
    )
}

pub fn alpha47() -> Expr {
    (((rank(c(1.0) / close()) * volume()) / adv(20))
        * ((high() * rank(high() - close())) / mean(high(), 5)))
        - rank(vwap() - delay(vwap(), 5))
}

pub fn alpha48() -> Expr {
    group_neutralize(
        (correlation(delta(close(), 1), delta(delay(close(), 1), 1), 250) * delta(close(), 1))
            / close(),
        subindustry(),
    ) / ts_sum(power(delta(close(), 1) / delay(close(), 1), 2.0), 250)
}

pub fn alpha49() -> Expr {
    where_(
        lt(trend_20_10_0(), c(-0.1)),
        c(1.0),
        -1.0 * (close() - delay(close(), 1)),
    )
}

pub fn alpha50() -> Expr {
    -1.0 * ts_max(rank(correlation(rank(volume()), rank(vwap()), 5)), 5)
}

pub fn alpha51() -> Expr {
    where_(
        lt(trend_20_10_0(), c(-0.05)),
        c(1.0),
        -1.0 * (close() - delay(close(), 1)),
    )
}

pub fn alpha52() -> Expr {
    ((-1.0 * ts_min(low(), 5) + delay(ts_min(low(), 5), 5))
        * rank((ts_sum(returns(), 240) - ts_sum(returns(), 20)) / 220.0))
        * ts_rank(volume(), 5)
}

pub fn alpha53() -> Expr {
    -1.0 * delta(
        ((close() - low()) - (high() - close())) / (close() - low()),
        9,
    )
}

pub fn alpha54() -> Expr {
    (-1.0 * ((low() - close()) * power(open(), 5.0))) / ((low() - high()) * power(close(), 5.0))
}

pub fn alpha55() -> Expr {
    -1.0 * correlation(
        rank((close() - ts_min(low(), 12)) / (ts_max(high(), 12) - ts_min(low(), 12))),
        rank(volume()),
        6,
    )
}

pub fn alpha56() -> Expr {
    -1.0 * (rank(ts_sum(returns(), 10) / ts_sum(ts_sum(returns(), 2), 3)) * rank(returns() * cap()))
}

pub fn alpha57() -> Expr {
    -1.0 * ((close() - vwap()) / decay_linear(rank(ts_argmax(close(), 30)), 2))
}

pub fn alpha58() -> Expr {
    -1.0 * ts_rank(
        decay_linear(
            correlation(group_neutralize(vwap(), sector()), volume(), 3),
            7,
        ),
        5,
    )
}

pub fn alpha59() -> Expr {
    -1.0 * ts_rank(
        decay_linear(
            correlation(group_neutralize(vwap(), industry()), volume(), 4),
            16,
        ),
        8,
    )
}

pub fn alpha60() -> Expr {
    -1.0 * (2.0
        * scale1(rank(
            (((close() - low()) - (high() - close())) / (high() - low())) * volume(),
        ))
        - scale1(rank(ts_argmax(close(), 10))))
}

pub fn alpha61() -> Expr {
    lt(
        rank(vwap() - ts_min(vwap(), 16)),
        rank(correlation(vwap(), adv(180), 17)),
    )
}

pub fn alpha62() -> Expr {
    -1.0 * lt(
        rank(correlation(vwap(), ts_sum(adv(20), 22), 9)),
        rank(lt(
            rank(open()) + rank(open()),
            rank((high() + low()) / 2.0) + rank(high()),
        )),
    )
}

pub fn alpha63() -> Expr {
    (rank(decay_linear(
        delta(group_neutralize(close(), industry()), 2),
        8,
    )) - rank(decay_linear(
        correlation(blend(vwap(), open(), 0.318108), ts_sum(adv(180), 37), 13),
        12,
    ))) * -1.0
}

pub fn alpha64() -> Expr {
    -1.0 * lt(
        rank(correlation(
            ts_sum(blend(open(), low(), 0.178404), 12),
            ts_sum(adv(120), 12),
            16,
        )),
        rank(delta(blend((high() + low()) / 2.0, vwap(), 0.178404), 3)),
    )
}

pub fn alpha65() -> Expr {
    -1.0 * lt(
        rank(correlation(
            blend(open(), vwap(), 0.00817205),
            ts_sum(adv(60), 8),
            6,
        )),
        rank(open() - ts_min(open(), 13)),
    )
}

pub fn alpha66() -> Expr {
    (rank(decay_linear(delta(vwap(), 3), 7))
        + ts_rank(
            decay_linear((low() - vwap()) / (open() - ((high() + low()) / 2.0)), 11),
            6,
        ))
        * -1.0
}

pub fn alpha67() -> Expr {
    power(
        rank(high() - ts_min(high(), 2)),
        rank(correlation(
            group_neutralize(vwap(), sector()),
            group_neutralize(adv(20), subindustry()),
            6,
        )),
    ) * -1.0
}

pub fn alpha68() -> Expr {
    -1.0 * lt(
        ts_rank(correlation(rank(high()), rank(adv(15)), 8), 13),
        rank(delta(blend(close(), low(), 0.518371), 1)),
    )
}

pub fn alpha69() -> Expr {
    power(
        rank(ts_max(delta(group_neutralize(vwap(), industry()), 2), 4)),
        ts_rank(correlation(blend(close(), vwap(), 0.490655), adv(20), 4), 9),
    ) * -1.0
}

pub fn alpha70() -> Expr {
    power(
        rank(delta(vwap(), 1)),
        ts_rank(
            correlation(group_neutralize(close(), industry()), adv(50), 17),
            17,
        ),
    ) * -1.0
}

pub fn alpha71() -> Expr {
    max(
        ts_rank(
            decay_linear(
                correlation(ts_rank(close(), 3), ts_rank(adv(180), 12), 18),
                4,
            ),
            15,
        ),
        ts_rank(
            decay_linear(power(rank((low() + open()) - (vwap() + vwap())), 2.0), 16),
            4,
        ),
    )
}

pub fn alpha72() -> Expr {
    rank(decay_linear(
        correlation((high() + low()) / 2.0, adv(40), 8),
        10,
    )) / rank(decay_linear(
        correlation(ts_rank(vwap(), 3), ts_rank(volume(), 18), 6),
        2,
    ))
}

pub fn alpha73() -> Expr {
    max(
        rank(decay_linear(delta(vwap(), 4), 2)),
        ts_rank(
            decay_linear(
                (delta(blend(open(), low(), 0.147155), 2) / blend(open(), low(), 0.147155)) * -1.0,
                3,
            ),
            16,
        ),
    ) * -1.0
}

pub fn alpha74() -> Expr {
    -1.0 * lt(
        rank(correlation(close(), ts_sum(adv(30), 37), 15)),
        rank(correlation(
            rank(blend(high(), vwap(), 0.0261661)),
            rank(volume()),
            11,
        )),
    )
}

pub fn alpha75() -> Expr {
    lt(
        rank(correlation(vwap(), volume(), 4)),
        rank(correlation(rank(low()), rank(adv(50)), 12)),
    )
}

pub fn alpha76() -> Expr {
    max(
        rank(decay_linear(delta(vwap(), 1), 11)),
        ts_rank(
            decay_linear(
                ts_rank(
                    correlation(group_neutralize(low(), sector()), adv(81), 8),
                    19,
                ),
                17,
            ),
            19,
        ),
    ) * -1.0
}

pub fn alpha77() -> Expr {
    min(
        rank(decay_linear(
            ((high() + low()) / 2.0 + high()) - (vwap() + high()),
            20,
        )),
        rank(decay_linear(
            correlation((high() + low()) / 2.0, adv(40), 3),
            5,
        )),
    )
}

pub fn alpha78() -> Expr {
    power(
        rank(correlation(
            ts_sum(blend(low(), vwap(), 0.352233), 19),
            ts_sum(adv(40), 19),
            6,
        )),
        rank(correlation(rank(vwap()), rank(volume()), 5)),
    )
}

pub fn alpha79() -> Expr {
    lt(
        rank(delta(
            group_neutralize(blend(close(), open(), 0.60733), sector()),
            1,
        )),
        rank(correlation(ts_rank(vwap(), 3), ts_rank(adv(150), 9), 14)),
    )
}

pub fn alpha80() -> Expr {
    power(
        rank(sign(delta(
            group_neutralize(blend(open(), high(), 0.868128), industry()),
            4,
        ))),
        ts_rank(correlation(high(), adv(10), 5), 5),
    ) * -1.0
}

pub fn alpha81() -> Expr {
    -1.0 * lt(
        rank(log(product(
            rank(power(
                rank(correlation(vwap(), ts_sum(adv(10), 49), 8)),
                4.0,
            )),
            14,
        ))),
        rank(correlation(rank(vwap()), rank(volume()), 5)),
    )
}

pub fn alpha82() -> Expr {
    min(
        rank(decay_linear(delta(open(), 1), 14)),
        ts_rank(
            decay_linear(
                correlation(group_neutralize(volume(), sector()), open(), 17),
                6,
            ),
            13,
        ),
    ) * -1.0
}

pub fn alpha83() -> Expr {
    (rank(delay((high() - low()) / mean(close(), 5), 2)) * rank(rank(volume())))
        / (((high() - low()) / mean(close(), 5)) / (vwap() - close()))
}

pub fn alpha84() -> Expr {
    signed_power(ts_rank(vwap() - ts_max(vwap(), 15), 20), delta(close(), 4))
}

pub fn alpha85() -> Expr {
    power(
        rank(correlation(blend(high(), close(), 0.876703), adv(30), 9)),
        rank(correlation(
            ts_rank((high() + low()) / 2.0, 3),
            ts_rank(volume(), 10),
            7,
        )),
    )
}

pub fn alpha86() -> Expr {
    -1.0 * lt(
        ts_rank(correlation(close(), ts_sum(adv(20), 14), 6), 20),
        rank((open() + close()) - (vwap() + open())),
    )
}

pub fn alpha87() -> Expr {
    max(
        rank(decay_linear(delta(blend(close(), vwap(), 0.369701), 1), 2)),
        ts_rank(
            decay_linear(
                abs(correlation(
                    group_neutralize(adv(81), industry()),
                    close(),
                    13,
                )),
                4,
            ),
            14,
        ),
    ) * -1.0
}

pub fn alpha88() -> Expr {
    min(
        rank(decay_linear(
            (rank(open()) + rank(low())) - (rank(high()) + rank(close())),
            8,
        )),
        ts_rank(
            decay_linear(correlation(ts_rank(close(), 8), ts_rank(adv(60), 20), 8), 6),
            2,
        ),
    )
}

pub fn alpha89() -> Expr {
    ts_rank(decay_linear(correlation(low(), adv(10), 6), 5), 3)
        - ts_rank(
            decay_linear(delta(group_neutralize(vwap(), industry()), 3), 10),
            15,
        )
}

pub fn alpha90() -> Expr {
    power(
        rank(close() - ts_max(close(), 4)),
        ts_rank(
            correlation(group_neutralize(adv(40), subindustry()), low(), 5),
            3,
        ),
    ) * -1.0
}

pub fn alpha91() -> Expr {
    (ts_rank(
        decay_linear(
            decay_linear(
                correlation(group_neutralize(close(), industry()), volume(), 9),
                16,
            ),
            3,
        ),
        4,
    ) - rank(decay_linear(correlation(vwap(), adv(30), 4), 2)))
        * -1.0
}

pub fn alpha92() -> Expr {
    min(
        ts_rank(
            decay_linear(lt((high() + low()) / 2.0 + close(), low() + open()), 14),
            18,
        ),
        ts_rank(
            decay_linear(correlation(rank(low()), rank(adv(30)), 7), 6),
            6,
        ),
    )
}

pub fn alpha93() -> Expr {
    ts_rank(
        decay_linear(
            correlation(group_neutralize(vwap(), industry()), adv(81), 17),
            19,
        ),
        7,
    ) / rank(decay_linear(delta(blend(close(), vwap(), 0.524434), 2), 16))
}

pub fn alpha94() -> Expr {
    power(
        rank(vwap() - ts_min(vwap(), 11)),
        ts_rank(correlation(ts_rank(vwap(), 19), ts_rank(adv(60), 4), 18), 2),
    ) * -1.0
}

pub fn alpha95() -> Expr {
    lt(
        rank(open() - ts_min(open(), 12)),
        ts_rank(
            power(
                rank(correlation(
                    ts_sum((high() + low()) / 2.0, 19),
                    ts_sum(adv(40), 19),
                    12,
                )),
                5.0,
            ),
            11,
        ),
    )
}

pub fn alpha96() -> Expr {
    max(
        ts_rank(
            decay_linear(correlation(rank(vwap()), rank(volume()), 3), 4),
            8,
        ),
        ts_rank(
            decay_linear(
                ts_argmax(correlation(ts_rank(close(), 7), ts_rank(adv(60), 4), 3), 12),
                14,
            ),
            13,
        ),
    ) * -1.0
}

pub fn alpha97() -> Expr {
    (rank(decay_linear(
        delta(
            group_neutralize(blend(low(), vwap(), 0.721001), industry()),
            3,
        ),
        20,
    )) - ts_rank(
        decay_linear(
            ts_rank(correlation(ts_rank(low(), 7), ts_rank(adv(60), 17), 4), 18),
            15,
        ),
        6,
    )) * -1.0
}

pub fn alpha98() -> Expr {
    rank(decay_linear(correlation(vwap(), ts_sum(adv(5), 26), 4), 7))
        - rank(decay_linear(
            ts_rank(
                ts_argmin(correlation(rank(open()), rank(adv(15)), 20), 8),
                6,
            ),
            8,
        ))
}

pub fn alpha99() -> Expr {
    -1.0 * lt(
        rank(correlation(
            ts_sum((high() + low()) / 2.0, 19),
            ts_sum(adv(60), 19),
            8,
        )),
        rank(correlation(low(), volume(), 6)),
    )
}

pub fn alpha100() -> Expr {
    let position = (((close() - low()) - (high() - close())) / (high() - low())) * volume();
    -1.0 * (((1.5
        * scale1(group_neutralize(
            group_neutralize(rank(position), subindustry()),
            subindustry(),
        )))
        - scale1(group_neutralize(
            correlation(close(), rank(adv(20)), 5) - rank(ts_argmin(close(), 30)),
            subindustry(),
        )))
        * (volume() / adv(20)))
}

pub fn alpha101() -> Expr {
    (close() - open()) / (high() - low() + 0.001)
}

/// Builds one alpha's expression.
type AlphaBuilder = fn() -> Expr;

pub fn worldquant_alpha101() -> Vec<(String, Expr)> {
    let alphas: [(&str, AlphaBuilder); 101] = [
        ("alpha1", alpha1 as fn() -> Expr),
        ("alpha2", alpha2 as fn() -> Expr),
        ("alpha3", alpha3 as fn() -> Expr),
        ("alpha4", alpha4 as fn() -> Expr),
        ("alpha5", alpha5 as fn() -> Expr),
        ("alpha6", alpha6 as fn() -> Expr),
        ("alpha7", alpha7 as fn() -> Expr),
        ("alpha8", alpha8 as fn() -> Expr),
        ("alpha9", alpha9 as fn() -> Expr),
        ("alpha10", alpha10 as fn() -> Expr),
        ("alpha11", alpha11 as fn() -> Expr),
        ("alpha12", alpha12 as fn() -> Expr),
        ("alpha13", alpha13 as fn() -> Expr),
        ("alpha14", alpha14 as fn() -> Expr),
        ("alpha15", alpha15 as fn() -> Expr),
        ("alpha16", alpha16 as fn() -> Expr),
        ("alpha17", alpha17 as fn() -> Expr),
        ("alpha18", alpha18 as fn() -> Expr),
        ("alpha19", alpha19 as fn() -> Expr),
        ("alpha20", alpha20 as fn() -> Expr),
        ("alpha21", alpha21 as fn() -> Expr),
        ("alpha22", alpha22 as fn() -> Expr),
        ("alpha23", alpha23 as fn() -> Expr),
        ("alpha24", alpha24 as fn() -> Expr),
        ("alpha25", alpha25 as fn() -> Expr),
        ("alpha26", alpha26 as fn() -> Expr),
        ("alpha27", alpha27 as fn() -> Expr),
        ("alpha28", alpha28 as fn() -> Expr),
        ("alpha29", alpha29 as fn() -> Expr),
        ("alpha30", alpha30 as fn() -> Expr),
        ("alpha31", alpha31 as fn() -> Expr),
        ("alpha32", alpha32 as fn() -> Expr),
        ("alpha33", alpha33 as fn() -> Expr),
        ("alpha34", alpha34 as fn() -> Expr),
        ("alpha35", alpha35 as fn() -> Expr),
        ("alpha36", alpha36 as fn() -> Expr),
        ("alpha37", alpha37 as fn() -> Expr),
        ("alpha38", alpha38 as fn() -> Expr),
        ("alpha39", alpha39 as fn() -> Expr),
        ("alpha40", alpha40 as fn() -> Expr),
        ("alpha41", alpha41 as fn() -> Expr),
        ("alpha42", alpha42 as fn() -> Expr),
        ("alpha43", alpha43 as fn() -> Expr),
        ("alpha44", alpha44 as fn() -> Expr),
        ("alpha45", alpha45 as fn() -> Expr),
        ("alpha46", alpha46 as fn() -> Expr),
        ("alpha47", alpha47 as fn() -> Expr),
        ("alpha48", alpha48 as fn() -> Expr),
        ("alpha49", alpha49 as fn() -> Expr),
        ("alpha50", alpha50 as fn() -> Expr),
        ("alpha51", alpha51 as fn() -> Expr),
        ("alpha52", alpha52 as fn() -> Expr),
        ("alpha53", alpha53 as fn() -> Expr),
        ("alpha54", alpha54 as fn() -> Expr),
        ("alpha55", alpha55 as fn() -> Expr),
        ("alpha56", alpha56 as fn() -> Expr),
        ("alpha57", alpha57 as fn() -> Expr),
        ("alpha58", alpha58 as fn() -> Expr),
        ("alpha59", alpha59 as fn() -> Expr),
        ("alpha60", alpha60 as fn() -> Expr),
        ("alpha61", alpha61 as fn() -> Expr),
        ("alpha62", alpha62 as fn() -> Expr),
        ("alpha63", alpha63 as fn() -> Expr),
        ("alpha64", alpha64 as fn() -> Expr),
        ("alpha65", alpha65 as fn() -> Expr),
        ("alpha66", alpha66 as fn() -> Expr),
        ("alpha67", alpha67 as fn() -> Expr),
        ("alpha68", alpha68 as fn() -> Expr),
        ("alpha69", alpha69 as fn() -> Expr),
        ("alpha70", alpha70 as fn() -> Expr),
        ("alpha71", alpha71 as fn() -> Expr),
        ("alpha72", alpha72 as fn() -> Expr),
        ("alpha73", alpha73 as fn() -> Expr),
        ("alpha74", alpha74 as fn() -> Expr),
        ("alpha75", alpha75 as fn() -> Expr),
        ("alpha76", alpha76 as fn() -> Expr),
        ("alpha77", alpha77 as fn() -> Expr),
        ("alpha78", alpha78 as fn() -> Expr),
        ("alpha79", alpha79 as fn() -> Expr),
        ("alpha80", alpha80 as fn() -> Expr),
        ("alpha81", alpha81 as fn() -> Expr),
        ("alpha82", alpha82 as fn() -> Expr),
        ("alpha83", alpha83 as fn() -> Expr),
        ("alpha84", alpha84 as fn() -> Expr),
        ("alpha85", alpha85 as fn() -> Expr),
        ("alpha86", alpha86 as fn() -> Expr),
        ("alpha87", alpha87 as fn() -> Expr),
        ("alpha88", alpha88 as fn() -> Expr),
        ("alpha89", alpha89 as fn() -> Expr),
        ("alpha90", alpha90 as fn() -> Expr),
        ("alpha91", alpha91 as fn() -> Expr),
        ("alpha92", alpha92 as fn() -> Expr),
        ("alpha93", alpha93 as fn() -> Expr),
        ("alpha94", alpha94 as fn() -> Expr),
        ("alpha95", alpha95 as fn() -> Expr),
        ("alpha96", alpha96 as fn() -> Expr),
        ("alpha97", alpha97 as fn() -> Expr),
        ("alpha98", alpha98 as fn() -> Expr),
        ("alpha99", alpha99 as fn() -> Expr),
        ("alpha100", alpha100 as fn() -> Expr),
        ("alpha101", alpha101 as fn() -> Expr),
    ];

    alphas
        .into_iter()
        .map(|(name, build)| (name.to_string(), build()))
        .collect()
}

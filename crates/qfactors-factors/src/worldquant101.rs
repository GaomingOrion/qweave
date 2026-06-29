use qfactors_core::A;
use qfactors_core::alpha::{
    abs, adv, cap, close, constant, correlation, covariance, decay_linear, delay, delta, field, ge,
    high, indneutralize, industry, le, log, low, lt, max, min, open, power, product, rank, returns,
    scale, sign, signedpower, stddev, sum, ts_argmax, ts_argmin, ts_max, ts_min, ts_rank, volume,
    vwap, where_,
};
use qfactors_macros::alpha;

fn c(value: f64) -> A {
    constant(value)
}

fn scale1(x: A) -> A {
    scale(x, 1.0)
}

fn sector() -> A {
    field("sector")
}

fn subindustry() -> A {
    field("subindustry")
}

fn mean(x: A, d: usize) -> A {
    sum(x, d) / d as f64
}

fn blend(lhs: A, rhs: A, lhs_weight: f64) -> A {
    lhs * lhs_weight + rhs * (1.0 - lhs_weight)
}

fn trend_20_10_0() -> A {
    ((delay(close(), 20) - delay(close(), 10)) / 10.0) - ((delay(close(), 10) - close()) / 10.0)
}

#[alpha]
pub fn alpha1() -> A {
    rank(ts_argmax(
        signedpower(
            where_(lt(returns(), c(0.0)), stddev(returns(), 20), close()),
            2.0,
        ),
        5,
    )) - 0.5
}

#[alpha]
pub fn alpha2() -> A {
    -1.0 * correlation(
        rank(delta(log(volume()), 2)),
        rank((close() - open()) / open()),
        6,
    )
}

#[alpha]
pub fn alpha3() -> A {
    -1.0 * correlation(rank(open()), rank(volume()), 10)
}

#[alpha]
pub fn alpha4() -> A {
    -1.0 * ts_rank(rank(low()), 9)
}

#[alpha]
pub fn alpha5() -> A {
    rank(open() - mean(vwap(), 10)) * (-1.0 * abs(rank(close() - vwap())))
}

#[alpha]
pub fn alpha7() -> A {
    where_(
        lt(adv(20), volume()),
        (-1.0 * ts_rank(abs(delta(close(), 7)), 60)) * sign(delta(close(), 7)),
        c(-1.0),
    )
}

#[alpha]
pub fn alpha9() -> A {
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

#[alpha]
pub fn alpha10() -> A {
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

#[alpha]
pub fn alpha11() -> A {
    (rank(ts_max(vwap() - close(), 3)) + rank(ts_min(vwap() - close(), 3)))
        * rank(delta(volume(), 3))
}

#[alpha]
pub fn alpha14() -> A {
    (-1.0 * rank(delta(returns(), 3))) * correlation(open(), volume(), 10)
}

#[alpha]
pub fn alpha15() -> A {
    -1.0 * sum(rank(correlation(rank(high()), rank(volume()), 3)), 3)
}

#[alpha]
pub fn alpha16() -> A {
    -1.0 * rank(covariance(rank(high()), rank(volume()), 5))
}

#[alpha]
pub fn alpha17() -> A {
    ((-1.0 * rank(ts_rank(close(), 10))) * rank(delta(delta(close(), 1), 1)))
        * rank(ts_rank(volume() / adv(20), 5))
}

#[alpha]
pub fn alpha18() -> A {
    -1.0 * rank(
        stddev(abs(close() - open()), 5) + (close() - open()) + correlation(close(), open(), 10),
    )
}

#[alpha]
pub fn alpha19() -> A {
    (-1.0 * sign((close() - delay(close(), 7)) + delta(close(), 7)))
        * (rank(sum(returns(), 250) + 1.0) + 1.0)
}

#[alpha]
pub fn alpha20() -> A {
    ((-1.0 * rank(open() - delay(high(), 1))) * rank(open() - delay(close(), 1)))
        * rank(open() - delay(low(), 1))
}

#[alpha]
pub fn alpha21() -> A {
    let close_mean8 = mean(close(), 8);
    let close_mean2 = mean(close(), 2);
    let close_std8 = stddev(close(), 8);
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

#[alpha]
pub fn alpha22() -> A {
    -1.0 * (delta(correlation(high(), volume(), 5), 5) * rank(stddev(close(), 20)))
}

#[alpha]
pub fn alpha23() -> A {
    where_(
        lt(mean(high(), 20), high()),
        -1.0 * delta(high(), 2),
        c(0.0),
    )
}

#[alpha]
pub fn alpha24() -> A {
    let close_mean100 = mean(close(), 100);
    where_(
        le(delta(close_mean100, 100) / delay(close(), 100), c(0.05)),
        -1.0 * (close() - ts_min(close(), 100)),
        -1.0 * delta(close(), 3),
    )
}

#[alpha]
pub fn alpha25() -> A {
    rank(((-1.0 * returns()) * adv(20)) * vwap() * (high() - close()))
}

#[alpha]
pub fn alpha26() -> A {
    -1.0 * ts_max(correlation(ts_rank(volume(), 5), ts_rank(high(), 5), 5), 3)
}

#[alpha]
pub fn alpha27() -> A {
    where_(
        lt(
            c(0.5),
            rank(mean(correlation(rank(volume()), rank(vwap()), 6), 2)),
        ),
        c(-1.0),
        c(1.0),
    )
}

#[alpha]
pub fn alpha28() -> A {
    scale1(correlation(adv(20), low(), 5) + ((high() + low()) / 2.0) - close())
}

#[alpha]
pub fn alpha29() -> A {
    let inner = rank(rank(-1.0 * rank(delta(close() - 1.0, 5))));
    ts_min(
        product(rank(rank(scale1(log(sum(ts_min(inner, 2), 1))))), 1),
        5,
    ) + ts_rank(delay(-1.0 * returns(), 6), 5)
}

#[alpha]
pub fn alpha30() -> A {
    let signs = sign(close() - delay(close(), 1))
        + sign(delay(close(), 1) - delay(close(), 2))
        + sign(delay(close(), 2) - delay(close(), 3));
    ((c(1.0) - rank(signs)) * sum(volume(), 5)) / sum(volume(), 20)
}

#[alpha]
pub fn alpha31() -> A {
    rank(rank(rank(decay_linear(
        -1.0 * rank(rank(delta(close(), 10))),
        10,
    )))) + rank(-1.0 * delta(close(), 3))
        + sign(scale1(correlation(adv(20), low(), 12)))
}

#[alpha]
pub fn alpha32() -> A {
    scale1(mean(close(), 7) - close()) + 20.0 * scale1(correlation(vwap(), delay(close(), 5), 230))
}

#[alpha]
pub fn alpha33() -> A {
    rank(-1.0 * power(c(1.0) - (open() / close()), 1.0))
}

#[alpha]
pub fn alpha34() -> A {
    rank(
        (c(1.0) - rank(stddev(returns(), 2) / stddev(returns(), 5)))
            + (c(1.0) - rank(delta(close(), 1))),
    )
}

#[alpha]
pub fn alpha35() -> A {
    (ts_rank(volume(), 32) * (c(1.0) - ts_rank((close() + high()) - low(), 16)))
        * (c(1.0) - ts_rank(returns(), 32))
}

#[alpha]
pub fn alpha36() -> A {
    2.21 * rank(correlation(close() - open(), delay(volume(), 1), 15))
        + 0.7 * rank(open() - close())
        + 0.73 * rank(ts_rank(delay(-1.0 * returns(), 6), 5))
        + rank(abs(correlation(vwap(), adv(20), 6)))
        + 0.6 * rank((mean(close(), 200) - open()) * (close() - open()))
}

#[alpha]
pub fn alpha37() -> A {
    rank(correlation(delay(open() - close(), 1), close(), 200)) + rank(open() - close())
}

#[alpha]
pub fn alpha38() -> A {
    (-1.0 * rank(ts_rank(close(), 10))) * rank(close() / open())
}

#[alpha]
pub fn alpha39() -> A {
    (-1.0 * rank(delta(close(), 7) * (c(1.0) - rank(decay_linear(volume() / adv(20), 9)))))
        * (c(1.0) + rank(sum(returns(), 250)))
}

#[alpha]
pub fn alpha40() -> A {
    (-1.0 * rank(stddev(high(), 10))) * correlation(high(), volume(), 10)
}

#[alpha]
pub fn alpha41() -> A {
    power(high() * low(), 0.5) - vwap()
}

#[alpha]
pub fn alpha42() -> A {
    rank(vwap() - close()) / rank(vwap() + close())
}

#[alpha]
pub fn alpha43() -> A {
    ts_rank(volume() / adv(20), 20) * ts_rank(-1.0 * delta(close(), 7), 8)
}

#[alpha]
pub fn alpha44() -> A {
    -1.0 * correlation(high(), rank(volume()), 5)
}

#[alpha]
pub fn alpha45() -> A {
    -1.0 * ((rank(mean(delay(close(), 5), 20)) * correlation(close(), volume(), 2))
        * rank(correlation(sum(close(), 5), sum(close(), 20), 2)))
}

#[alpha]
pub fn alpha46() -> A {
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

#[alpha]
pub fn alpha47() -> A {
    (((rank(c(1.0) / close()) * volume()) / adv(20))
        * ((high() * rank(high() - close())) / mean(high(), 5)))
        - rank(vwap() - delay(vwap(), 5))
}

#[alpha]
pub fn alpha48() -> A {
    indneutralize(
        (correlation(delta(close(), 1), delta(delay(close(), 1), 1), 250) * delta(close(), 1))
            / close(),
        subindustry(),
    ) / sum(power(delta(close(), 1) / delay(close(), 1), 2.0), 250)
}

#[alpha]
pub fn alpha49() -> A {
    where_(
        lt(trend_20_10_0(), c(-0.1)),
        c(1.0),
        -1.0 * (close() - delay(close(), 1)),
    )
}

#[alpha]
pub fn alpha50() -> A {
    -1.0 * ts_max(rank(correlation(rank(volume()), rank(vwap()), 5)), 5)
}

#[alpha]
pub fn alpha51() -> A {
    where_(
        lt(trend_20_10_0(), c(-0.05)),
        c(1.0),
        -1.0 * (close() - delay(close(), 1)),
    )
}

#[alpha]
pub fn alpha52() -> A {
    ((-1.0 * ts_min(low(), 5) + delay(ts_min(low(), 5), 5))
        * rank((sum(returns(), 240) - sum(returns(), 20)) / 220.0))
        * ts_rank(volume(), 5)
}

#[alpha]
pub fn alpha53() -> A {
    -1.0 * delta(
        ((close() - low()) - (high() - close())) / (close() - low()),
        9,
    )
}

#[alpha]
pub fn alpha54() -> A {
    (-1.0 * ((low() - close()) * power(open(), 5.0))) / ((low() - high()) * power(close(), 5.0))
}

#[alpha]
pub fn alpha55() -> A {
    -1.0 * correlation(
        rank((close() - ts_min(low(), 12)) / (ts_max(high(), 12) - ts_min(low(), 12))),
        rank(volume()),
        6,
    )
}

#[alpha]
pub fn alpha56() -> A {
    -1.0 * (rank(sum(returns(), 10) / sum(sum(returns(), 2), 3)) * rank(returns() * cap()))
}

#[alpha]
pub fn alpha57() -> A {
    -1.0 * ((close() - vwap()) / decay_linear(rank(ts_argmax(close(), 30)), 2))
}

#[alpha]
pub fn alpha58() -> A {
    -1.0 * ts_rank(
        decay_linear(correlation(indneutralize(vwap(), sector()), volume(), 3), 7),
        5,
    )
}

#[alpha]
pub fn alpha59() -> A {
    -1.0 * ts_rank(
        decay_linear(
            correlation(indneutralize(vwap(), industry()), volume(), 4),
            16,
        ),
        8,
    )
}

#[alpha]
pub fn alpha60() -> A {
    -1.0 * (2.0
        * scale1(rank(
            (((close() - low()) - (high() - close())) / (high() - low())) * volume(),
        ))
        - scale1(rank(ts_argmax(close(), 10))))
}

#[alpha]
pub fn alpha61() -> A {
    lt(
        rank(vwap() - ts_min(vwap(), 16)),
        rank(correlation(vwap(), adv(180), 17)),
    )
}

#[alpha]
pub fn alpha62() -> A {
    -1.0 * lt(
        rank(correlation(vwap(), sum(adv(20), 22), 9)),
        rank(lt(
            rank(open()) + rank(open()),
            rank((high() + low()) / 2.0) + rank(high()),
        )),
    )
}

#[alpha]
pub fn alpha63() -> A {
    (rank(decay_linear(
        delta(indneutralize(close(), industry()), 2),
        8,
    )) - rank(decay_linear(
        correlation(blend(vwap(), open(), 0.318108), sum(adv(180), 37), 13),
        12,
    ))) * -1.0
}

#[alpha]
pub fn alpha64() -> A {
    -1.0 * lt(
        rank(correlation(
            sum(blend(open(), low(), 0.178404), 12),
            sum(adv(120), 12),
            16,
        )),
        rank(delta(blend((high() + low()) / 2.0, vwap(), 0.178404), 3)),
    )
}

#[alpha]
pub fn alpha65() -> A {
    -1.0 * lt(
        rank(correlation(
            blend(open(), vwap(), 0.00817205),
            sum(adv(60), 8),
            6,
        )),
        rank(open() - ts_min(open(), 13)),
    )
}

#[alpha]
pub fn alpha66() -> A {
    (rank(decay_linear(delta(vwap(), 3), 7))
        + ts_rank(
            decay_linear((low() - vwap()) / (open() - ((high() + low()) / 2.0)), 11),
            6,
        ))
        * -1.0
}

#[alpha]
pub fn alpha67() -> A {
    power(
        rank(high() - ts_min(high(), 2)),
        rank(correlation(
            indneutralize(vwap(), sector()),
            indneutralize(adv(20), subindustry()),
            6,
        )),
    ) * -1.0
}

#[alpha]
pub fn alpha68() -> A {
    -1.0 * lt(
        ts_rank(correlation(rank(high()), rank(adv(15)), 8), 13),
        rank(delta(blend(close(), low(), 0.518371), 1)),
    )
}

#[alpha]
pub fn alpha69() -> A {
    power(
        rank(ts_max(delta(indneutralize(vwap(), industry()), 2), 4)),
        ts_rank(correlation(blend(close(), vwap(), 0.490655), adv(20), 4), 9),
    ) * -1.0
}

#[alpha]
pub fn alpha70() -> A {
    power(
        rank(delta(vwap(), 1)),
        ts_rank(
            correlation(indneutralize(close(), industry()), adv(50), 17),
            17,
        ),
    ) * -1.0
}

#[alpha]
pub fn alpha71() -> A {
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

#[alpha]
pub fn alpha72() -> A {
    rank(decay_linear(
        correlation((high() + low()) / 2.0, adv(40), 8),
        10,
    )) / rank(decay_linear(
        correlation(ts_rank(vwap(), 3), ts_rank(volume(), 18), 6),
        2,
    ))
}

#[alpha]
pub fn alpha73() -> A {
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

#[alpha]
pub fn alpha74() -> A {
    -1.0 * lt(
        rank(correlation(close(), sum(adv(30), 37), 15)),
        rank(correlation(
            rank(blend(high(), vwap(), 0.0261661)),
            rank(volume()),
            11,
        )),
    )
}

#[alpha]
pub fn alpha75() -> A {
    lt(
        rank(correlation(vwap(), volume(), 4)),
        rank(correlation(rank(low()), rank(adv(50)), 12)),
    )
}

#[alpha]
pub fn alpha76() -> A {
    max(
        rank(decay_linear(delta(vwap(), 1), 11)),
        ts_rank(
            decay_linear(
                ts_rank(correlation(indneutralize(low(), sector()), adv(81), 8), 19),
                17,
            ),
            19,
        ),
    ) * -1.0
}

#[alpha]
pub fn alpha77() -> A {
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

#[alpha]
pub fn alpha78() -> A {
    power(
        rank(correlation(
            sum(blend(low(), vwap(), 0.352233), 19),
            sum(adv(40), 19),
            6,
        )),
        rank(correlation(rank(vwap()), rank(volume()), 5)),
    )
}

#[alpha]
pub fn alpha79() -> A {
    lt(
        rank(delta(
            indneutralize(blend(close(), open(), 0.60733), sector()),
            1,
        )),
        rank(correlation(ts_rank(vwap(), 3), ts_rank(adv(150), 9), 14)),
    )
}

#[alpha]
pub fn alpha80() -> A {
    power(
        rank(sign(delta(
            indneutralize(blend(open(), high(), 0.868128), industry()),
            4,
        ))),
        ts_rank(correlation(high(), adv(10), 5), 5),
    ) * -1.0
}

#[alpha]
pub fn alpha81() -> A {
    -1.0 * lt(
        rank(log(product(
            rank(power(rank(correlation(vwap(), sum(adv(10), 49), 8)), 4.0)),
            14,
        ))),
        rank(correlation(rank(vwap()), rank(volume()), 5)),
    )
}

#[alpha]
pub fn alpha82() -> A {
    min(
        rank(decay_linear(delta(open(), 1), 14)),
        ts_rank(
            decay_linear(
                correlation(indneutralize(volume(), sector()), open(), 17),
                6,
            ),
            13,
        ),
    ) * -1.0
}

#[alpha]
pub fn alpha83() -> A {
    (rank(delay((high() - low()) / mean(close(), 5), 2)) * rank(rank(volume())))
        / (((high() - low()) / mean(close(), 5)) / (vwap() - close()))
}

#[alpha]
pub fn alpha84() -> A {
    signedpower(ts_rank(vwap() - ts_max(vwap(), 15), 20), delta(close(), 4))
}

#[alpha]
pub fn alpha85() -> A {
    power(
        rank(correlation(blend(high(), close(), 0.876703), adv(30), 9)),
        rank(correlation(
            ts_rank((high() + low()) / 2.0, 3),
            ts_rank(volume(), 10),
            7,
        )),
    )
}

#[alpha]
pub fn alpha86() -> A {
    -1.0 * lt(
        ts_rank(correlation(close(), sum(adv(20), 14), 6), 20),
        rank((open() + close()) - (vwap() + open())),
    )
}

#[alpha]
pub fn alpha87() -> A {
    max(
        rank(decay_linear(delta(blend(close(), vwap(), 0.369701), 1), 2)),
        ts_rank(
            decay_linear(
                abs(correlation(indneutralize(adv(81), industry()), close(), 13)),
                4,
            ),
            14,
        ),
    ) * -1.0
}

#[alpha]
pub fn alpha88() -> A {
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

#[alpha]
pub fn alpha89() -> A {
    ts_rank(decay_linear(correlation(low(), adv(10), 6), 5), 3)
        - ts_rank(
            decay_linear(delta(indneutralize(vwap(), industry()), 3), 10),
            15,
        )
}

#[alpha]
pub fn alpha90() -> A {
    power(
        rank(close() - ts_max(close(), 4)),
        ts_rank(
            correlation(indneutralize(adv(40), subindustry()), low(), 5),
            3,
        ),
    ) * -1.0
}

#[alpha]
pub fn alpha91() -> A {
    (ts_rank(
        decay_linear(
            decay_linear(
                correlation(indneutralize(close(), industry()), volume(), 9),
                16,
            ),
            3,
        ),
        4,
    ) - rank(decay_linear(correlation(vwap(), adv(30), 4), 2)))
        * -1.0
}

#[alpha]
pub fn alpha92() -> A {
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

#[alpha]
pub fn alpha93() -> A {
    ts_rank(
        decay_linear(
            correlation(indneutralize(vwap(), industry()), adv(81), 17),
            19,
        ),
        7,
    ) / rank(decay_linear(delta(blend(close(), vwap(), 0.524434), 2), 16))
}

#[alpha]
pub fn alpha94() -> A {
    power(
        rank(vwap() - ts_min(vwap(), 11)),
        ts_rank(correlation(ts_rank(vwap(), 19), ts_rank(adv(60), 4), 18), 2),
    ) * -1.0
}

#[alpha]
pub fn alpha95() -> A {
    lt(
        rank(open() - ts_min(open(), 12)),
        ts_rank(
            power(
                rank(correlation(
                    sum((high() + low()) / 2.0, 19),
                    sum(adv(40), 19),
                    12,
                )),
                5.0,
            ),
            11,
        ),
    )
}

#[alpha]
pub fn alpha96() -> A {
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

#[alpha]
pub fn alpha97() -> A {
    (rank(decay_linear(
        delta(indneutralize(blend(low(), vwap(), 0.721001), industry()), 3),
        20,
    )) - ts_rank(
        decay_linear(
            ts_rank(correlation(ts_rank(low(), 7), ts_rank(adv(60), 17), 4), 18),
            15,
        ),
        6,
    )) * -1.0
}

#[alpha]
pub fn alpha98() -> A {
    rank(decay_linear(correlation(vwap(), sum(adv(5), 26), 4), 7))
        - rank(decay_linear(
            ts_rank(
                ts_argmin(correlation(rank(open()), rank(adv(15)), 20), 8),
                6,
            ),
            8,
        ))
}

#[alpha]
pub fn alpha99() -> A {
    -1.0 * lt(
        rank(correlation(
            sum((high() + low()) / 2.0, 19),
            sum(adv(60), 19),
            8,
        )),
        rank(correlation(low(), volume(), 6)),
    )
}

#[alpha]
pub fn alpha100() -> A {
    let position = (((close() - low()) - (high() - close())) / (high() - low())) * volume();
    -1.0 * (((1.5
        * scale1(indneutralize(
            indneutralize(rank(position), subindustry()),
            subindustry(),
        )))
        - scale1(indneutralize(
            correlation(close(), rank(adv(20)), 5) - rank(ts_argmin(close(), 30)),
            subindustry(),
        )))
        * (volume() / adv(20)))
}

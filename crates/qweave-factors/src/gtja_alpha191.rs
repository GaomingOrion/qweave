#![allow(unused_parens, clippy::double_parens)]

// Formula structure follows Guotai Junan's 2017 Alpha191 report. The source
// locations and qweave-specific calibers are documented in docs/gtja_alpha191_sources.md.
use qweave_core::Expr;
use qweave_core::alpha::*;

fn c(value: f64) -> Expr {
    lit(value)
}
fn amount() -> Expr {
    volume() * vwap()
}
fn and_(a: Expr, b: Expr) -> Expr {
    where_(a, b, c(0.0))
}
fn or_(a: Expr, b: Expr) -> Expr {
    where_(a, c(1.0), b)
}
fn dtm() -> Expr {
    let p = delay(open(), 1);
    where_(
        le(open(), p.clone()),
        c(0.0),
        max(high() - open(), open() - p),
    )
}
fn dbm() -> Expr {
    let p = delay(open(), 1);
    where_(
        ge(open(), p.clone()),
        c(0.0),
        max(open() - low(), p - open()),
    )
}
fn hd() -> Expr {
    high() - delay(high(), 1)
}
fn ld() -> Expr {
    delay(low(), 1) - low()
}
fn tr() -> Expr {
    let p = delay(close(), 1);
    max(max(high() - low(), abs(high() - p.clone())), abs(low() - p))
}

fn alpha_001() -> Expr {
    let _open = open();
    ((-c(1.0))
        * correlation(
            rank(delta(log(volume()), 1)),
            rank(((close() - _open.clone()) / _open.clone())),
            6,
        ))
}

fn alpha_002() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    ((-c(1.0))
        * delta(
            (((_close.clone() - _low.clone()) - (_high.clone() - _close.clone()))
                / (_high.clone() - _low.clone())),
            1,
        ))
}

fn alpha_003() -> Expr {
    let _close = close();
    ts_sum(
        where_(
            eq(_close.clone(), delay(_close.clone(), 1)),
            c(0.0),
            (_close.clone()
                - where_(
                    gt(_close.clone(), delay(_close.clone(), 1)),
                    min(low(), delay(_close.clone(), 1)),
                    max(high(), delay(_close.clone(), 1)),
                )),
        ),
        6,
    )
}

fn alpha_004() -> Expr {
    let _close = close();
    let _volume = volume();
    where_(
        lt(
            ((ts_sum(_close.clone(), 8) / c(8.0)) + ts_std(_close.clone(), 8)),
            (ts_sum(_close.clone(), 2) / c(2.0)),
        ),
        ((-c(1.0)) * c(1.0)),
        where_(
            lt(
                (ts_sum(_close.clone(), 2) / c(2.0)),
                ((ts_sum(_close.clone(), 8) / c(8.0)) - ts_std(_close.clone(), 8)),
            ),
            c(1.0),
            where_(
                or_(
                    lt(c(1.0), (_volume.clone() / ts_mean(_volume.clone(), 20))),
                    eq((_volume.clone() / ts_mean(_volume.clone(), 20)), c(1.0)),
                ),
                c(1.0),
                ((-c(1.0)) * c(1.0)),
            ),
        ),
    )
}

fn alpha_005() -> Expr {
    ((-c(1.0)) * ts_max(correlation(ts_rank(volume(), 5), ts_rank(high(), 5), 5), 3))
}

fn alpha_006() -> Expr {
    (rank(sign(delta(((open() * c(0.85)) + (high() * c(0.15))), 4))) * (-c(1.0)))
}

fn alpha_007() -> Expr {
    let _close = close();
    let _vwap = vwap();
    ((rank(ts_max((_vwap.clone() - _close.clone()), 3))
        + rank(ts_min((_vwap.clone() - _close.clone()), 3)))
        * rank(delta(volume(), 3)))
}

fn alpha_008() -> Expr {
    rank(
        (delta(
            ((((high() + low()) / c(2.0)) * c(0.2)) + (vwap() * c(0.8))),
            4,
        ) * (-c(1.0))),
    )
}

fn alpha_009() -> Expr {
    let _high = high();
    let _low = low();
    sma(
        (((((_high.clone() + _low.clone()) / c(2.0))
            - ((delay(_high.clone(), 1) + delay(_low.clone(), 1)) / c(2.0)))
            * (_high.clone() - _low.clone()))
            / volume()),
        7,
        2,
    )
}

fn alpha_010() -> Expr {
    let _ret = returns();
    rank(ts_max(
        power(
            where_(lt(_ret.clone(), c(0.0)), ts_std(_ret.clone(), 20), close()),
            c(2.0),
        ),
        5,
    ))
}

fn alpha_011() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    ts_sum(
        ((((_close.clone() - _low.clone()) - (_high.clone() - _close.clone()))
            / (_high.clone() - _low.clone()))
            * volume()),
        6,
    )
}

fn alpha_012() -> Expr {
    let _vwap = vwap();
    (rank((open() - (ts_sum(_vwap.clone(), 10) / c(10.0))))
        * ((-c(1.0)) * rank(abs((close() - _vwap.clone())))))
}

fn alpha_013() -> Expr {
    (power((high() * low()), c(0.5)) - vwap())
}

fn alpha_014() -> Expr {
    let _close = close();
    (_close.clone() - delay(_close.clone(), 5))
}

fn alpha_015() -> Expr {
    ((open() / delay(close(), 1)) - c(1.0))
}

fn alpha_016() -> Expr {
    ((-c(1.0)) * ts_max(rank(correlation(rank(volume()), rank(vwap()), 5)), 5))
}

fn alpha_017() -> Expr {
    let _vwap = vwap();
    power(
        rank((_vwap.clone() - ts_max(_vwap.clone(), 15))),
        delta(close(), 5),
    )
}

fn alpha_018() -> Expr {
    let _close = close();
    (_close.clone() / delay(_close.clone(), 5))
}

fn alpha_019() -> Expr {
    let _close = close();
    where_(
        lt(_close.clone(), delay(_close.clone(), 5)),
        ((_close.clone() - delay(_close.clone(), 5)) / delay(_close.clone(), 5)),
        where_(
            eq(_close.clone(), delay(_close.clone(), 5)),
            c(0.0),
            ((_close.clone() - delay(_close.clone(), 5)) / _close.clone()),
        ),
    )
}

fn alpha_020() -> Expr {
    let _close = close();
    (((_close.clone() - delay(_close.clone(), 6)) / delay(_close.clone(), 6)) * c(100.0))
}

fn alpha_021() -> Expr {
    slope(ts_mean(close(), 6), 6)
}

fn alpha_022() -> Expr {
    let _close = close();
    sma(
        (((_close.clone() - ts_mean(_close.clone(), 6)) / ts_mean(_close.clone(), 6))
            - delay(
                ((_close.clone() - ts_mean(_close.clone(), 6)) / ts_mean(_close.clone(), 6)),
                3,
            )),
        12,
        1,
    )
}

fn alpha_023() -> Expr {
    let _close = close();
    ((sma(
        where_(
            gt(_close.clone(), delay(_close.clone(), 1)),
            ts_std(_close.clone(), 20),
            c(0.0),
        ),
        20,
        1,
    ) / (sma(
        where_(
            gt(_close.clone(), delay(_close.clone(), 1)),
            ts_std(_close.clone(), 20),
            c(0.0),
        ),
        20,
        1,
    ) + sma(
        where_(
            le(_close.clone(), delay(_close.clone(), 1)),
            ts_std(_close.clone(), 20),
            c(0.0),
        ),
        20,
        1,
    ))) * c(100.0))
}

fn alpha_024() -> Expr {
    let _close = close();
    sma((_close.clone() - delay(_close.clone(), 5)), 5, 1)
}

fn alpha_025() -> Expr {
    let _volume = volume();
    (((-c(1.0))
        * rank(
            (delta(close(), 7)
                * (c(1.0)
                    - rank(decay_linear(
                        (_volume.clone() / ts_mean(_volume.clone(), 20)),
                        9,
                    )))),
        ))
        * (c(1.0) + rank(ts_sum(returns(), 250))))
}

fn alpha_026() -> Expr {
    let _close = close();
    (((ts_sum(_close.clone(), 7) / c(7.0)) - _close.clone())
        + correlation(vwap(), delay(_close.clone(), 5), 230))
}

fn alpha_027() -> Expr {
    let _close = close();
    wma(
        ((((_close.clone() - delay(_close.clone(), 3)) / delay(_close.clone(), 3)) * c(100.0))
            + (((_close.clone() - delay(_close.clone(), 6)) / delay(_close.clone(), 6))
                * c(100.0))),
        12,
    )
}

fn alpha_028() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    ((c(3.0)
        * sma(
            (((_close.clone() - ts_min(_low.clone(), 9))
                / (ts_max(_high.clone(), 9) - ts_min(_low.clone(), 9)))
                * c(100.0)),
            3,
            1,
        ))
        - (c(2.0)
            * sma(
                sma(
                    (((_close.clone() - ts_min(_low.clone(), 9))
                        / (ts_max(_high.clone(), 9) - ts_max(_low.clone(), 9)))
                        * c(100.0)),
                    3,
                    1,
                ),
                3,
                1,
            )))
}

fn alpha_029() -> Expr {
    let _close = close();
    (((_close.clone() - delay(_close.clone(), 6)) / delay(_close.clone(), 6)) * volume())
}

fn alpha_030() -> Expr {
    let _close = close();
    wma(
        power(
            multi_resi(
                ((_close.clone() / delay(_close.clone(), 1)) - c(1.0)),
                col("mkt"),
                col("smb"),
                col("hml"),
                60,
            ),
            c(2.0),
        ),
        20,
    )
}

fn alpha_031() -> Expr {
    let _close = close();
    (((_close.clone() - ts_mean(_close.clone(), 12)) / ts_mean(_close.clone(), 12)) * c(100.0))
}

fn alpha_032() -> Expr {
    ((-c(1.0)) * ts_sum(rank(correlation(rank(high()), rank(volume()), 3)), 3))
}

fn alpha_033() -> Expr {
    let _low = low();
    let _ret = returns();
    (((((-c(1.0)) * ts_min(_low.clone(), 5)) + delay(ts_min(_low.clone(), 5), 5))
        * rank(((ts_sum(_ret.clone(), 240) - ts_sum(_ret.clone(), 20)) / c(220.0))))
        * ts_rank(volume(), 5))
}

fn alpha_034() -> Expr {
    let _close = close();
    (ts_mean(_close.clone(), 12) / _close.clone())
}

fn alpha_035() -> Expr {
    let _open = open();
    (min(
        rank(decay_linear(delta(_open.clone(), 1), 15)),
        rank(decay_linear(
            correlation(
                volume(),
                ((_open.clone() * c(0.65)) + (_open.clone() * c(0.35))),
                17,
            ),
            7,
        )),
    ) * (-c(1.0)))
}

fn alpha_036() -> Expr {
    rank(ts_sum(correlation(rank(volume()), rank(vwap()), 6), 2))
}

fn alpha_037() -> Expr {
    let _open = open();
    let _ret = returns();
    ((-c(1.0))
        * rank(
            ((ts_sum(_open.clone(), 5) * ts_sum(_ret.clone(), 5))
                - delay((ts_sum(_open.clone(), 5) * ts_sum(_ret.clone(), 5)), 10)),
        ))
}

fn alpha_038() -> Expr {
    let _high = high();
    where_(
        lt((ts_sum(_high.clone(), 20) / c(20.0)), _high.clone()),
        ((-c(1.0)) * delta(_high.clone(), 2)),
        c(0.0),
    )
}

fn alpha_039() -> Expr {
    ((rank(decay_linear(delta(close(), 2), 8))
        - rank(decay_linear(
            correlation(
                ((vwap() * c(0.3)) + (open() * c(0.7))),
                ts_sum(ts_mean(volume(), 180), 37),
                14,
            ),
            12,
        )))
        * (-c(1.0)))
}

fn alpha_040() -> Expr {
    let _close = close();
    let _volume = volume();
    ((ts_sum(
        where_(
            gt(_close.clone(), delay(_close.clone(), 1)),
            _volume.clone(),
            c(0.0),
        ),
        26,
    ) / ts_sum(
        where_(
            le(_close.clone(), delay(_close.clone(), 1)),
            _volume.clone(),
            c(0.0),
        ),
        26,
    )) * c(100.0))
}

fn alpha_041() -> Expr {
    (rank(ts_max(delta(vwap(), 3), 5)) * (-c(1.0)))
}

fn alpha_042() -> Expr {
    let _high = high();
    (((-c(1.0)) * rank(ts_std(_high.clone(), 10))) * correlation(_high.clone(), volume(), 10))
}

fn alpha_043() -> Expr {
    let _close = close();
    let _volume = volume();
    ts_sum(
        where_(
            gt(_close.clone(), delay(_close.clone(), 1)),
            _volume.clone(),
            where_(
                lt(_close.clone(), delay(_close.clone(), 1)),
                (-_volume.clone()),
                c(0.0),
            ),
        ),
        6,
    )
}

fn alpha_044() -> Expr {
    (ts_rank(
        decay_linear(correlation(low(), ts_mean(volume(), 10), 7), 6),
        4,
    ) + ts_rank(decay_linear(delta(vwap(), 3), 10), 15))
}

fn alpha_045() -> Expr {
    (rank(delta(((close() * c(0.6)) + (open() * c(0.4))), 1))
        * rank(correlation(vwap(), ts_mean(volume(), 150), 15)))
}

fn alpha_046() -> Expr {
    let _close = close();
    ((((ts_mean(_close.clone(), 3) + ts_mean(_close.clone(), 6)) + ts_mean(_close.clone(), 12))
        + ts_mean(_close.clone(), 24))
        / (c(4.0) * _close.clone()))
}

fn alpha_047() -> Expr {
    let _high = high();
    sma(
        (((ts_max(_high.clone(), 6) - close()) / (ts_max(_high.clone(), 6) - ts_min(low(), 6)))
            * c(100.0)),
        9,
        1,
    )
}

fn alpha_048() -> Expr {
    let _close = close();
    let _volume = volume();
    (((-c(1.0))
        * (rank(
            ((sign((_close.clone() - delay(_close.clone(), 1)))
                + sign((delay(_close.clone(), 1) - delay(_close.clone(), 2))))
                + sign((delay(_close.clone(), 2) - delay(_close.clone(), 3)))),
        ) * ts_sum(_volume.clone(), 5)))
        / ts_sum(_volume.clone(), 20))
}

fn alpha_049() -> Expr {
    let _high = high();
    let _low = low();
    (ts_sum(
        where_(
            ge(
                (_high.clone() + _low.clone()),
                (delay(_high.clone(), 1) + delay(_low.clone(), 1)),
            ),
            c(0.0),
            max(
                abs((_high.clone() - delay(_high.clone(), 1))),
                abs((_low.clone() - delay(_low.clone(), 1))),
            ),
        ),
        12,
    ) / (ts_sum(
        where_(
            ge(
                (_high.clone() + _low.clone()),
                (delay(_high.clone(), 1) + delay(_low.clone(), 1)),
            ),
            c(0.0),
            max(
                abs((_high.clone() - delay(_high.clone(), 1))),
                abs((_low.clone() - delay(_low.clone(), 1))),
            ),
        ),
        12,
    ) + ts_sum(
        where_(
            le(
                (_high.clone() + _low.clone()),
                (delay(_high.clone(), 1) + delay(_low.clone(), 1)),
            ),
            c(0.0),
            max(
                abs((_high.clone() - delay(_high.clone(), 1))),
                abs((_low.clone() - delay(_low.clone(), 1))),
            ),
        ),
        12,
    )))
}

fn alpha_050() -> Expr {
    let _high = high();
    let _low = low();
    ((ts_sum(
        where_(
            le(
                (_high.clone() + _low.clone()),
                (delay(_high.clone(), 1) + delay(_low.clone(), 1)),
            ),
            c(0.0),
            max(
                abs((_high.clone() - delay(_high.clone(), 1))),
                abs((_low.clone() - delay(_low.clone(), 1))),
            ),
        ),
        12,
    ) / (ts_sum(
        where_(
            le(
                (_high.clone() + _low.clone()),
                (delay(_high.clone(), 1) + delay(_low.clone(), 1)),
            ),
            c(0.0),
            max(
                abs((_high.clone() - delay(_high.clone(), 1))),
                abs((_low.clone() - delay(_low.clone(), 1))),
            ),
        ),
        12,
    ) + ts_sum(
        where_(
            ge(
                (_high.clone() + _low.clone()),
                (delay(_high.clone(), 1) + delay(_low.clone(), 1)),
            ),
            c(0.0),
            max(
                abs((_high.clone() - delay(_high.clone(), 1))),
                abs((_low.clone() - delay(_low.clone(), 1))),
            ),
        ),
        12,
    ))) - (ts_sum(
        where_(
            ge(
                (_high.clone() + _low.clone()),
                (delay(_high.clone(), 1) + delay(_low.clone(), 1)),
            ),
            c(0.0),
            max(
                abs((_high.clone() - delay(_high.clone(), 1))),
                abs((_low.clone() - delay(_low.clone(), 1))),
            ),
        ),
        12,
    ) / (ts_sum(
        where_(
            ge(
                (_high.clone() + _low.clone()),
                (delay(_high.clone(), 1) + delay(_low.clone(), 1)),
            ),
            c(0.0),
            max(
                abs((_high.clone() - delay(_high.clone(), 1))),
                abs((_low.clone() - delay(_low.clone(), 1))),
            ),
        ),
        12,
    ) + ts_sum(
        where_(
            le(
                (_high.clone() + _low.clone()),
                (delay(_high.clone(), 1) + delay(_low.clone(), 1)),
            ),
            c(0.0),
            max(
                abs((_high.clone() - delay(_high.clone(), 1))),
                abs((_low.clone() - delay(_low.clone(), 1))),
            ),
        ),
        12,
    ))))
}

fn alpha_051() -> Expr {
    let _high = high();
    let _low = low();
    (ts_sum(
        where_(
            le(
                (_high.clone() + _low.clone()),
                (delay(_high.clone(), 1) + delay(_low.clone(), 1)),
            ),
            c(0.0),
            max(
                abs((_high.clone() - delay(_high.clone(), 1))),
                abs((_low.clone() - delay(_low.clone(), 1))),
            ),
        ),
        12,
    ) / (ts_sum(
        where_(
            le(
                (_high.clone() + _low.clone()),
                (delay(_high.clone(), 1) + delay(_low.clone(), 1)),
            ),
            c(0.0),
            max(
                abs((_high.clone() - delay(_high.clone(), 1))),
                abs((_low.clone() - delay(_low.clone(), 1))),
            ),
        ),
        12,
    ) + ts_sum(
        where_(
            ge(
                (_high.clone() + _low.clone()),
                (delay(_high.clone(), 1) + delay(_low.clone(), 1)),
            ),
            c(0.0),
            max(
                abs((_high.clone() - delay(_high.clone(), 1))),
                abs((_low.clone() - delay(_low.clone(), 1))),
            ),
        ),
        12,
    )))
}

fn alpha_052() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    ((ts_sum(
        max(
            c(0.0),
            (_high.clone()
                - delay(
                    (((_high.clone() + _low.clone()) + _close.clone()) / c(3.0)),
                    1,
                )),
        ),
        26,
    ) / ts_sum(
        max(
            c(0.0),
            (delay(
                (((_high.clone() + _low.clone()) + _close.clone()) / c(3.0)),
                1,
            ) - _low.clone()),
        ),
        26,
    )) * c(100.0))
}

fn alpha_053() -> Expr {
    let _close = close();
    ((ts_sum(gt(_close.clone(), delay(_close.clone(), 1)), 12) / c(12.0)) * c(100.0))
}

fn alpha_054() -> Expr {
    let _close = close();
    let _open = open();
    ((-c(1.0))
        * rank(
            ((ts_std(abs((_close.clone() - _open.clone())), 10)
                + (_close.clone() - _open.clone()))
                + correlation(_close.clone(), _open.clone(), 10)),
        ))
}

fn alpha_055() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    let _open = open();
    ts_sum(
        (((c(16.0)
            * ((((_close.clone() - delay(_close.clone(), 1))
                + ((_close.clone() - _open.clone()) / c(2.0)))
                + delay(_close.clone(), 1))
                - delay(_open.clone(), 1)))
            / where_(
                and_(
                    gt(
                        abs((_high.clone() - delay(_close.clone(), 1))),
                        abs((_low.clone() - delay(_close.clone(), 1))),
                    ),
                    gt(
                        abs((_high.clone() - delay(_close.clone(), 1))),
                        abs((_high.clone() - delay(_low.clone(), 1))),
                    ),
                ),
                ((abs((_high.clone() - delay(_close.clone(), 1)))
                    + (abs((_low.clone() - delay(_close.clone(), 1))) / c(2.0)))
                    + (abs((delay(_close.clone(), 1) - delay(_open.clone(), 1))) / c(4.0))),
                where_(
                    and_(
                        gt(
                            abs((_low.clone() - delay(_close.clone(), 1))),
                            abs((_high.clone() - delay(_low.clone(), 1))),
                        ),
                        gt(
                            abs((_low.clone() - delay(_close.clone(), 1))),
                            abs((_high.clone() - delay(_close.clone(), 1))),
                        ),
                    ),
                    ((abs((_low.clone() - delay(_close.clone(), 1)))
                        + (abs((_high.clone() - delay(_close.clone(), 1))) / c(2.0)))
                        + (abs((delay(_close.clone(), 1) - delay(_open.clone(), 1))) / c(4.0))),
                    (abs((_high.clone() - delay(_low.clone(), 1)))
                        + (abs((delay(_close.clone(), 1) - delay(_open.clone(), 1))) / c(4.0))),
                ),
            ))
            * max(
                abs((_high.clone() - delay(_close.clone(), 1))),
                abs((_low.clone() - delay(_close.clone(), 1))),
            )),
        20,
    )
}

fn alpha_056() -> Expr {
    let _open = open();
    lt(
        rank((_open.clone() - ts_min(_open.clone(), 12))),
        rank(power(
            rank(correlation(
                ts_sum(((high() + low()) / c(2.0)), 19),
                ts_sum(ts_mean(volume(), 40), 19),
                13,
            )),
            c(5.0),
        )),
    )
}

fn alpha_057() -> Expr {
    let _low = low();
    sma(
        (((close() - ts_min(_low.clone(), 9)) / (ts_max(high(), 9) - ts_min(_low.clone(), 9)))
            * c(100.0)),
        3,
        1,
    )
}

fn alpha_058() -> Expr {
    let _close = close();
    ((ts_sum(gt(_close.clone(), delay(_close.clone(), 1)), 20) / c(20.0)) * c(100.0))
}

fn alpha_059() -> Expr {
    let _close = close();
    ts_sum(
        where_(
            eq(_close.clone(), delay(_close.clone(), 1)),
            c(0.0),
            (_close.clone()
                - where_(
                    gt(_close.clone(), delay(_close.clone(), 1)),
                    min(low(), delay(_close.clone(), 1)),
                    max(high(), delay(_close.clone(), 1)),
                )),
        ),
        20,
    )
}

fn alpha_060() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    ts_sum(
        ((((_close.clone() - _low.clone()) - (_high.clone() - _close.clone()))
            / (_high.clone() - _low.clone()))
            * volume()),
        20,
    )
}

fn alpha_061() -> Expr {
    (max(
        rank(decay_linear(delta(vwap(), 1), 12)),
        rank(decay_linear(
            rank(correlation(low(), ts_mean(volume(), 80), 8)),
            17,
        )),
    ) * (-c(1.0)))
}

fn alpha_062() -> Expr {
    ((-c(1.0)) * correlation(high(), rank(volume()), 5))
}

fn alpha_063() -> Expr {
    let _close = close();
    ((sma(ts_max((_close.clone() - delay(_close.clone(), 1)), 0), 6, 1)
        / sma(abs((_close.clone() - delay(_close.clone(), 1))), 6, 1))
        * c(100.0))
}

fn alpha_064() -> Expr {
    let _volume = volume();
    (max(
        rank(decay_linear(
            correlation(rank(vwap()), rank(_volume.clone()), 4),
            4,
        )),
        rank(decay_linear(
            ts_max(
                correlation(rank(close()), rank(ts_mean(_volume.clone(), 60)), 4),
                13,
            ),
            14,
        )),
    ) * (-c(1.0)))
}

fn alpha_065() -> Expr {
    let _close = close();
    (ts_mean(_close.clone(), 6) / _close.clone())
}

fn alpha_066() -> Expr {
    let _close = close();
    (((_close.clone() - ts_mean(_close.clone(), 6)) / ts_mean(_close.clone(), 6)) * c(100.0))
}

fn alpha_067() -> Expr {
    let _close = close();
    ((sma(
        ts_max((_close.clone() - delay(_close.clone(), 1)), 0),
        24,
        1,
    ) / sma(abs((_close.clone() - delay(_close.clone(), 1))), 24, 1))
        * c(100.0))
}

fn alpha_068() -> Expr {
    let _high = high();
    let _low = low();
    sma(
        (((((_high.clone() + _low.clone()) / c(2.0))
            - ((delay(_high.clone(), 1) + delay(_low.clone(), 1)) / c(2.0)))
            * (_high.clone() - _low.clone()))
            / volume()),
        15,
        2,
    )
}

fn alpha_069() -> Expr {
    let _dbm = dbm();
    let _dtm = dtm();
    where_(
        gt(ts_sum(_dtm.clone(), 20), ts_sum(_dbm.clone(), 20)),
        ((ts_sum(_dtm.clone(), 20) - ts_sum(_dbm.clone(), 20)) / ts_sum(_dtm.clone(), 20)),
        where_(
            eq(ts_sum(_dtm.clone(), 20), ts_sum(_dbm.clone(), 20)),
            c(0.0),
            ((ts_sum(_dtm.clone(), 20) - ts_sum(_dbm.clone(), 20)) / ts_sum(_dbm.clone(), 20)),
        ),
    )
}

fn alpha_070() -> Expr {
    ts_std(amount(), 6)
}

fn alpha_071() -> Expr {
    let _close = close();
    (((_close.clone() - ts_mean(_close.clone(), 24)) / ts_mean(_close.clone(), 24)) * c(100.0))
}

fn alpha_072() -> Expr {
    let _high = high();
    sma(
        (((ts_max(_high.clone(), 6) - close()) / (ts_max(_high.clone(), 6) - ts_min(low(), 6)))
            * c(100.0)),
        15,
        1,
    )
}

fn alpha_073() -> Expr {
    let _volume = volume();
    ((ts_rank(
        decay_linear(
            decay_linear(correlation(close(), _volume.clone(), 10), 16),
            4,
        ),
        5,
    ) - rank(decay_linear(
        correlation(vwap(), ts_mean(_volume.clone(), 30), 4),
        3,
    ))) * (-c(1.0)))
}

fn alpha_074() -> Expr {
    let _volume = volume();
    let _vwap = vwap();
    (rank(correlation(
        ts_sum(((low() * c(0.35)) + (_vwap.clone() * c(0.65))), 20),
        ts_sum(ts_mean(_volume.clone(), 40), 20),
        7,
    )) + rank(correlation(rank(_vwap.clone()), rank(_volume.clone()), 6)))
}

fn alpha_075() -> Expr {
    let _banchmarkindexclose = col("index_close");
    let _banchmarkindexopen = col("index_open");
    (ts_sum(
        and_(
            gt(close(), open()),
            lt(_banchmarkindexclose.clone(), _banchmarkindexopen.clone()),
        ),
        50,
    ) / ts_sum(
        lt(_banchmarkindexclose.clone(), _banchmarkindexopen.clone()),
        50,
    ))
}

fn alpha_076() -> Expr {
    let _close = close();
    let _volume = volume();
    (ts_std(
        (abs(((_close.clone() / delay(_close.clone(), 1)) - c(1.0))) / _volume.clone()),
        20,
    ) / ts_mean(
        (abs(((_close.clone() / delay(_close.clone(), 1)) - c(1.0))) / _volume.clone()),
        20,
    ))
}

fn alpha_077() -> Expr {
    let _high = high();
    let _low = low();
    min(
        rank(decay_linear(
            ((((_high.clone() + _low.clone()) / c(2.0)) + _high.clone())
                - (vwap() + _high.clone())),
            20,
        )),
        rank(decay_linear(
            correlation(
                ((_high.clone() + _low.clone()) / c(2.0)),
                ts_mean(volume(), 40),
                3,
            ),
            6,
        )),
    )
}

fn alpha_078() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    (((((_high.clone() + _low.clone()) + _close.clone()) / c(3.0))
        - ts_mean(
            (((_high.clone() + _low.clone()) + _close.clone()) / c(3.0)),
            12,
        ))
        / (c(0.015)
            * ts_mean(
                abs((_close.clone()
                    - ts_mean(
                        (((_high.clone() + _low.clone()) + _close.clone()) / c(3.0)),
                        12,
                    ))),
                12,
            )))
}

fn alpha_079() -> Expr {
    let _close = close();
    ((sma(
        ts_max((_close.clone() - delay(_close.clone(), 1)), 0),
        12,
        1,
    ) / sma(abs((_close.clone() - delay(_close.clone(), 1))), 12, 1))
        * c(100.0))
}

fn alpha_080() -> Expr {
    let _volume = volume();
    (((_volume.clone() - delay(_volume.clone(), 5)) / delay(_volume.clone(), 5)) * c(100.0))
}

fn alpha_081() -> Expr {
    sma(volume(), 21, 2)
}

fn alpha_082() -> Expr {
    let _high = high();
    sma(
        (((ts_max(_high.clone(), 6) - close()) / (ts_max(_high.clone(), 6) - ts_min(low(), 6)))
            * c(100.0)),
        20,
        1,
    )
}

fn alpha_083() -> Expr {
    ((-c(1.0)) * rank(covariance(rank(high()), rank(volume()), 5)))
}

fn alpha_084() -> Expr {
    let _close = close();
    let _volume = volume();
    ts_sum(
        where_(
            gt(_close.clone(), delay(_close.clone(), 1)),
            _volume.clone(),
            where_(
                lt(_close.clone(), delay(_close.clone(), 1)),
                (-_volume.clone()),
                c(0.0),
            ),
        ),
        20,
    )
}

fn alpha_085() -> Expr {
    let _volume = volume();
    (ts_rank((_volume.clone() / ts_mean(_volume.clone(), 20)), 20)
        * ts_rank(((-c(1.0)) * delta(close(), 7)), 8))
}

fn alpha_086() -> Expr {
    let _close = close();
    where_(
        lt(
            c(0.25),
            (((delay(_close.clone(), 20) - delay(_close.clone(), 10)) / c(10.0))
                - ((delay(_close.clone(), 10) - _close.clone()) / c(10.0))),
        ),
        ((-c(1.0)) * c(1.0)),
        where_(
            lt(
                (((delay(_close.clone(), 20) - delay(_close.clone(), 10)) / c(10.0))
                    - ((delay(_close.clone(), 10) - _close.clone()) / c(10.0))),
                c(0.0),
            ),
            c(1.0),
            (((-c(1.0)) * c(1.0)) * (_close.clone() - delay(_close.clone(), 1))),
        ),
    )
}

fn alpha_087() -> Expr {
    let _low = low();
    let _vwap = vwap();
    ((rank(decay_linear(delta(_vwap.clone(), 4), 7))
        + ts_rank(
            decay_linear(
                ((((_low.clone() * c(0.9)) + (_low.clone() * c(0.1))) - _vwap.clone())
                    / (open() - ((high() + _low.clone()) / c(2.0)))),
                11,
            ),
            7,
        ))
        * (-c(1.0)))
}

fn alpha_088() -> Expr {
    let _close = close();
    (((_close.clone() - delay(_close.clone(), 20)) / delay(_close.clone(), 20)) * c(100.0))
}

fn alpha_089() -> Expr {
    let _close = close();
    (c(2.0)
        * ((sma(_close.clone(), 13, 2) - sma(_close.clone(), 27, 2))
            - sma(
                (sma(_close.clone(), 13, 2) - sma(_close.clone(), 27, 2)),
                10,
                2,
            )))
}

fn alpha_090() -> Expr {
    (rank(correlation(rank(vwap()), rank(volume()), 5)) * (-c(1.0)))
}

fn alpha_091() -> Expr {
    let _close = close();
    ((rank((_close.clone() - ts_max(_close.clone(), 5)))
        * rank(correlation(ts_mean(volume(), 40), low(), 5)))
        * (-c(1.0)))
}

fn alpha_092() -> Expr {
    let _close = close();
    (max(
        rank(decay_linear(
            delta(((_close.clone() * c(0.35)) + (vwap() * c(0.65))), 2),
            3,
        )),
        ts_rank(
            decay_linear(
                abs(correlation(ts_mean(volume(), 180), _close.clone(), 13)),
                5,
            ),
            15,
        ),
    ) * (-c(1.0)))
}

fn alpha_093() -> Expr {
    let _open = open();
    ts_sum(
        where_(
            ge(_open.clone(), delay(_open.clone(), 1)),
            c(0.0),
            max(
                (_open.clone() - low()),
                (_open.clone() - delay(_open.clone(), 1)),
            ),
        ),
        20,
    )
}

fn alpha_094() -> Expr {
    let _close = close();
    let _volume = volume();
    ts_sum(
        where_(
            gt(_close.clone(), delay(_close.clone(), 1)),
            _volume.clone(),
            where_(
                lt(_close.clone(), delay(_close.clone(), 1)),
                (-_volume.clone()),
                c(0.0),
            ),
        ),
        30,
    )
}

fn alpha_095() -> Expr {
    ts_std(amount(), 20)
}

fn alpha_096() -> Expr {
    let _low = low();
    sma(
        sma(
            (((close() - ts_min(_low.clone(), 9)) / (ts_max(high(), 9) - ts_min(_low.clone(), 9)))
                * c(100.0)),
            3,
            1,
        ),
        3,
        1,
    )
}

fn alpha_097() -> Expr {
    ts_std(volume(), 10)
}

fn alpha_098() -> Expr {
    let _close = close();
    where_(
        or_(
            lt(
                (delta((ts_sum(_close.clone(), 100) / c(100.0)), 100) / delay(_close.clone(), 100)),
                c(0.05),
            ),
            eq(
                (delta((ts_sum(_close.clone(), 100) / c(100.0)), 100) / delay(_close.clone(), 100)),
                c(0.05),
            ),
        ),
        ((-c(1.0)) * (_close.clone() - ts_min(_close.clone(), 100))),
        ((-c(1.0)) * delta(_close.clone(), 3)),
    )
}

fn alpha_099() -> Expr {
    ((-c(1.0)) * rank(covariance(rank(close()), rank(volume()), 5)))
}

fn alpha_100() -> Expr {
    ts_std(volume(), 20)
}

fn alpha_101() -> Expr {
    let _volume = volume();
    (lt(
        rank(correlation(
            close(),
            ts_sum(ts_mean(_volume.clone(), 30), 37),
            15,
        )),
        rank(correlation(
            rank(((high() * c(0.1)) + (vwap() * c(0.9)))),
            rank(_volume.clone()),
            11,
        )),
    ) * (-c(1.0)))
}

fn alpha_102() -> Expr {
    let _volume = volume();
    ((sma(
        ts_max((_volume.clone() - delay(_volume.clone(), 1)), 0),
        6,
        1,
    ) / sma(abs((_volume.clone() - delay(_volume.clone(), 1))), 6, 1))
        * c(100.0))
}

fn alpha_103() -> Expr {
    (((c(20.0) - (c(19.0) - ts_argmin(low(), 20))) / c(20.0)) * c(100.0))
}

fn alpha_104() -> Expr {
    ((-c(1.0)) * (delta(correlation(high(), volume(), 5), 5) * rank(ts_std(close(), 20))))
}

fn alpha_105() -> Expr {
    ((-c(1.0)) * correlation(rank(open()), rank(volume()), 10))
}

fn alpha_106() -> Expr {
    let _close = close();
    (_close.clone() - delay(_close.clone(), 20))
}

fn alpha_107() -> Expr {
    let _open = open();
    ((((-c(1.0)) * rank((_open.clone() - delay(high(), 1))))
        * rank((_open.clone() - delay(close(), 1))))
        * rank((_open.clone() - delay(low(), 1))))
}

fn alpha_108() -> Expr {
    let _high = high();
    (power(
        rank((_high.clone() - ts_min(_high.clone(), 2))),
        rank(correlation(vwap(), ts_mean(volume(), 120), 6)),
    ) * (-c(1.0)))
}

fn alpha_109() -> Expr {
    let _high = high();
    let _low = low();
    (sma((_high.clone() - _low.clone()), 10, 2)
        / sma(sma((_high.clone() - _low.clone()), 10, 2), 10, 2))
}

fn alpha_110() -> Expr {
    let _close = close();
    ((ts_sum(max(c(0.0), (high() - delay(_close.clone(), 1))), 20)
        / ts_sum(max(c(0.0), (delay(_close.clone(), 1) - low())), 20))
        * c(100.0))
}

fn alpha_111() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    let _volume = volume();
    (sma(
        ((_volume.clone() * ((_close.clone() - _low.clone()) - (_high.clone() - _close.clone())))
            / (_high.clone() - _low.clone())),
        11,
        2,
    ) - sma(
        ((_volume.clone() * ((_close.clone() - _low.clone()) - (_high.clone() - _close.clone())))
            / (_high.clone() - _low.clone())),
        4,
        2,
    ))
}

fn alpha_112() -> Expr {
    let _close = close();
    (((ts_sum(
        where_(
            gt((_close.clone() - delay(_close.clone(), 1)), c(0.0)),
            (_close.clone() - delay(_close.clone(), 1)),
            c(0.0),
        ),
        12,
    ) - ts_sum(
        where_(
            lt((_close.clone() - delay(_close.clone(), 1)), c(0.0)),
            abs((_close.clone() - delay(_close.clone(), 1))),
            c(0.0),
        ),
        12,
    )) / (ts_sum(
        where_(
            gt((_close.clone() - delay(_close.clone(), 1)), c(0.0)),
            (_close.clone() - delay(_close.clone(), 1)),
            c(0.0),
        ),
        12,
    ) + ts_sum(
        where_(
            lt((_close.clone() - delay(_close.clone(), 1)), c(0.0)),
            abs((_close.clone() - delay(_close.clone(), 1))),
            c(0.0),
        ),
        12,
    ))) * c(100.0))
}

fn alpha_113() -> Expr {
    let _close = close();
    ((-c(1.0))
        * ((rank((ts_sum(delay(_close.clone(), 5), 20) / c(20.0)))
            * correlation(_close.clone(), volume(), 2))
            * rank(correlation(
                ts_sum(_close.clone(), 5),
                ts_sum(_close.clone(), 20),
                2,
            ))))
}

fn alpha_114() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    ((rank(delay(
        ((_high.clone() - _low.clone()) / (ts_sum(_close.clone(), 5) / c(5.0))),
        2,
    )) * rank(rank(volume())))
        / (((_high.clone() - _low.clone()) / (ts_sum(_close.clone(), 5) / c(5.0)))
            / (vwap() - _close.clone())))
}

fn alpha_115() -> Expr {
    let _high = high();
    let _volume = volume();
    power(
        rank(correlation(
            ((_high.clone() * c(0.9)) + (close() * c(0.1))),
            ts_mean(_volume.clone(), 30),
            10,
        )),
        rank(correlation(
            ts_rank(((_high.clone() + low()) / c(2.0)), 4),
            ts_rank(_volume.clone(), 10),
            7,
        )),
    )
}

fn alpha_116() -> Expr {
    slope(close(), 20)
}

fn alpha_117() -> Expr {
    ((ts_rank(volume(), 32) * (c(1.0) - ts_rank(((close() + high()) - low()), 16)))
        * (c(1.0) - ts_rank(returns(), 32)))
}

fn alpha_118() -> Expr {
    let _open = open();
    ((ts_sum((high() - _open.clone()), 20) / ts_sum((_open.clone() - low()), 20)) * c(100.0))
}

fn alpha_119() -> Expr {
    let _volume = volume();
    (rank(decay_linear(
        correlation(vwap(), ts_sum(ts_mean(_volume.clone(), 5), 26), 5),
        7,
    )) - rank(decay_linear(
        ts_rank(
            ts_min(
                correlation(rank(open()), rank(ts_mean(_volume.clone(), 15)), 21),
                9,
            ),
            7,
        ),
        8,
    )))
}

fn alpha_120() -> Expr {
    let _close = close();
    let _vwap = vwap();
    (rank((_vwap.clone() - _close.clone())) / rank((_vwap.clone() + _close.clone())))
}

fn alpha_121() -> Expr {
    let _vwap = vwap();
    (power(
        rank((_vwap.clone() - ts_min(_vwap.clone(), 12))),
        ts_rank(
            correlation(
                ts_rank(_vwap.clone(), 20),
                ts_rank(ts_mean(volume(), 60), 2),
                18,
            ),
            3,
        ),
    ) * (-c(1.0)))
}

fn alpha_122() -> Expr {
    let _close = close();
    ((sma(sma(sma(log(_close.clone()), 13, 2), 13, 2), 13, 2)
        - delay(sma(sma(sma(log(_close.clone()), 13, 2), 13, 2), 13, 2), 1))
        / delay(sma(sma(sma(log(_close.clone()), 13, 2), 13, 2), 13, 2), 1))
}

fn alpha_123() -> Expr {
    let _low = low();
    let _volume = volume();
    (lt(
        rank(correlation(
            ts_sum(((high() + _low.clone()) / c(2.0)), 20),
            ts_sum(ts_mean(_volume.clone(), 60), 20),
            9,
        )),
        rank(correlation(_low.clone(), _volume.clone(), 6)),
    ) * (-c(1.0)))
}

fn alpha_124() -> Expr {
    let _close = close();
    ((_close.clone() - vwap()) / decay_linear(rank(ts_max(_close.clone(), 30)), 2))
}

fn alpha_125() -> Expr {
    let _vwap = vwap();
    (rank(decay_linear(
        correlation(_vwap.clone(), ts_mean(volume(), 80), 17),
        20,
    )) / rank(decay_linear(
        delta(((close() * c(0.5)) + (_vwap.clone() * c(0.5))), 3),
        16,
    )))
}

fn alpha_126() -> Expr {
    (((close() + high()) + low()) / c(3.0))
}

fn alpha_127() -> Expr {
    let _close = close();
    power(
        ts_mean(
            power(
                ((c(100.0) * (_close.clone() - ts_max(_close.clone(), 12)))
                    / ts_max(_close.clone(), 12)),
                c(2.0),
            ),
            12,
        ),
        (c(1.0) / c(2.0)),
    )
}

fn alpha_128() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    let _volume = volume();
    (c(100.0)
        - (c(100.0)
            / (c(1.0)
                + (ts_sum(
                    where_(
                        gt(
                            (((_high.clone() + _low.clone()) + _close.clone()) / c(3.0)),
                            delay(
                                (((_high.clone() + _low.clone()) + _close.clone()) / c(3.0)),
                                1,
                            ),
                        ),
                        ((((_high.clone() + _low.clone()) + _close.clone()) / c(3.0))
                            * _volume.clone()),
                        c(0.0),
                    ),
                    14,
                ) / ts_sum(
                    where_(
                        lt(
                            (((_high.clone() + _low.clone()) + _close.clone()) / c(3.0)),
                            delay(
                                (((_high.clone() + _low.clone()) + _close.clone()) / c(3.0)),
                                1,
                            ),
                        ),
                        ((((_high.clone() + _low.clone()) + _close.clone()) / c(3.0))
                            * _volume.clone()),
                        c(0.0),
                    ),
                    14,
                )))))
}

fn alpha_129() -> Expr {
    let _close = close();
    ts_sum(
        where_(
            lt((_close.clone() - delay(_close.clone(), 1)), c(0.0)),
            abs((_close.clone() - delay(_close.clone(), 1))),
            c(0.0),
        ),
        12,
    )
}

fn alpha_130() -> Expr {
    let _volume = volume();
    (rank(decay_linear(
        correlation(((high() + low()) / c(2.0)), ts_mean(_volume.clone(), 40), 9),
        10,
    )) / rank(decay_linear(
        correlation(rank(vwap()), rank(_volume.clone()), 7),
        3,
    )))
}

fn alpha_131() -> Expr {
    power(
        rank(delta(vwap(), 1)),
        ts_rank(correlation(close(), ts_mean(volume(), 50), 18), 18),
    )
}

fn alpha_132() -> Expr {
    ts_mean(amount(), 20)
}

fn alpha_133() -> Expr {
    ((((c(20.0) - (c(19.0) - ts_argmax(high(), 20))) / c(20.0)) * c(100.0))
        - (((c(20.0) - (c(19.0) - ts_argmin(low(), 20))) / c(20.0)) * c(100.0)))
}

fn alpha_134() -> Expr {
    let _close = close();
    (((_close.clone() - delay(_close.clone(), 12)) / delay(_close.clone(), 12)) * volume())
}

fn alpha_135() -> Expr {
    let _close = close();
    sma(
        delay((_close.clone() / delay(_close.clone(), 20)), 1),
        20,
        1,
    )
}

fn alpha_136() -> Expr {
    (((-c(1.0)) * rank(delta(returns(), 3))) * correlation(open(), volume(), 10))
}

fn alpha_137() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    let _open = open();
    (((c(16.0)
        * ((((_close.clone() - delay(_close.clone(), 1))
            + ((_close.clone() - _open.clone()) / c(2.0)))
            + delay(_close.clone(), 1))
            - delay(_open.clone(), 1)))
        / where_(
            and_(
                gt(
                    abs((_high.clone() - delay(_close.clone(), 1))),
                    abs((_low.clone() - delay(_close.clone(), 1))),
                ),
                gt(
                    abs((_high.clone() - delay(_close.clone(), 1))),
                    abs((_high.clone() - delay(_low.clone(), 1))),
                ),
            ),
            ((abs((_high.clone() - delay(_close.clone(), 1)))
                + (abs((_low.clone() - delay(_close.clone(), 1))) / c(2.0)))
                + (abs((delay(_close.clone(), 1) - delay(_open.clone(), 1))) / c(4.0))),
            where_(
                and_(
                    gt(
                        abs((_low.clone() - delay(_close.clone(), 1))),
                        abs((_high.clone() - delay(_low.clone(), 1))),
                    ),
                    gt(
                        abs((_low.clone() - delay(_close.clone(), 1))),
                        abs((_high.clone() - delay(_close.clone(), 1))),
                    ),
                ),
                ((abs((_low.clone() - delay(_close.clone(), 1)))
                    + (abs((_high.clone() - delay(_close.clone(), 1))) / c(2.0)))
                    + (abs((delay(_close.clone(), 1) - delay(_open.clone(), 1))) / c(4.0))),
                (abs((_high.clone() - delay(_low.clone(), 1)))
                    + (abs((delay(_close.clone(), 1) - delay(_open.clone(), 1))) / c(4.0))),
            ),
        ))
        * max(
            abs((_high.clone() - delay(_close.clone(), 1))),
            abs((_low.clone() - delay(_close.clone(), 1))),
        ))
}

fn alpha_138() -> Expr {
    let _low = low();
    ((rank(decay_linear(
        delta(((_low.clone() * c(0.7)) + (vwap() * c(0.3))), 3),
        20,
    )) - ts_rank(
        decay_linear(
            ts_rank(
                correlation(
                    ts_rank(_low.clone(), 8),
                    ts_rank(ts_mean(volume(), 60), 17),
                    5,
                ),
                19,
            ),
            16,
        ),
        7,
    )) * (-c(1.0)))
}

fn alpha_139() -> Expr {
    ((-c(1.0)) * correlation(open(), volume(), 10))
}

fn alpha_140() -> Expr {
    let _close = close();
    min(
        rank(decay_linear(
            ((rank(open()) + rank(low())) - (rank(high()) + rank(_close.clone()))),
            8,
        )),
        ts_rank(
            decay_linear(
                correlation(
                    ts_rank(_close.clone(), 8),
                    ts_rank(ts_mean(volume(), 60), 20),
                    8,
                ),
                7,
            ),
            3,
        ),
    )
}

fn alpha_141() -> Expr {
    (rank(correlation(rank(high()), rank(ts_mean(volume(), 15)), 9)) * (-c(1.0)))
}

fn alpha_142() -> Expr {
    let _close = close();
    let _volume = volume();
    ((((-c(1.0)) * rank(ts_rank(_close.clone(), 10))) * rank(delta(delta(_close.clone(), 1), 1)))
        * rank(ts_rank((_volume.clone() / ts_mean(_volume.clone(), 20)), 5)))
}

fn alpha_143() -> Expr {
    let _close = close();
    scan_mul(
        ((_close.clone() - delay(_close.clone(), 1)) / delay(_close.clone(), 1)),
        gt(_close.clone(), delay(_close.clone(), 1)),
    )
}

fn alpha_144() -> Expr {
    let _close = close();
    (ts_sum(
        where_(
            lt(_close.clone(), delay(_close.clone(), 1)),
            (abs(((_close.clone() / delay(_close.clone(), 1)) - c(1.0))) / amount()),
            c(0.0),
        ),
        20,
    ) / ts_sum(lt(_close.clone(), delay(_close.clone(), 1)), 20))
}

fn alpha_145() -> Expr {
    let _volume = volume();
    (((ts_mean(_volume.clone(), 9) - ts_mean(_volume.clone(), 26)) / ts_mean(_volume.clone(), 12))
        * c(100.0))
}

fn alpha_146() -> Expr {
    let _close = close();
    ((ts_mean(
        (((_close.clone() - delay(_close.clone(), 1)) / delay(_close.clone(), 1))
            - sma(
                ((_close.clone() - delay(_close.clone(), 1)) / delay(_close.clone(), 1)),
                61,
                2,
            )),
        20,
    ) * (((_close.clone() - delay(_close.clone(), 1)) / delay(_close.clone(), 1))
        - sma(
            ((_close.clone() - delay(_close.clone(), 1)) / delay(_close.clone(), 1)),
            61,
            2,
        )))
        / sma(
            power(
                (((_close.clone() - delay(_close.clone(), 1)) / delay(_close.clone(), 1))
                    - (((_close.clone() - delay(_close.clone(), 1)) / delay(_close.clone(), 1))
                        - sma(
                            ((_close.clone() - delay(_close.clone(), 1))
                                / delay(_close.clone(), 1)),
                            61,
                            2,
                        ))),
                c(2.0),
            ),
            60,
            1,
        ))
}

fn alpha_147() -> Expr {
    slope(ts_mean(close(), 12), 12)
}

fn alpha_148() -> Expr {
    let _open = open();
    (lt(
        rank(correlation(
            _open.clone(),
            ts_sum(ts_mean(volume(), 60), 9),
            6,
        )),
        rank((_open.clone() - ts_min(_open.clone(), 14))),
    ) * (-c(1.0)))
}

fn alpha_149() -> Expr {
    let c0 = close();
    let index = col("index_close");
    let cond = lt(index.clone(), delay(index.clone(), 1));
    conditional_beta(
        c0.clone() / delay(c0, 1) - c(1.0),
        index.clone() / delay(index, 1) - c(1.0),
        cond,
        252,
    )
}

fn alpha_150() -> Expr {
    ((((close() + high()) + low()) / c(3.0)) * volume())
}

fn alpha_151() -> Expr {
    let _close = close();
    sma((_close.clone() - delay(_close.clone(), 20)), 20, 1)
}

fn alpha_152() -> Expr {
    let _close = close();
    sma(
        (ts_mean(
            delay(
                sma(delay((_close.clone() / delay(_close.clone(), 9)), 1), 9, 1),
                1,
            ),
            12,
        ) - ts_mean(
            delay(
                sma(delay((_close.clone() / delay(_close.clone(), 9)), 1), 9, 1),
                1,
            ),
            26,
        )),
        9,
        1,
    )
}

fn alpha_153() -> Expr {
    let _close = close();
    ((((ts_mean(_close.clone(), 3) + ts_mean(_close.clone(), 6)) + ts_mean(_close.clone(), 12))
        + ts_mean(_close.clone(), 24))
        / c(4.0))
}

fn alpha_154() -> Expr {
    let _vwap = vwap();
    lt(
        (_vwap.clone() - ts_min(_vwap.clone(), 16)),
        correlation(_vwap.clone(), ts_mean(volume(), 180), 18),
    )
}

fn alpha_155() -> Expr {
    let _volume = volume();
    ((sma(_volume.clone(), 13, 2) - sma(_volume.clone(), 27, 2))
        - sma(
            (sma(_volume.clone(), 13, 2) - sma(_volume.clone(), 27, 2)),
            10,
            2,
        ))
}

fn alpha_156() -> Expr {
    let _low = low();
    let _open = open();
    (max(
        rank(decay_linear(delta(vwap(), 5), 3)),
        rank(decay_linear(
            ((delta(((_open.clone() * c(0.15)) + (_low.clone() * c(0.85))), 2)
                / ((_open.clone() * c(0.15)) + (_low.clone() * c(0.85))))
                * (-c(1.0))),
            3,
        )),
    ) * (-c(1.0)))
}

fn alpha_157() -> Expr {
    (ts_min(
        product(
            rank(rank(log(ts_sum(
                ts_min(
                    rank(rank(((-c(1.0)) * rank(delta((close() - c(1.0)), 5))))),
                    2,
                ),
                1,
            )))),
            1,
        ),
        5,
    ) + ts_rank(delay(((-c(1.0)) * returns()), 6), 5))
}

fn alpha_158() -> Expr {
    let _close = close();
    (((high() - sma(_close.clone(), 15, 2)) - (low() - sma(_close.clone(), 15, 2)))
        / _close.clone())
}

fn alpha_159() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    ((((((((_close.clone() - ts_sum(min(_low.clone(), delay(_close.clone(), 1)), 6))
        / ts_sum(
            (max(_high.clone(), delay(_close.clone(), 1))
                - min(_low.clone(), delay(_close.clone(), 1))),
            6,
        ))
        * c(12.0))
        * c(24.0))
        + ((((_close.clone() - ts_sum(min(_low.clone(), delay(_close.clone(), 1)), 12))
            / ts_sum(
                (max(_high.clone(), delay(_close.clone(), 1))
                    - min(_low.clone(), delay(_close.clone(), 1))),
                12,
            ))
            * c(6.0))
            * c(24.0)))
        + ((((_close.clone() - ts_sum(min(_low.clone(), delay(_close.clone(), 1)), 24))
            / ts_sum(
                (max(_high.clone(), delay(_close.clone(), 1))
                    - min(_low.clone(), delay(_close.clone(), 1))),
                24,
            ))
            * c(6.0))
            * c(24.0)))
        * c(100.0))
        / (((c(6.0) * c(12.0)) + (c(6.0) * c(24.0))) + (c(12.0) * c(24.0))))
}

fn alpha_160() -> Expr {
    let _close = close();
    sma(
        where_(
            le(_close.clone(), delay(_close.clone(), 1)),
            ts_std(_close.clone(), 20),
            c(0.0),
        ),
        20,
        1,
    )
}

fn alpha_161() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    ts_mean(
        max(
            max(
                (_high.clone() - _low.clone()),
                abs((delay(_close.clone(), 1) - _high.clone())),
            ),
            abs((delay(_close.clone(), 1) - _low.clone())),
        ),
        12,
    )
}

fn alpha_162() -> Expr {
    let _close = close();
    ((((sma(
        ts_max((_close.clone() - delay(_close.clone(), 1)), 0),
        12,
        1,
    ) / sma(abs((_close.clone() - delay(_close.clone(), 1))), 12, 1))
        * c(100.0))
        - ts_min(
            ((sma(
                ts_max((_close.clone() - delay(_close.clone(), 1)), 0),
                12,
                1,
            ) / sma(abs((_close.clone() - delay(_close.clone(), 1))), 12, 1))
                * c(100.0)),
            12,
        ))
        / (ts_max(
            ((sma(
                ts_max((_close.clone() - delay(_close.clone(), 1)), 0),
                12,
                1,
            ) / sma(abs((_close.clone() - delay(_close.clone(), 1))), 12, 1))
                * c(100.0)),
            12,
        ) - ts_min(
            ((sma(
                ts_max((_close.clone() - delay(_close.clone(), 1)), 0),
                12,
                1,
            ) / sma(abs((_close.clone() - delay(_close.clone(), 1))), 12, 1))
                * c(100.0)),
            12,
        )))
}

fn alpha_163() -> Expr {
    rank((((((-c(1.0)) * returns()) * ts_mean(volume(), 20)) * vwap()) * (high() - close())))
}

fn alpha_164() -> Expr {
    let _close = close();
    sma(
        (((where_(
            gt(_close.clone(), delay(_close.clone(), 1)),
            (c(1.0) / (_close.clone() - delay(_close.clone(), 1))),
            c(1.0),
        ) - ts_min(
            where_(
                gt(_close.clone(), delay(_close.clone(), 1)),
                (c(1.0) / (_close.clone() - delay(_close.clone(), 1))),
                c(1.0),
            ),
            12,
        )) / (high() - low()))
            * c(100.0)),
        13,
        2,
    )
}

fn alpha_165() -> Expr {
    let _close = close();
    (ts_max(
        ts_sum((_close.clone() - ts_mean(_close.clone(), 48)), 48),
        48,
    ) - (ts_min(
        ts_sum((_close.clone() - ts_mean(_close.clone(), 48)), 48),
        48,
    ) / ts_std(_close.clone(), 48)))
}

fn alpha_166() -> Expr {
    let _close = close();
    ((((-c(20.0)) * power((c(20.0) - c(1.0)), c(1.5)))
        * ts_sum(
            (((_close.clone() / delay(_close.clone(), 1)) - c(1.0))
                - ts_mean(((_close.clone() / delay(_close.clone(), 1)) - c(1.0)), 20)),
            20,
        ))
        / power(
            (((c(20.0) - c(1.0)) * (c(20.0) - c(2.0)))
                * power(
                    ts_sum((_close.clone() / delay(_close.clone(), 1)), 20),
                    c(2.0),
                )),
            c(1.5),
        ))
}

fn alpha_167() -> Expr {
    let _close = close();
    ts_sum(
        where_(
            gt((_close.clone() - delay(_close.clone(), 1)), c(0.0)),
            (_close.clone() - delay(_close.clone(), 1)),
            c(0.0),
        ),
        12,
    )
}

fn alpha_168() -> Expr {
    let _volume = volume();
    (((-c(1.0)) * _volume.clone()) / ts_mean(_volume.clone(), 20))
}

fn alpha_169() -> Expr {
    let _close = close();
    sma(
        (ts_mean(
            delay(sma((_close.clone() - delay(_close.clone(), 1)), 9, 1), 1),
            12,
        ) - ts_mean(
            delay(sma((_close.clone() - delay(_close.clone(), 1)), 9, 1), 1),
            26,
        )),
        10,
        1,
    )
}

fn alpha_170() -> Expr {
    let _close = close();
    let _high = high();
    let _volume = volume();
    let _vwap = vwap();
    ((((rank((c(1.0) / _close.clone())) * _volume.clone()) / ts_mean(_volume.clone(), 20))
        * ((_high.clone() * rank((_high.clone() - _close.clone())))
            / (ts_sum(_high.clone(), 5) / c(5.0))))
        - rank((_vwap.clone() - delay(_vwap.clone(), 5))))
}

fn alpha_171() -> Expr {
    let _close = close();
    (((-c(1.0)) * ((low() - _close.clone()) * power(open(), c(5.0))))
        / ((_close.clone() - high()) * power(_close.clone(), c(5.0))))
}

fn alpha_172() -> Expr {
    let _hd = hd();
    let _ld = ld();
    let _tr = tr();
    ts_mean(
        ((abs((((ts_sum(
            where_(
                and_(gt(_ld.clone(), c(0.0)), gt(_ld.clone(), _hd.clone())),
                _ld.clone(),
                c(0.0),
            ),
            14,
        ) * c(100.0))
            / ts_sum(_tr.clone(), 14))
            - ((ts_sum(
                where_(
                    and_(gt(_hd.clone(), c(0.0)), gt(_hd.clone(), _ld.clone())),
                    _hd.clone(),
                    c(0.0),
                ),
                14,
            ) * c(100.0))
                / ts_sum(_tr.clone(), 14))))
            / (((ts_sum(
                where_(
                    and_(gt(_ld.clone(), c(0.0)), gt(_ld.clone(), _hd.clone())),
                    _ld.clone(),
                    c(0.0),
                ),
                14,
            ) * c(100.0))
                / ts_sum(_tr.clone(), 14))
                + ((ts_sum(
                    where_(
                        and_(gt(_hd.clone(), c(0.0)), gt(_hd.clone(), _ld.clone())),
                        _hd.clone(),
                        c(0.0),
                    ),
                    14,
                ) * c(100.0))
                    / ts_sum(_tr.clone(), 14))))
            * c(100.0)),
        6,
    )
}

fn alpha_173() -> Expr {
    let _close = close();
    (((c(3.0) * sma(_close.clone(), 13, 2)) - (c(2.0) * sma(sma(_close.clone(), 13, 2), 13, 2)))
        + sma(sma(sma(log(_close.clone()), 13, 2), 13, 2), 13, 2))
}

fn alpha_174() -> Expr {
    let _close = close();
    sma(
        where_(
            gt(_close.clone(), delay(_close.clone(), 1)),
            ts_std(_close.clone(), 20),
            c(0.0),
        ),
        20,
        1,
    )
}

fn alpha_175() -> Expr {
    let _close = close();
    let _high = high();
    let _low = low();
    ts_mean(
        max(
            max(
                (_high.clone() - _low.clone()),
                abs((delay(_close.clone(), 1) - _high.clone())),
            ),
            abs((delay(_close.clone(), 1) - _low.clone())),
        ),
        6,
    )
}

fn alpha_176() -> Expr {
    let _low = low();
    correlation(
        rank(
            ((close() - ts_min(_low.clone(), 12))
                / (ts_max(high(), 12) - ts_min(_low.clone(), 12))),
        ),
        rank(volume()),
        6,
    )
}

fn alpha_177() -> Expr {
    (((c(20.0) - (c(19.0) - ts_argmax(high(), 20))) / c(20.0)) * c(100.0))
}

fn alpha_178() -> Expr {
    let _close = close();
    (((_close.clone() - delay(_close.clone(), 1)) / delay(_close.clone(), 1)) * volume())
}

fn alpha_179() -> Expr {
    let _volume = volume();
    (rank(correlation(vwap(), _volume.clone(), 4))
        * rank(correlation(
            rank(low()),
            rank(ts_mean(_volume.clone(), 50)),
            12,
        )))
}

fn alpha_180() -> Expr {
    let _close = close();
    let _volume = volume();
    where_(
        lt(ts_mean(_volume.clone(), 20), _volume.clone()),
        (((-c(1.0)) * ts_rank(abs(delta(_close.clone(), 7)), 60)) * sign(delta(_close.clone(), 7))),
        ((-c(1.0)) * _volume.clone()),
    )
}

fn alpha_181() -> Expr {
    let _banchmarkindexclose = col("index_close");
    let _close = close();
    (ts_sum(
        ((((_close.clone() / delay(_close.clone(), 1)) - c(1.0))
            - ts_mean(((_close.clone() / delay(_close.clone(), 1)) - c(1.0)), 20))
            - power(
                (_banchmarkindexclose.clone() - ts_mean(_banchmarkindexclose.clone(), 20)),
                c(2.0),
            )),
        20,
    ) / ts_sum(
        power(
            (_banchmarkindexclose.clone() - ts_mean(_banchmarkindexclose.clone(), 20)),
            c(3.0),
        ),
        20,
    ))
}

fn alpha_182() -> Expr {
    let _banchmarkindexclose = col("index_close");
    let _banchmarkindexopen = col("index_open");
    let _close = close();
    let _open = open();
    (ts_sum(
        or_(
            and_(
                gt(_close.clone(), _open.clone()),
                gt(_banchmarkindexclose.clone(), _banchmarkindexopen.clone()),
            ),
            and_(
                lt(_close.clone(), _open.clone()),
                lt(_banchmarkindexclose.clone(), _banchmarkindexopen.clone()),
            ),
        ),
        20,
    ) / c(20.0))
}

fn alpha_183() -> Expr {
    let _close = close();
    (ts_max(
        ts_sum((_close.clone() - ts_mean(_close.clone(), 24)), 24),
        24,
    ) - (ts_min(
        ts_sum((_close.clone() - ts_mean(_close.clone(), 24)), 24),
        24,
    ) / ts_std(_close.clone(), 24)))
}

fn alpha_184() -> Expr {
    let _close = close();
    let _open = open();
    (rank(correlation(
        delay((_open.clone() - _close.clone()), 1),
        _close.clone(),
        200,
    )) + rank((_open.clone() - _close.clone())))
}

fn alpha_185() -> Expr {
    rank(((-c(1.0)) * power((c(1.0) - (open() / close())), c(2.0))))
}

fn alpha_186() -> Expr {
    let _hd = hd();
    let _ld = ld();
    let _tr = tr();
    ((ts_mean(
        ((abs((((ts_sum(
            where_(
                and_(gt(_ld.clone(), c(0.0)), gt(_ld.clone(), _hd.clone())),
                _ld.clone(),
                c(0.0),
            ),
            14,
        ) * c(100.0))
            / ts_sum(_tr.clone(), 14))
            - ((ts_sum(
                where_(
                    and_(gt(_hd.clone(), c(0.0)), gt(_hd.clone(), _ld.clone())),
                    _hd.clone(),
                    c(0.0),
                ),
                14,
            ) * c(100.0))
                / ts_sum(_tr.clone(), 14))))
            / (((ts_sum(
                where_(
                    and_(gt(_ld.clone(), c(0.0)), gt(_ld.clone(), _hd.clone())),
                    _ld.clone(),
                    c(0.0),
                ),
                14,
            ) * c(100.0))
                / ts_sum(_tr.clone(), 14))
                + ((ts_sum(
                    where_(
                        and_(gt(_hd.clone(), c(0.0)), gt(_hd.clone(), _ld.clone())),
                        _hd.clone(),
                        c(0.0),
                    ),
                    14,
                ) * c(100.0))
                    / ts_sum(_tr.clone(), 14))))
            * c(100.0)),
        6,
    ) + delay(
        ts_mean(
            ((abs((((ts_sum(
                where_(
                    and_(gt(_ld.clone(), c(0.0)), gt(_ld.clone(), _hd.clone())),
                    _ld.clone(),
                    c(0.0),
                ),
                14,
            ) * c(100.0))
                / ts_sum(_tr.clone(), 14))
                - ((ts_sum(
                    where_(
                        and_(gt(_hd.clone(), c(0.0)), gt(_hd.clone(), _ld.clone())),
                        _hd.clone(),
                        c(0.0),
                    ),
                    14,
                ) * c(100.0))
                    / ts_sum(_tr.clone(), 14))))
                / (((ts_sum(
                    where_(
                        and_(gt(_ld.clone(), c(0.0)), gt(_ld.clone(), _hd.clone())),
                        _ld.clone(),
                        c(0.0),
                    ),
                    14,
                ) * c(100.0))
                    / ts_sum(_tr.clone(), 14))
                    + ((ts_sum(
                        where_(
                            and_(gt(_hd.clone(), c(0.0)), gt(_hd.clone(), _ld.clone())),
                            _hd.clone(),
                            c(0.0),
                        ),
                        14,
                    ) * c(100.0))
                        / ts_sum(_tr.clone(), 14))))
                * c(100.0)),
            6,
        ),
        6,
    )) / c(2.0))
}

fn alpha_187() -> Expr {
    let _open = open();
    ts_sum(
        where_(
            le(_open.clone(), delay(_open.clone(), 1)),
            c(0.0),
            max(
                (high() - _open.clone()),
                (_open.clone() - delay(_open.clone(), 1)),
            ),
        ),
        20,
    )
}

fn alpha_188() -> Expr {
    let _high = high();
    let _low = low();
    ((((_high.clone() - _low.clone()) - sma((_high.clone() - _low.clone()), 11, 2))
        / sma((_high.clone() - _low.clone()), 11, 2))
        * c(100.0))
}

fn alpha_189() -> Expr {
    let _close = close();
    ts_mean(abs((_close.clone() - ts_mean(_close.clone(), 6))), 6)
}

fn alpha_190() -> Expr {
    let _close = close();
    log((((ts_sum(
        gt(
            ((_close.clone() / delay(_close.clone(), 1)) - c(1.0)),
            (power(
                (_close.clone() / delay(_close.clone(), 19)),
                (c(1.0) / c(20.0)),
            ) - c(1.0)),
        ),
        20,
    ) - c(1.0))
        * ts_sum(
            where_(
                lt(
                    ((_close.clone() / delay(_close.clone(), 1)) - c(1.0)),
                    (power(
                        (_close.clone() / delay(_close.clone(), 19)),
                        (c(1.0) / c(20.0)),
                    ) - c(1.0)),
                ),
                power(
                    ((((_close.clone() / delay(_close.clone(), 1)) - c(1.0))
                        - power(
                            (_close.clone() / delay(_close.clone(), 19)),
                            (c(1.0) / c(20.0)),
                        ))
                        - c(1.0)),
                    c(2.0),
                ),
                c(0.0),
            ),
            20,
        ))
        / (ts_sum(
            lt(
                ((_close.clone() / delay(_close.clone(), 1)) - c(1.0)),
                (power(
                    (_close.clone() / delay(_close.clone(), 19)),
                    (c(1.0) / c(20.0)),
                ) - c(1.0)),
            ),
            20,
        ) * ts_sum(
            where_(
                gt(
                    ((_close.clone() / delay(_close.clone(), 1)) - c(1.0)),
                    (power(
                        (_close.clone() / delay(_close.clone(), 19)),
                        (c(1.0) / c(20.0)),
                    ) - c(1.0)),
                ),
                power(
                    (((_close.clone() / delay(_close.clone(), 1)) - c(1.0))
                        - (power(
                            (_close.clone() / delay(_close.clone(), 19)),
                            (c(1.0) / c(20.0)),
                        ) - c(1.0))),
                    c(2.0),
                ),
                c(0.0),
            ),
            20,
        ))))
}

fn alpha_191() -> Expr {
    let _low = low();
    ((correlation(ts_mean(volume(), 20), _low.clone(), 5) + ((high() + _low.clone()) / c(2.0)))
        - close())
}

pub fn gtja_alpha191() -> Vec<(String, Expr)> {
    vec![
        ("gtja_alpha001".to_string(), alpha_001()),
        ("gtja_alpha002".to_string(), alpha_002()),
        ("gtja_alpha003".to_string(), alpha_003()),
        ("gtja_alpha004".to_string(), alpha_004()),
        ("gtja_alpha005".to_string(), alpha_005()),
        ("gtja_alpha006".to_string(), alpha_006()),
        ("gtja_alpha007".to_string(), alpha_007()),
        ("gtja_alpha008".to_string(), alpha_008()),
        ("gtja_alpha009".to_string(), alpha_009()),
        ("gtja_alpha010".to_string(), alpha_010()),
        ("gtja_alpha011".to_string(), alpha_011()),
        ("gtja_alpha012".to_string(), alpha_012()),
        ("gtja_alpha013".to_string(), alpha_013()),
        ("gtja_alpha014".to_string(), alpha_014()),
        ("gtja_alpha015".to_string(), alpha_015()),
        ("gtja_alpha016".to_string(), alpha_016()),
        ("gtja_alpha017".to_string(), alpha_017()),
        ("gtja_alpha018".to_string(), alpha_018()),
        ("gtja_alpha019".to_string(), alpha_019()),
        ("gtja_alpha020".to_string(), alpha_020()),
        ("gtja_alpha021".to_string(), alpha_021()),
        ("gtja_alpha022".to_string(), alpha_022()),
        ("gtja_alpha023".to_string(), alpha_023()),
        ("gtja_alpha024".to_string(), alpha_024()),
        ("gtja_alpha025".to_string(), alpha_025()),
        ("gtja_alpha026".to_string(), alpha_026()),
        ("gtja_alpha027".to_string(), alpha_027()),
        ("gtja_alpha028".to_string(), alpha_028()),
        ("gtja_alpha029".to_string(), alpha_029()),
        ("gtja_alpha030".to_string(), alpha_030()),
        ("gtja_alpha031".to_string(), alpha_031()),
        ("gtja_alpha032".to_string(), alpha_032()),
        ("gtja_alpha033".to_string(), alpha_033()),
        ("gtja_alpha034".to_string(), alpha_034()),
        ("gtja_alpha035".to_string(), alpha_035()),
        ("gtja_alpha036".to_string(), alpha_036()),
        ("gtja_alpha037".to_string(), alpha_037()),
        ("gtja_alpha038".to_string(), alpha_038()),
        ("gtja_alpha039".to_string(), alpha_039()),
        ("gtja_alpha040".to_string(), alpha_040()),
        ("gtja_alpha041".to_string(), alpha_041()),
        ("gtja_alpha042".to_string(), alpha_042()),
        ("gtja_alpha043".to_string(), alpha_043()),
        ("gtja_alpha044".to_string(), alpha_044()),
        ("gtja_alpha045".to_string(), alpha_045()),
        ("gtja_alpha046".to_string(), alpha_046()),
        ("gtja_alpha047".to_string(), alpha_047()),
        ("gtja_alpha048".to_string(), alpha_048()),
        ("gtja_alpha049".to_string(), alpha_049()),
        ("gtja_alpha050".to_string(), alpha_050()),
        ("gtja_alpha051".to_string(), alpha_051()),
        ("gtja_alpha052".to_string(), alpha_052()),
        ("gtja_alpha053".to_string(), alpha_053()),
        ("gtja_alpha054".to_string(), alpha_054()),
        ("gtja_alpha055".to_string(), alpha_055()),
        ("gtja_alpha056".to_string(), alpha_056()),
        ("gtja_alpha057".to_string(), alpha_057()),
        ("gtja_alpha058".to_string(), alpha_058()),
        ("gtja_alpha059".to_string(), alpha_059()),
        ("gtja_alpha060".to_string(), alpha_060()),
        ("gtja_alpha061".to_string(), alpha_061()),
        ("gtja_alpha062".to_string(), alpha_062()),
        ("gtja_alpha063".to_string(), alpha_063()),
        ("gtja_alpha064".to_string(), alpha_064()),
        ("gtja_alpha065".to_string(), alpha_065()),
        ("gtja_alpha066".to_string(), alpha_066()),
        ("gtja_alpha067".to_string(), alpha_067()),
        ("gtja_alpha068".to_string(), alpha_068()),
        ("gtja_alpha069".to_string(), alpha_069()),
        ("gtja_alpha070".to_string(), alpha_070()),
        ("gtja_alpha071".to_string(), alpha_071()),
        ("gtja_alpha072".to_string(), alpha_072()),
        ("gtja_alpha073".to_string(), alpha_073()),
        ("gtja_alpha074".to_string(), alpha_074()),
        ("gtja_alpha075".to_string(), alpha_075()),
        ("gtja_alpha076".to_string(), alpha_076()),
        ("gtja_alpha077".to_string(), alpha_077()),
        ("gtja_alpha078".to_string(), alpha_078()),
        ("gtja_alpha079".to_string(), alpha_079()),
        ("gtja_alpha080".to_string(), alpha_080()),
        ("gtja_alpha081".to_string(), alpha_081()),
        ("gtja_alpha082".to_string(), alpha_082()),
        ("gtja_alpha083".to_string(), alpha_083()),
        ("gtja_alpha084".to_string(), alpha_084()),
        ("gtja_alpha085".to_string(), alpha_085()),
        ("gtja_alpha086".to_string(), alpha_086()),
        ("gtja_alpha087".to_string(), alpha_087()),
        ("gtja_alpha088".to_string(), alpha_088()),
        ("gtja_alpha089".to_string(), alpha_089()),
        ("gtja_alpha090".to_string(), alpha_090()),
        ("gtja_alpha091".to_string(), alpha_091()),
        ("gtja_alpha092".to_string(), alpha_092()),
        ("gtja_alpha093".to_string(), alpha_093()),
        ("gtja_alpha094".to_string(), alpha_094()),
        ("gtja_alpha095".to_string(), alpha_095()),
        ("gtja_alpha096".to_string(), alpha_096()),
        ("gtja_alpha097".to_string(), alpha_097()),
        ("gtja_alpha098".to_string(), alpha_098()),
        ("gtja_alpha099".to_string(), alpha_099()),
        ("gtja_alpha100".to_string(), alpha_100()),
        ("gtja_alpha101".to_string(), alpha_101()),
        ("gtja_alpha102".to_string(), alpha_102()),
        ("gtja_alpha103".to_string(), alpha_103()),
        ("gtja_alpha104".to_string(), alpha_104()),
        ("gtja_alpha105".to_string(), alpha_105()),
        ("gtja_alpha106".to_string(), alpha_106()),
        ("gtja_alpha107".to_string(), alpha_107()),
        ("gtja_alpha108".to_string(), alpha_108()),
        ("gtja_alpha109".to_string(), alpha_109()),
        ("gtja_alpha110".to_string(), alpha_110()),
        ("gtja_alpha111".to_string(), alpha_111()),
        ("gtja_alpha112".to_string(), alpha_112()),
        ("gtja_alpha113".to_string(), alpha_113()),
        ("gtja_alpha114".to_string(), alpha_114()),
        ("gtja_alpha115".to_string(), alpha_115()),
        ("gtja_alpha116".to_string(), alpha_116()),
        ("gtja_alpha117".to_string(), alpha_117()),
        ("gtja_alpha118".to_string(), alpha_118()),
        ("gtja_alpha119".to_string(), alpha_119()),
        ("gtja_alpha120".to_string(), alpha_120()),
        ("gtja_alpha121".to_string(), alpha_121()),
        ("gtja_alpha122".to_string(), alpha_122()),
        ("gtja_alpha123".to_string(), alpha_123()),
        ("gtja_alpha124".to_string(), alpha_124()),
        ("gtja_alpha125".to_string(), alpha_125()),
        ("gtja_alpha126".to_string(), alpha_126()),
        ("gtja_alpha127".to_string(), alpha_127()),
        ("gtja_alpha128".to_string(), alpha_128()),
        ("gtja_alpha129".to_string(), alpha_129()),
        ("gtja_alpha130".to_string(), alpha_130()),
        ("gtja_alpha131".to_string(), alpha_131()),
        ("gtja_alpha132".to_string(), alpha_132()),
        ("gtja_alpha133".to_string(), alpha_133()),
        ("gtja_alpha134".to_string(), alpha_134()),
        ("gtja_alpha135".to_string(), alpha_135()),
        ("gtja_alpha136".to_string(), alpha_136()),
        ("gtja_alpha137".to_string(), alpha_137()),
        ("gtja_alpha138".to_string(), alpha_138()),
        ("gtja_alpha139".to_string(), alpha_139()),
        ("gtja_alpha140".to_string(), alpha_140()),
        ("gtja_alpha141".to_string(), alpha_141()),
        ("gtja_alpha142".to_string(), alpha_142()),
        ("gtja_alpha143".to_string(), alpha_143()),
        ("gtja_alpha144".to_string(), alpha_144()),
        ("gtja_alpha145".to_string(), alpha_145()),
        ("gtja_alpha146".to_string(), alpha_146()),
        ("gtja_alpha147".to_string(), alpha_147()),
        ("gtja_alpha148".to_string(), alpha_148()),
        ("gtja_alpha149".to_string(), alpha_149()),
        ("gtja_alpha150".to_string(), alpha_150()),
        ("gtja_alpha151".to_string(), alpha_151()),
        ("gtja_alpha152".to_string(), alpha_152()),
        ("gtja_alpha153".to_string(), alpha_153()),
        ("gtja_alpha154".to_string(), alpha_154()),
        ("gtja_alpha155".to_string(), alpha_155()),
        ("gtja_alpha156".to_string(), alpha_156()),
        ("gtja_alpha157".to_string(), alpha_157()),
        ("gtja_alpha158".to_string(), alpha_158()),
        ("gtja_alpha159".to_string(), alpha_159()),
        ("gtja_alpha160".to_string(), alpha_160()),
        ("gtja_alpha161".to_string(), alpha_161()),
        ("gtja_alpha162".to_string(), alpha_162()),
        ("gtja_alpha163".to_string(), alpha_163()),
        ("gtja_alpha164".to_string(), alpha_164()),
        ("gtja_alpha165".to_string(), alpha_165()),
        ("gtja_alpha166".to_string(), alpha_166()),
        ("gtja_alpha167".to_string(), alpha_167()),
        ("gtja_alpha168".to_string(), alpha_168()),
        ("gtja_alpha169".to_string(), alpha_169()),
        ("gtja_alpha170".to_string(), alpha_170()),
        ("gtja_alpha171".to_string(), alpha_171()),
        ("gtja_alpha172".to_string(), alpha_172()),
        ("gtja_alpha173".to_string(), alpha_173()),
        ("gtja_alpha174".to_string(), alpha_174()),
        ("gtja_alpha175".to_string(), alpha_175()),
        ("gtja_alpha176".to_string(), alpha_176()),
        ("gtja_alpha177".to_string(), alpha_177()),
        ("gtja_alpha178".to_string(), alpha_178()),
        ("gtja_alpha179".to_string(), alpha_179()),
        ("gtja_alpha180".to_string(), alpha_180()),
        ("gtja_alpha181".to_string(), alpha_181()),
        ("gtja_alpha182".to_string(), alpha_182()),
        ("gtja_alpha183".to_string(), alpha_183()),
        ("gtja_alpha184".to_string(), alpha_184()),
        ("gtja_alpha185".to_string(), alpha_185()),
        ("gtja_alpha186".to_string(), alpha_186()),
        ("gtja_alpha187".to_string(), alpha_187()),
        ("gtja_alpha188".to_string(), alpha_188()),
        ("gtja_alpha189".to_string(), alpha_189()),
        ("gtja_alpha190".to_string(), alpha_190()),
        ("gtja_alpha191".to_string(), alpha_191()),
    ]
}

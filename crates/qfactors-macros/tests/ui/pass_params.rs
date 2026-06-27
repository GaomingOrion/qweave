use qfactors_macros::factor;

#[factor(
    windows = [20, 60],
    params = [
        { name = "k15", k = 1.5 },
        { name = "k20", k = 2.0 },
    ]
)]
fn volume_breakout(volume: &[f64], k: f64) -> f64 {
    let last = volume[volume.len() - 1];
    let mean = volume.iter().sum::<f64>() / volume.len() as f64;
    if last > k * mean { 1.0 } else { 0.0 }
}

fn main() {}

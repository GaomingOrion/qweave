use qfactors_macros::factor;

#[factor(
    window = 20,
    params = [
        { name = "k15", threshold = 1.5 },
    ]
)]
fn bad(volume: &[f64], k: f64) -> f64 {
    volume[volume.len() - 1] * k
}

fn main() {}

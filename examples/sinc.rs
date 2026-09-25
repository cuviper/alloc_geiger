use std::f32::consts::PI;

const PEAK: f32 = 0.5;

const SAMPLE_HZ: u16 = 48_000;
const SINC_HZ: u16 = 4_000;
const SINC_SAMPLES: u16 = SAMPLE_HZ / SINC_HZ;
const SAMPLE_SCALE: f32 = 2.0 * PI / SINC_SAMPLES as f32;

fn main() {
    // Use multiple periods for a waning tail
    let samples = 4 * SINC_SAMPLES;

    let pos_pulse = (1..=samples).map(|i| {
        let x = f32::from(i) * SAMPLE_SCALE;
        x.sin() / x * PEAK
    });
    let neg_pulse = pos_pulse.clone().rev();

    let pulse = neg_pulse
        .chain(Some(PEAK))
        .chain(pos_pulse)
        .collect::<Vec<_>>();
    println!("{pulse:?}");
}

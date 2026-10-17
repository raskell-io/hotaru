//! Runs a culture headless and prints its activity, for tuning Params.
//!
//!     cargo run --release --example probe -- seed=1 seconds=20 w_exc=5 …

use hotaru_core::{Culture, Params};

const BIN_MS: u32 = 10;

fn main() {
    let (mut seed, mut seconds) = (1u64, 20u32);
    let mut p = Params::default();
    for arg in std::env::args().skip(1) {
        let (key, value) = arg.split_once('=').expect("key=value");
        let f: f32 = value.parse().expect("number");
        match key {
            "seed" => seed = f as u64,
            "seconds" => seconds = f as u32,
            "neurons" => p.neurons = f as usize,
            "synapses" => p.synapses = f as usize,
            "w_exc" => p.w_exc = f,
            "w_inh" => p.w_inh = f,
            "noise_rate" => p.noise_rate = f,
            "depression" => p.depression = f,
            "recovery_ms" => p.recovery_ms = f,
            "long_range" => p.long_range = f,
            "length_scale" => p.length_scale = f,
            _ => panic!("unknown parameter {key}"),
        }
    }
    let neurons = p.neurons;
    let mut c = Culture::new(seed, p);
    let started = std::time::Instant::now();
    let mut bins = Vec::new();
    let mut total = 0usize;
    for _ in 0..seconds * 1000 / BIN_MS {
        c.clear_output();
        c.run(BIN_MS);
        let spikes = c.spikes().len() / 2;
        total += spikes;
        // Population rate in Hz per neuron.
        bins.push(spikes as f32 / neurons as f32 / (BIN_MS as f32 / 1000.0));
    }
    let elapsed = started.elapsed().as_secs_f32();

    // A burst: the population rate crosses 20 Hz, counted once per crossing.
    let threshold = 20.0;
    let mut bursts = 0;
    let mut above = false;
    for &r in &bins {
        if r >= threshold && !above {
            bursts += 1;
        }
        above = r >= threshold;
    }
    let mut k = 0;
    while k < bins.len() {
        if bins[k] >= threshold {
            let s = k;
            while k < bins.len() && bins[k] >= 1.0 { k += 1; }
            let p = bins[s..k].iter().cloned().fold(0.0, f32::max);
            println!("burst at {:>6} ms, {:>3} ms long, peak {p:.0} Hz", s as u32 * BIN_MS, (k - s) as u32 * BIN_MS);
        }
        k += 1;
    }
    let peak = bins.iter().cloned().fold(0.0, f32::max);
    let mut sorted = bins.clone();
    sorted.sort_by(f32::total_cmp);

    println!(
        "seed {seed}, {neurons} neurons, {} synapses, {seconds} s in {elapsed:.2} s ({:.1}x real time)",
        c.synapse_count(),
        seconds as f32 / elapsed
    );
    println!(
        "mean rate {:.2} Hz, median bin {:.2} Hz, peak bin {peak:.1} Hz, {bursts} bursts ({:.1}/min)",
        total as f32 / neurons as f32 / seconds as f32,
        sorted[sorted.len() / 2],
        bursts as f32 * 60.0 / seconds as f32
    );
    // One character per 100 ms, the peak bin in each.
    let ramp = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
    let line: String = bins
        .chunks(100 / BIN_MS as usize)
        .map(|w| {
            let m = w.iter().cloned().fold(0.0, f32::max);
            ramp[((m / peak.max(1e-6)) * 9.0).round() as usize]
        })
        .collect();
    for (k, row) in line.as_bytes().chunks(100).enumerate() {
        println!("{:>3}s |{}|", k * 10, std::str::from_utf8(row).unwrap());
    }
}

//! Plays Pong with several cultures under each feedback mode and compares
//! rally lengths early and late in the session.
//!
//!     cargo run --release --example experiment -- seeds=8 minutes=20 neurons=2048

use std::thread;

use hotaru_core::Params;
use hotaru_core::session::{Feedback, Session};

const MODES: [Feedback; 3] = [Feedback::Contingent, Feedback::Random, Feedback::Silent];
/// Length of the early and late windows compared, minutes.
const WINDOW_MIN: u32 = 5;

struct Run {
    early: Summary,
    late: Summary,
    rate_hz: f32,
    weight: (f32, f32),
}

#[derive(Default)]
struct Summary {
    rallies: usize,
    mean: f32,
    aces: f32,
    long: f32,
}

fn summarize(rallies: &[u32], from_ms: u32, to_ms: u32) -> Summary {
    let hits: Vec<u32> = rallies
        .chunks(2)
        .filter(|r| r[0] >= from_ms && r[0] < to_ms)
        .map(|r| r[1])
        .collect();
    if hits.is_empty() {
        return Summary::default();
    }
    let n = hits.len() as f32;
    Summary {
        rallies: hits.len(),
        mean: hits.iter().sum::<u32>() as f32 / n,
        aces: hits.iter().filter(|&&h| h == 0).count() as f32 / n,
        long: hits.iter().filter(|&&h| h >= 3).count() as f32 / n,
    }
}

fn run(seed: u64, feedback: Feedback, minutes: u32, p: Params) -> Run {
    let n = p.neurons as f32;
    let mut s = Session::new(seed, p, Some(feedback));
    let w0 = s.culture().mean_excitatory_weight();
    let mut spikes = 0usize;
    for _ in 0..minutes * 60 {
        s.culture_mut().clear_output();
        s.run(1000);
        spikes += s.culture().spikes().len() / 2;
    }
    let end = minutes * 60_000;
    let rallies = s.game().unwrap().rallies();
    Run {
        early: summarize(rallies, 0, WINDOW_MIN * 60_000),
        late: summarize(rallies, end - WINDOW_MIN * 60_000, end),
        rate_hz: spikes as f32 / n / (minutes * 60) as f32,
        weight: (w0, s.culture().mean_excitatory_weight()),
    }
}

fn main() {
    let (mut seeds, mut minutes) = (6u64, 20u32);
    let mut p = Params { neurons: 2048, ..Params::default() };
    for arg in std::env::args().skip(1) {
        let (key, value) = arg.split_once('=').expect("key=value");
        let f: f32 = value.parse().expect("number");
        match key {
            "seeds" => seeds = f as u64,
            "minutes" => minutes = f as u32,
            "neurons" => p.neurons = f as usize,
            "stdp_gain" => p.stdp_gain = f,
            "stdp_loss" => p.stdp_loss = f,
            "w_max" => p.w_max = f,
            _ => panic!("unknown parameter {key}"),
        }
    }
    assert!(minutes >= 2 * WINDOW_MIN, "need at least {} minutes", 2 * WINDOW_MIN);

    let started = std::time::Instant::now();
    let handles: Vec<_> = (1..=seeds)
        .flat_map(|seed| MODES.map(|m| (seed, m)))
        .map(|(seed, mode)| {
            let p = p.clone();
            thread::spawn(move || (seed, mode, run(seed, mode, minutes, p)))
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    println!(
        "{seeds} cultures × {} modes, {} neurons, {minutes} min each, in {:.0} s",
        MODES.len(),
        p.neurons,
        started.elapsed().as_secs_f32()
    );
    println!("first {WINDOW_MIN} min vs last {WINDOW_MIN} min; mean hits per rally, aces, rallies of 3+ hits\n");
    for mode in MODES {
        println!("{mode:?}");
        let mut deltas = Vec::new();
        for (seed, _, r) in results.iter().filter(|(_, m, _)| *m == mode) {
            println!(
                "  seed {seed:>2}: {:.2} → {:.2} hits/rally | aces {:>3.0}% → {:>3.0}% | 3+ {:>3.0}% → {:>3.0}% | n {:>3} → {:>3} | {:.2} Hz | w {:.2} → {:.2}",
                r.early.mean,
                r.late.mean,
                100.0 * r.early.aces,
                100.0 * r.late.aces,
                100.0 * r.early.long,
                100.0 * r.late.long,
                r.early.rallies,
                r.late.rallies,
                r.rate_hz,
                r.weight.0,
                r.weight.1,
            );
            deltas.push(r.late.mean - r.early.mean);
        }
        let mean = deltas.iter().sum::<f32>() / deltas.len() as f32;
        let sd = (deltas.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / (deltas.len().max(2) - 1) as f32).sqrt();
        let improved = deltas.iter().filter(|&&d| d > 0.0).count();
        println!(
            "  change in hits/rally: {mean:+.2} ± {:.2} (s.e.), longer in {improved} of {}\n",
            sd / (deltas.len() as f32).sqrt(),
            deltas.len()
        );
    }
}

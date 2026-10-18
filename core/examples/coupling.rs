//! Does the ball's height reach the motor regions? Plays six cultures for
//! five minutes without feedback and prints how well the paddle drive
//! follows the ball (see `Game::coupling`).
//!
//!     cargo run --release --example coupling -- length_scale=150
use hotaru_core::Params;
use hotaru_core::session::{Feedback, Session};
fn main() {
    let mut p = Params { neurons: 2048, ..Params::default() };
    for arg in std::env::args().skip(1) {
        let (key, value) = arg.split_once('=').expect("key=value");
        let f: f32 = value.parse().expect("number");
        match key {
            "length_scale" => p.length_scale = f,
            "long_range" => p.long_range = f,
            "stim_current" => p.stim_current = f,
            "stim_radius" => p.stim_radius = f,
            "neurons" => p.neurons = f as usize,
            _ => panic!("unknown parameter {key}"),
        }
    }
    let mut all = Vec::new();
    for seed in 1..=6u64 {
        let mut s = Session::new(seed, p.clone(), Some(Feedback::Silent));
        s.run(300_000);
        let r = s.game().unwrap().coupling();
        all.push(r);
    }
    println!("corr(ball height, drive) per seed: {}", all.iter().map(|r| format!("{r:+.2}")).collect::<Vec<_>>().join(" "));
}

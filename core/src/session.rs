//! The closed loop of DishBrain (Kagan et al. 2022): the culture senses the
//! ball through stimulation, moves the paddle with its own activity, and is
//! told how it did through feedback.
//!
//! Electrode roles on the 8×8 array:
//!
//! ```text
//!   col  1 2 3 4 5 6 7 8
//!   row 1  U U . S S . U U     S  sensory: the row of the ball's height,
//!   ...    U U . S S . U U        pulsed faster as the ball comes closer
//!   row 4  U U . S S . U U     U  motor "up"
//!   row 5  D D . S S . D D     D  motor "down"
//!   ...    D D . S S . D D     .  not used
//!   row 8  D D . S S . D D
//! ```
//!
//! Feedback, as in the paper: a hit is followed by a predictable stimulus
//! (all sensory electrodes at 100 Hz for 100 ms), a miss by an unpredictable
//! one (random electrodes at random times, 5 Hz on average, for 4 s) while
//! the ball waits to be served again.

use crate::pong::{Event, Pong};
use crate::rng::Rng;
use crate::{Culture, Params};

const PREDICTABLE_MS: u32 = 100;
const PREDICTABLE_EVERY_MS: u32 = 10;
const UNPREDICTABLE_MS: u32 = 4000;
const UNPREDICTABLE_HZ: f32 = 5.0;
const UNPREDICTABLE_SITES: usize = 4;
/// Ball pause after a miss, whatever the feedback.
const SERVE_PAUSE_MS: u32 = 4000;
/// Sensory pulse rate: far wall to paddle.
const SENSE_HZ: (f32, f32) = (4.0, 40.0);
/// Motor readout smoothing and gain.
const MOTOR_TAU_MS: f32 = 50.0;
const MOTOR_FLOOR: f32 = 2.0;
const MOTOR_GAIN: f32 = 3.0;

/// What the culture is told after a hit or a miss.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feedback {
    /// Predictable after hits, unpredictable after misses: the DishBrain protocol.
    Contingent,
    /// The same two stimuli, chosen by a coin flip at every hit and miss.
    Random,
    /// No feedback at all.
    Silent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Unused,
    Sensory,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stimulus {
    None,
    Predictable { since: u32 },
    Unpredictable { since: u32 },
}

pub struct Session {
    culture: Culture,
    game: Option<Game>,
}

impl Session {
    /// A culture on its own (`feedback` None) or playing Pong. Plasticity is
    /// switched on for the game.
    pub fn new(seed: u64, mut params: Params, feedback: Option<Feedback>) -> Self {
        if feedback.is_some() {
            params.plasticity = true;
        }
        let culture = Culture::new(seed, params);
        let game = feedback.map(|f| Game::new(seed, f, &culture));
        Session { culture, game }
    }

    pub fn step(&mut self) {
        match &mut self.game {
            Some(game) => game.step(&mut self.culture),
            None => self.culture.step(),
        }
    }

    pub fn run(&mut self, ms: u32) {
        for _ in 0..ms {
            self.step();
        }
    }

    pub fn culture(&self) -> &Culture {
        &self.culture
    }

    pub fn culture_mut(&mut self) -> &mut Culture {
        &mut self.culture
    }

    pub fn game(&self) -> Option<&Game> {
        self.game.as_ref()
    }
}

pub struct Game {
    feedback: Feedback,
    pong: Pong,
    /// Serves and returns; seeded alike for every feedback mode.
    rng: Rng,
    /// Coin flips and unpredictable stimulation.
    noise: Rng,
    roles: Vec<Role>,
    /// The two sensory electrodes of each row, top to bottom.
    sensory_rows: Vec<Vec<usize>>,
    sensory: Vec<usize>,
    up: Vec<usize>,
    down: Vec<usize>,

    stimulus: Stimulus,
    serve_at: Option<u32>,
    sense_phase: f32,
    up_level: f32,
    down_level: f32,
    drive: f32,

    rally: u32,
    hits: u32,
    misses: u32,
    /// Finished rallies as (end time ms, hits) pairs.
    rallies: Vec<u32>,
}

impl Game {
    fn new(seed: u64, feedback: Feedback, culture: &Culture) -> Self {
        let mut rng = Rng::new(seed ^ 0x005E_ED0F_9A3E);
        let pong = Pong::new(&mut rng);
        let mut roles = Vec::new();
        let mut sensory_rows = vec![Vec::new(); 8];
        let (mut sensory, mut up, mut down) = (Vec::new(), Vec::new(), Vec::new());
        for (i, e) in culture.electrodes().iter().enumerate() {
            let role = match (e.col, e.row) {
                (4 | 5, _) => Role::Sensory,
                (1 | 2 | 7 | 8, 1..=4) => Role::Up,
                (1 | 2 | 7 | 8, _) => Role::Down,
                _ => Role::Unused,
            };
            match role {
                Role::Sensory => {
                    sensory_rows[e.row as usize - 1].push(i);
                    sensory.push(i);
                }
                Role::Up => up.push(i),
                Role::Down => down.push(i),
                Role::Unused => {}
            }
            roles.push(role);
        }
        Game {
            feedback,
            pong,
            rng,
            noise: Rng::new(seed ^ 0xF00D_CAFE),
            roles,
            sensory_rows,
            sensory,
            up,
            down,
            stimulus: Stimulus::None,
            serve_at: None,
            sense_phase: 0.0,
            up_level: 0.0,
            down_level: 0.0,
            drive: 0.0,
            rally: 0,
            hits: 0,
            misses: 0,
            rallies: Vec::new(),
        }
    }

    fn step(&mut self, culture: &mut Culture) {
        let t = culture.time();
        self.stimulate(culture, t);
        culture.step();
        self.read_motor(culture);

        if let Some(at) = self.serve_at {
            if t >= at {
                self.serve_at = None;
                self.pong.serve(&mut self.rng);
            }
            return;
        }
        match self.pong.step(self.drive, &mut self.rng) {
            Some(Event::Hit) => {
                self.hits += 1;
                self.rally += 1;
                self.give_feedback(true, t);
            }
            Some(Event::Miss) => {
                self.misses += 1;
                self.rallies.extend_from_slice(&[t, self.rally]);
                self.rally = 0;
                self.serve_at = Some(t + SERVE_PAUSE_MS);
                self.give_feedback(false, t);
            }
            None => {}
        }
    }

    fn give_feedback(&mut self, hit: bool, t: u32) {
        let predictable = match self.feedback {
            Feedback::Contingent => hit,
            Feedback::Random => self.noise.f32() < 0.5,
            Feedback::Silent => return,
        };
        self.stimulus = if predictable {
            Stimulus::Predictable { since: t }
        } else {
            Stimulus::Unpredictable { since: t }
        };
    }

    /// Pulses for this millisecond: feedback when it is running, otherwise
    /// the ball's position while it is in play.
    fn stimulate(&mut self, culture: &mut Culture, t: u32) {
        match self.stimulus {
            Stimulus::Predictable { since } if t - since < PREDICTABLE_MS => {
                if (t - since).is_multiple_of(PREDICTABLE_EVERY_MS) {
                    for &e in &self.sensory {
                        culture.stimulate(e);
                    }
                }
                return;
            }
            Stimulus::Unpredictable { since } if t - since < UNPREDICTABLE_MS => {
                if self.noise.f32() < UNPREDICTABLE_HZ / 1000.0 {
                    let count = culture.electrodes().len() as u32;
                    for _ in 0..UNPREDICTABLE_SITES {
                        culture.stimulate(self.noise.below(count) as usize);
                    }
                }
                return;
            }
            Stimulus::None => {}
            _ => self.stimulus = Stimulus::None,
        }
        if self.serve_at.is_some() {
            return;
        }
        let [x, y] = self.pong.ball;
        let near = (1.0 - x).clamp(0.0, 1.0);
        self.sense_phase += (SENSE_HZ.0 + (SENSE_HZ.1 - SENSE_HZ.0) * near) / 1000.0;
        if self.sense_phase >= 1.0 {
            self.sense_phase -= 1.0;
            let row = ((y * 8.0) as usize).min(7);
            for &e in &self.sensory_rows[row] {
                culture.stimulate(e);
            }
        }
    }

    fn read_motor(&mut self, culture: &Culture) {
        let counts = culture.step_counts();
        let sum = |set: &[usize]| set.iter().map(|&e| counts[e]).sum::<u32>() as f32;
        let keep = (-1.0 / MOTOR_TAU_MS).exp();
        self.up_level = self.up_level * keep + sum(&self.up);
        self.down_level = self.down_level * keep + sum(&self.down);
        let (u, d) = (self.up_level, self.down_level);
        self.drive = (MOTOR_GAIN * (d - u) / (u + d + MOTOR_FLOOR)).clamp(-1.0, 1.0);
    }

    pub fn feedback(&self) -> Feedback {
        self.feedback
    }
    pub fn pong(&self) -> &Pong {
        &self.pong
    }
    pub fn roles(&self) -> &[Role] {
        &self.roles
    }
    pub fn stimulus(&self) -> Stimulus {
        self.stimulus
    }
    /// True while the ball waits to be served after a miss.
    pub fn waiting(&self) -> bool {
        self.serve_at.is_some()
    }
    /// Paddle drive from the motor regions, -1 (up) to 1 (down).
    pub fn drive(&self) -> f32 {
        self.drive
    }
    pub fn hits(&self) -> u32 {
        self.hits
    }
    pub fn misses(&self) -> u32 {
        self.misses
    }
    /// Hits in the rally still being played.
    pub fn rally(&self) -> u32 {
        self.rally
    }
    /// Finished rallies as (end time ms, hits) pairs.
    pub fn rallies(&self) -> &[u32] {
        &self.rallies
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> Params {
        Params { neurons: 1024, ..Params::default() }
    }

    #[test]
    fn roles_cover_the_array() {
        let s = Session::new(1, small(), Some(Feedback::Contingent));
        let roles = s.game().unwrap().roles();
        let count = |r| roles.iter().filter(|&&x| x == r).count();
        assert_eq!(count(Role::Sensory), 16);
        assert_eq!(count(Role::Up), 14);
        assert_eq!(count(Role::Down), 14);
        assert_eq!(count(Role::Unused), 16);
    }

    #[test]
    fn plays_rallies_and_is_deterministic() {
        let mut a = Session::new(2, small(), Some(Feedback::Contingent));
        let mut b = Session::new(2, small(), Some(Feedback::Contingent));
        a.run(60_000);
        b.run(60_000);
        let (ga, gb) = (a.game().unwrap(), b.game().unwrap());
        assert!(ga.misses() >= 3, "{} misses in a minute", ga.misses());
        assert_eq!(ga.rallies(), gb.rallies());
        assert_eq!(a.culture().spikes(), b.culture().spikes());
    }

    #[test]
    fn feedback_modes_start_from_the_same_culture() {
        let a = Session::new(3, small(), Some(Feedback::Contingent));
        let b = Session::new(3, small(), Some(Feedback::Random));
        assert_eq!(a.culture().positions(), b.culture().positions());
        assert_eq!(a.game().unwrap().pong().ball, b.game().unwrap().pong().ball);
    }
}

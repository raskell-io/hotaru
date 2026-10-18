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

use crate::math;
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
const MOTOR_GAIN: f32 = 3.0;
/// Each motor region is read relative to its own mean over this long, so a
/// region that is simply more active does not pin the paddle to one wall.
const MOTOR_BASELINE_MS: f32 = 20_000.0;
/// Lowest baseline, and the relative activity that counts as no signal.
const MOTOR_FLOOR: f32 = 2.0;
const MOTOR_RELATIVE_FLOOR: f32 = 1.0;
/// Stimulation avoidance: the paddle counts as level with the target within
/// this distance, and as heading for it with at least this drive.
const AVOID_LEVEL: f32 = 0.05;
const AVOID_DRIVE: f32 = 0.2;
/// How fast the expected outcome follows the feedback, per event.
const EXPECTATION_RATE: f32 = 0.1;

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

/// Stimulation avoidance (Sinapayen, Masumori & Ikegami 2017): while the
/// ball is in play, the sensory pulses stop whenever the paddle moves toward
/// a target, so acting on the input is what makes it go quiet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Avoidance {
    Off,
    /// The target is the ball.
    Ball,
    /// The target is the ball mirrored top to bottom: the same rule with no
    /// relation to the game, as a control.
    Mirror,
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

    /// Switch stimulation avoidance on or off for the game.
    pub fn set_avoidance(&mut self, avoidance: Avoidance) {
        if let Some(game) = &mut self.game {
            game.avoidance = avoidance;
        }
    }
}

pub struct Game {
    feedback: Feedback,
    avoidance: Avoidance,
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
    up_baseline: f32,
    down_baseline: f32,
    drive: f32,
    /// Running mean of the feedback, 1 for predictable and 0 for
    /// unpredictable; the reward is the surprise relative to it.
    expected: f32,

    rally: u32,
    hits: u32,
    misses: u32,
    /// Finished rallies as (end time ms, hits) pairs.
    rallies: Vec<u32>,
    /// Sums for the correlation of ball height with paddle drive while the
    /// ball is in play: n, x, y, xx, yy, xy.
    coupling: [f64; 6],
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
            avoidance: Avoidance::Off,
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
            up_baseline: MOTOR_FLOOR,
            down_baseline: MOTOR_FLOOR,
            drive: 0.0,
            expected: 0.3,
            rally: 0,
            hits: 0,
            misses: 0,
            rallies: Vec::new(),
            coupling: [0.0; 6],
        }
    }

    fn step(&mut self, culture: &mut Culture) {
        let t = culture.time();
        self.stimulate(culture, t);
        culture.step();
        self.read_motor(culture);

        if self.serve_at.is_none() {
            let (x, y) = ((self.pong.ball[1] - 0.5) as f64, self.drive as f64);
            for (sum, v) in self.coupling.iter_mut().zip([1.0, x, y, x * x, y * y, x * y]) {
                *sum += v;
            }
        }
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
                self.give_feedback(true, t, culture);
            }
            Some(Event::Miss) => {
                self.misses += 1;
                self.rallies.extend_from_slice(&[t, self.rally]);
                self.rally = 0;
                self.serve_at = Some(t + SERVE_PAUSE_MS);
                self.give_feedback(false, t, culture);
            }
            None => {}
        }
    }

    /// Stimulate after a hit or miss, and reward the culture by how much
    /// better the feedback was than expected (only with eligibility traces).
    fn give_feedback(&mut self, hit: bool, t: u32, culture: &mut Culture) {
        let predictable = match self.feedback {
            Feedback::Contingent => hit,
            Feedback::Random => self.noise.f32() < 0.5,
            Feedback::Silent => return,
        };
        let outcome = if predictable { 1.0 } else { 0.0 };
        culture.reward(outcome - self.expected);
        self.expected += (outcome - self.expected) * EXPECTATION_RATE;
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
        if self.serve_at.is_some() || self.avoided() {
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

    /// True while the paddle heads for the avoidance target, or is level
    /// with it, so the sensory input stays quiet.
    fn avoided(&self) -> bool {
        let y = self.pong.ball[1];
        let target = match self.avoidance {
            Avoidance::Off => return false,
            Avoidance::Ball => y,
            Avoidance::Mirror => 1.0 - y,
        };
        let error = target - self.pong.paddle;
        error.abs() < AVOID_LEVEL || error * self.drive > AVOID_DRIVE * error.abs()
    }

    fn read_motor(&mut self, culture: &Culture) {
        let counts = culture.step_counts();
        let sum = |set: &[usize]| set.iter().map(|&e| counts[e]).sum::<u32>() as f32;
        let keep = math::exp(-1.0 / MOTOR_TAU_MS);
        self.up_level = self.up_level * keep + sum(&self.up);
        self.down_level = self.down_level * keep + sum(&self.down);
        let slow = 1.0 / MOTOR_BASELINE_MS;
        self.up_baseline += (self.up_level - self.up_baseline) * slow;
        self.down_baseline += (self.down_level - self.down_baseline) * slow;
        let u = self.up_level / self.up_baseline.max(MOTOR_FLOOR);
        let d = self.down_level / self.down_baseline.max(MOTOR_FLOOR);
        self.drive = (MOTOR_GAIN * (d - u) / (u + d + MOTOR_RELATIVE_FLOOR)).clamp(-1.0, 1.0);
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
    /// True while stimulation avoidance holds the sensory input back.
    pub fn quiet(&self) -> bool {
        self.serve_at.is_none() && self.stimulus == Stimulus::None && self.avoided()
    }
    /// Correlation of the ball's height with the paddle drive so far, -1..1:
    /// how much the ball's position reaches the motor regions. Positive means
    /// the paddle tends to move toward the ball. NaN before there is data.
    pub fn coupling(&self) -> f32 {
        let [n, x, y, xx, yy, xy] = self.coupling;
        let cov = xy / n - x / n * (y / n);
        let sd = ((xx / n - (x / n).powi(2)) * (yy / n - (y / n).powi(2))).sqrt();
        (cov / sd) as f32
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

    #[test]
    fn avoidance_quiets_the_input_only_when_on() {
        let quiet_ms = |avoidance| {
            let mut s = Session::new(4, small(), Some(Feedback::Contingent));
            s.set_avoidance(avoidance);
            (0..20_000).filter(|_| {
                s.step();
                s.game().unwrap().quiet()
            }).count()
        };
        assert_eq!(quiet_ms(Avoidance::Off), 0);
        let (ball, mirror) = (quiet_ms(Avoidance::Ball), quiet_ms(Avoidance::Mirror));
        assert!(ball > 1000 && mirror > 1000, "quiet for {ball} and {mirror} ms");
        assert_ne!(ball, mirror);
    }
}

//! Single-paddle Pong, as in DishBrain: the paddle guards the left edge of a
//! unit square, the ball bounces off the other three walls.

use crate::rng::Rng;

pub const PADDLE_HEIGHT: f32 = 0.25;
pub const BALL_RADIUS: f32 = 0.02;
/// Ball speed in field units per second.
pub const BALL_SPEED: f32 = 0.5;
/// Fastest the paddle can move, field units per second.
pub const PADDLE_SPEED: f32 = 1.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Hit,
    Miss,
}

#[derive(Clone, Debug)]
pub struct Pong {
    /// Ball centre; x = 0 is the paddle side, y = 0 the top.
    pub ball: [f32; 2],
    pub velocity: [f32; 2],
    /// Paddle centre on the y axis.
    pub paddle: f32,
}

impl Pong {
    pub fn new(rng: &mut Rng) -> Self {
        let mut pong = Pong { ball: [0.0; 2], velocity: [0.0; 2], paddle: 0.5 };
        pong.serve(rng);
        pong
    }

    /// Put the ball on the far wall, heading for the paddle side.
    pub fn serve(&mut self, rng: &mut Rng) {
        let angle = (15.0 + 35.0 * rng.f32()).to_radians();
        let down = if rng.f32() < 0.5 { 1.0 } else { -1.0 };
        self.ball = [1.0 - BALL_RADIUS, 0.2 + 0.6 * rng.f32()];
        self.velocity = [-BALL_SPEED * angle.cos(), down * BALL_SPEED * angle.sin()];
    }

    /// Advance one millisecond. `drive` in -1..1 moves the paddle up (negative)
    /// or down (positive).
    pub fn step(&mut self, drive: f32, rng: &mut Rng) -> Option<Event> {
        let dt = 0.001;
        let half = PADDLE_HEIGHT / 2.0;
        self.paddle = (self.paddle + drive.clamp(-1.0, 1.0) * PADDLE_SPEED * dt).clamp(half, 1.0 - half);

        let [mut x, mut y] = self.ball;
        x += self.velocity[0] * dt;
        y += self.velocity[1] * dt;
        if y < BALL_RADIUS {
            y = 2.0 * BALL_RADIUS - y;
            self.velocity[1] = self.velocity[1].abs();
        } else if y > 1.0 - BALL_RADIUS {
            y = 2.0 * (1.0 - BALL_RADIUS) - y;
            self.velocity[1] = -self.velocity[1].abs();
        }
        if x > 1.0 - BALL_RADIUS {
            x = 2.0 * (1.0 - BALL_RADIUS) - x;
            self.velocity[0] = -self.velocity[0].abs();
        }
        self.ball = [x, y];

        if x <= BALL_RADIUS && self.velocity[0] < 0.0 {
            if (y - self.paddle).abs() <= half + BALL_RADIUS {
                // Return it at a slightly new angle, so rallies do not loop.
                let angle = (15.0 + 35.0 * rng.f32()).to_radians();
                let down = if self.velocity[1] >= 0.0 { 1.0 } else { -1.0 };
                self.velocity = [BALL_SPEED * angle.cos(), down * BALL_SPEED * angle.sin()];
                self.ball[0] = BALL_RADIUS;
                return Some(Event::Hit);
            }
            return Some(Event::Miss);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_still_paddle_misses_most_serves() {
        let mut rng = Rng::new(9);
        let mut pong = Pong::new(&mut rng);
        let (mut hits, mut misses) = (0, 0);
        for _ in 0..600_000 {
            match pong.step(0.0, &mut rng) {
                Some(Event::Hit) => hits += 1,
                Some(Event::Miss) => {
                    misses += 1;
                    pong.serve(&mut rng);
                }
                None => {}
            }
        }
        assert!(misses > 50, "{misses} misses");
        assert!(hits < misses, "{hits} hits, {misses} misses");
    }

    #[test]
    fn a_tracking_paddle_never_misses() {
        let mut rng = Rng::new(4);
        let mut pong = Pong::new(&mut rng);
        for _ in 0..300_000 {
            let drive = (pong.ball[1] - pong.paddle) * 20.0;
            assert_ne!(pong.step(drive, &mut rng), Some(Event::Miss));
        }
    }
}

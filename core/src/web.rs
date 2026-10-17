//! The C ABI that web/hotaru.js calls. Sessions are addressed by handle.
//! Pointers are offsets into this module's memory; JS rebuilds its views
//! after each call that may allocate.

use std::cell::UnsafeCell;

use crate::Params;
use crate::session::{Feedback, Role, Session, Stimulus};

struct Slot {
    session: Session,
    /// Scratch for hotaru_game, see there.
    game: [f32; 10],
}

struct Slots(UnsafeCell<Vec<Option<Slot>>>);
// SAFETY: wasm32-unknown-unknown is single-threaded.
unsafe impl Sync for Slots {}

static SLOTS: Slots = Slots(UnsafeCell::new(Vec::new()));

fn slots() -> &'static mut Vec<Option<Slot>> {
    // SAFETY: single-threaded; no reference outlives an exported call.
    unsafe { &mut *SLOTS.0.get() }
}

fn slot(id: u32) -> &'static mut Slot {
    slots().get_mut(id as usize).and_then(Option::as_mut).expect("unknown session")
}

fn session(id: u32) -> &'static mut Session {
    &mut slot(id).session
}

/// `mode`: 0 a culture on its own, 1 Pong with DishBrain feedback, 2 Pong
/// with random feedback, 3 Pong without feedback. Returns the handle.
#[unsafe(no_mangle)]
pub extern "C" fn hotaru_create(seed_hi: u32, seed_lo: u32, neurons: u32, mode: u32) -> u32 {
    let seed = (seed_hi as u64) << 32 | seed_lo as u64;
    let params = Params { neurons: neurons.clamp(64, 16384) as usize, ..Params::default() };
    let feedback = match mode {
        1 => Some(Feedback::Contingent),
        2 => Some(Feedback::Random),
        3 => Some(Feedback::Silent),
        _ => None,
    };
    let fresh = Slot { session: Session::new(seed, params, feedback), game: [0.0; 10] };
    let all = slots();
    match all.iter().position(Option::is_none) {
        Some(free) => {
            all[free] = Some(fresh);
            free as u32
        }
        None => {
            all.push(Some(fresh));
            all.len() as u32 - 1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_free(id: u32) {
    if let Some(s) = slots().get_mut(id as usize) {
        *s = None;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_neurons(id: u32) -> u32 {
    session(id).culture().params().neurons as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_synapses(id: u32) -> u32 {
    session(id).culture().synapse_count() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_radius(id: u32) -> f32 {
    session(id).culture().params().radius
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_positions(id: u32) -> *const f32 {
    session(id).culture().positions().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_kinds(id: u32) -> *const u8 {
    session(id).culture().kinds().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_electrodes(id: u32) -> u32 {
    session(id).culture().electrodes().len() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_electrode_positions(id: u32) -> *const f32 {
    session(id).culture().electrode_positions().as_ptr()
}

/// Packed column * 10 + row, e.g. 45 for column 4, row 5.
#[unsafe(no_mangle)]
pub extern "C" fn hotaru_electrode_name(id: u32, e: u32) -> u32 {
    let c = session(id).culture();
    c.electrodes().get(e as usize).map_or(0, |e| e.col as u32 * 10 + e.row as u32)
}

/// 0 unused, 1 sensory, 2 motor up, 3 motor down; 0 without a game.
#[unsafe(no_mangle)]
pub extern "C" fn hotaru_electrode_role(id: u32, e: u32) -> u32 {
    let Some(game) = session(id).game() else { return 0 };
    match game.roles().get(e as usize) {
        Some(Role::Sensory) => 1,
        Some(Role::Up) => 2,
        Some(Role::Down) => 3,
        _ => 0,
    }
}

/// Clears the previous output, then advances `ms` milliseconds.
#[unsafe(no_mangle)]
pub extern "C" fn hotaru_step(id: u32, ms: u32) {
    let s = session(id);
    s.culture_mut().clear_output();
    s.run(ms);
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_time(id: u32) -> u32 {
    session(id).culture().time()
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_stimulate(id: u32, e: u32) {
    session(id).culture_mut().stimulate(e as usize);
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_spikes(id: u32) -> *const u32 {
    session(id).culture().spikes().as_ptr()
}

/// Number of (time, neuron) pairs behind hotaru_spikes.
#[unsafe(no_mangle)]
pub extern "C" fn hotaru_spike_count(id: u32) -> u32 {
    session(id).culture().spikes().len() as u32 / 2
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_counts(id: u32) -> *const u32 {
    session(id).culture().counts().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_glow(id: u32, tau_ms: f32) -> *const f32 {
    session(id).culture_mut().update_glow(tau_ms).as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_weight(id: u32) -> f32 {
    session(id).culture().mean_excitatory_weight()
}

/// Game state as ten floats: ball x, ball y, paddle y, paddle drive,
/// stimulus (0 none, 1 predictable, 2 unpredictable), ms since the stimulus
/// began, waiting for a serve (0 or 1), hits in this rally, hits, misses.
/// Null without a game.
#[unsafe(no_mangle)]
pub extern "C" fn hotaru_game(id: u32) -> *const f32 {
    let slot = slot(id);
    let Some(game) = slot.session.game() else { return std::ptr::null() };
    let now = slot.session.culture().time();
    let (kind, since) = match game.stimulus() {
        Stimulus::None => (0.0, now),
        Stimulus::Predictable { since } => (1.0, since),
        Stimulus::Unpredictable { since } => (2.0, since),
    };
    let pong = game.pong();
    slot.game = [
        pong.ball[0],
        pong.ball[1],
        pong.paddle,
        game.drive(),
        kind,
        now.saturating_sub(since) as f32,
        game.waiting() as u8 as f32,
        game.rally() as f32,
        game.hits() as f32,
        game.misses() as f32,
    ];
    slot.game.as_ptr()
}

/// Finished rallies as (end time ms, hits) pairs.
#[unsafe(no_mangle)]
pub extern "C" fn hotaru_rallies(id: u32) -> *const u32 {
    session(id).game().map_or(std::ptr::null(), |g| g.rallies().as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_rally_count(id: u32) -> u32 {
    session(id).game().map_or(0, |g| g.rallies().len() as u32 / 2)
}

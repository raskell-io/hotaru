//! The C ABI that web/hotaru.js calls. Pointers are offsets into this
//! module's memory; JS rebuilds its views after each call that may allocate.

use std::cell::UnsafeCell;

use crate::{Culture, Params};

struct Slot(UnsafeCell<Option<Culture>>);
// SAFETY: wasm32-unknown-unknown is single-threaded.
unsafe impl Sync for Slot {}

static CULTURE: Slot = Slot(UnsafeCell::new(None));

fn culture() -> &'static mut Culture {
    // SAFETY: single-threaded; no reference outlives an exported call.
    unsafe { (*CULTURE.0.get()).as_mut().expect("hotaru_new not called") }
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_new(seed_hi: u32, seed_lo: u32, neurons: u32) {
    let seed = (seed_hi as u64) << 32 | seed_lo as u64;
    let params = Params { neurons: neurons.clamp(64, 16384) as usize, ..Params::default() };
    let fresh = Culture::new(seed, params);
    // SAFETY: as in culture().
    unsafe { *CULTURE.0.get() = Some(fresh) };
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_neurons() -> u32 {
    culture().params().neurons as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_synapses() -> u32 {
    culture().synapse_count() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_radius() -> f32 {
    culture().params().radius
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_positions() -> *const f32 {
    culture().positions().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_kinds() -> *const u8 {
    culture().kinds().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_electrodes() -> u32 {
    culture().electrodes().len() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_electrode_positions() -> *const f32 {
    culture().electrode_positions().as_ptr()
}

/// Packed column * 10 + row, e.g. 45 for column 4, row 5.
#[unsafe(no_mangle)]
pub extern "C" fn hotaru_electrode_name(e: u32) -> u32 {
    culture().electrodes().get(e as usize).map_or(0, |e| e.col as u32 * 10 + e.row as u32)
}

/// Clears the previous output, then advances `ms` milliseconds.
#[unsafe(no_mangle)]
pub extern "C" fn hotaru_step(ms: u32) {
    let c = culture();
    c.clear_output();
    c.run(ms);
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_time() -> u32 {
    culture().time()
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_stimulate(e: u32) {
    culture().stimulate(e as usize);
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_spikes() -> *const u32 {
    culture().spikes().as_ptr()
}

/// Number of (time, neuron) pairs behind hotaru_spikes.
#[unsafe(no_mangle)]
pub extern "C" fn hotaru_spike_count() -> u32 {
    culture().spikes().len() as u32 / 2
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_counts() -> *const u32 {
    culture().counts().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn hotaru_glow(tau_ms: f32) -> *const f32 {
    culture().update_glow(tau_ms).as_ptr()
}

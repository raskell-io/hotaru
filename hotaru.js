// Thin wrapper around hotaru.wasm (core/src/web.rs). Each culture is a
// session in the module, addressed by handle.

export const MODES = { culture: 0, feedback: 1, random: 2, silent: 3 };

export async function loadHotaru(url = 'hotaru.wasm') {
  let instance;
  try {
    ({ instance } = await WebAssembly.instantiateStreaming(fetch(url), {}));
  } catch {
    // Servers that send .wasm without the application/wasm type.
    const bytes = await (await fetch(url)).arrayBuffer();
    ({ instance } = await WebAssembly.instantiate(bytes, {}));
  }
  return {
    /** `mode` is one of MODES; anything but `culture` plays Pong. */
    create: (seed, neurons, mode = 'culture') => new Culture(instance.exports, seed, neurons, mode),
  };
}

const ROLES = ['unused', 'sensory', 'up', 'down'];

class Culture {
  constructor(x, seed, neurons, mode) {
    this.x = x;
    this.id = x.hotaru_create(0, seed >>> 0, neurons, MODES[mode] ?? 0);
    this.mode = mode;
    this.seed = seed >>> 0;
    this.n = x.hotaru_neurons(this.id);
    this.synapses = x.hotaru_synapses(this.id);
    this.radius = x.hotaru_radius(this.id);
    // Static data is copied out once; later calls may move the memory.
    this.positions = this.#f32(x.hotaru_positions(this.id), 2 * this.n).slice();
    this.kinds = new Uint8Array(x.memory.buffer, x.hotaru_kinds(this.id), this.n).slice();
    this.electrodeCount = x.hotaru_electrodes(this.id);
    this.electrodes = this.#f32(x.hotaru_electrode_positions(this.id), 2 * this.electrodeCount).slice();
    this.electrodeNames = Array.from({ length: this.electrodeCount }, (_, e) => {
      const p = x.hotaru_electrode_name(this.id, e);
      return { col: Math.floor(p / 10), row: p % 10 };
    });
    this.roles = Array.from({ length: this.electrodeCount }, (_, e) => ROLES[x.hotaru_electrode_role(this.id, e)]);
  }

  free() { this.x.hotaru_free(this.id); }

  get time() { return this.x.hotaru_time(this.id); }

  /** Advance `ms` milliseconds; spikes() and counts() then cover just those. */
  step(ms) { this.x.hotaru_step(this.id, ms); }

  stimulate(electrode) { this.x.hotaru_stimulate(this.id, electrode); }

  /** (time ms, neuron) pairs from the last step() call. */
  spikes() {
    return new Uint32Array(this.x.memory.buffer, this.x.hotaru_spikes(this.id), 2 * this.x.hotaru_spike_count(this.id));
  }

  /** Spikes heard by each electrode during the last step() call. */
  counts() {
    return new Uint32Array(this.x.memory.buffer, this.x.hotaru_counts(this.id), this.electrodeCount);
  }

  /** Per-neuron brightness: 1 at a spike, decaying with `tauMs`. */
  glow(tauMs) { return this.#f32(this.x.hotaru_glow(this.id, tauMs), this.n); }

  /** Mean weight of excitatory synapses; it drifts with plasticity. */
  weight() { return this.x.hotaru_weight(this.id); }

  /** The Pong game, or null for a culture on its own. */
  game() {
    const ptr = this.x.hotaru_game(this.id);
    if (!ptr) return null;
    const g = this.#f32(ptr, 10);
    return {
      ball: [g[0], g[1]],
      paddle: g[2],
      drive: g[3],
      stimulus: ['none', 'predictable', 'unpredictable'][g[4]],
      stimulusAge: g[5],
      waiting: g[6] > 0,
      rally: g[7],
      hits: g[8],
      misses: g[9],
    };
  }

  /** Finished rallies as (end time ms, hits) pairs. */
  rallies() {
    const count = this.x.hotaru_rally_count(this.id);
    return count ? new Uint32Array(this.x.memory.buffer, this.x.hotaru_rallies(this.id), 2 * count) : new Uint32Array(0);
  }

  #f32(ptr, len) { return new Float32Array(this.x.memory.buffer, ptr, len); }
}

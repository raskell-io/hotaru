// Thin wrapper around hotaru.wasm (core/src/web.rs). One culture lives in
// the module at a time; creating a new one replaces it.

export async function loadHotaru(url = 'hotaru.wasm') {
  let instance;
  try {
    ({ instance } = await WebAssembly.instantiateStreaming(fetch(url), {}));
  } catch {
    // Servers that send .wasm without the application/wasm type.
    const bytes = await (await fetch(url)).arrayBuffer();
    ({ instance } = await WebAssembly.instantiate(bytes, {}));
  }
  return { create: (seed, neurons) => new Culture(instance.exports, seed, neurons) };
}

class Culture {
  constructor(x, seed, neurons) {
    this.x = x;
    x.hotaru_new(0, seed >>> 0, neurons);
    this.seed = seed >>> 0;
    this.n = x.hotaru_neurons();
    this.synapses = x.hotaru_synapses();
    this.radius = x.hotaru_radius();
    // Static data is copied out once; later calls may move the memory.
    this.positions = this.#f32(x.hotaru_positions(), 2 * this.n).slice();
    this.kinds = new Uint8Array(x.memory.buffer, x.hotaru_kinds(), this.n).slice();
    this.electrodeCount = x.hotaru_electrodes();
    this.electrodes = this.#f32(x.hotaru_electrode_positions(), 2 * this.electrodeCount).slice();
    this.electrodeNames = Array.from({ length: this.electrodeCount }, (_, e) => {
      const p = x.hotaru_electrode_name(e);
      return { col: Math.floor(p / 10), row: p % 10 };
    });
  }

  get time() { return this.x.hotaru_time(); }

  /** Advance `ms` milliseconds; spikes() and counts() then cover just those. */
  step(ms) { this.x.hotaru_step(ms); }

  stimulate(electrode) { this.x.hotaru_stimulate(electrode); }

  /** (time ms, neuron) pairs from the last step() call. */
  spikes() {
    return new Uint32Array(this.x.memory.buffer, this.x.hotaru_spikes(), 2 * this.x.hotaru_spike_count());
  }

  /** Spikes heard by each electrode during the last step() call. */
  counts() {
    return new Uint32Array(this.x.memory.buffer, this.x.hotaru_counts(), this.electrodeCount);
  }

  /** Per-neuron brightness: 1 at a spike, decaying with `tauMs`. */
  glow(tauMs) { return this.#f32(this.x.hotaru_glow(tauMs), this.n); }

  #f32(ptr, len) { return new Float32Array(this.x.memory.buffer, ptr, len); }
}

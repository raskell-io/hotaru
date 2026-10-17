<p align="center"><img src="web/favicon.svg" alt="" width="72"></p>

<h1 align="center">Hotaru <sub>蛍</sub></h1>

<p align="center"><b>A neuron culture on a multi-electrode array, simulated in your browser.</b></p>

<p align="center">
  <a href="https://hotaru.raskell.io"><b>Open the dish</b></a> ·
  <a href="#the-model">The model</a> ·
  <a href="#roadmap">Roadmap</a>
</p>

Thousands of spiking neurons grow at random positions in a 2 mm dish on top
of a 60-electrode array. Left alone they fire sparsely, then burst together
every few seconds in waves that sweep across the dish, the rhythm real
cultures show on multi-electrode arrays. You can watch every spike, read what
each electrode hears, and stimulate the culture through any electrode.

Hotaru is the first step towards a simulation of the DishBrain protocol
(Kagan et al. 2022), in which a culture is wired to a game of Pong and gets
predictable feedback for hits and noise for misses.

## The model

| | |
|---|---|
| Neurons | Izhikevich (2003): 80% excitatory (regular spiking, chattering), 20% fast-spiking inhibitory |
| Dish | 1 024–8 192 neurons, uniform in a disc of radius 1 000 µm |
| Synapses | 100 per neuron, mostly within ~150 µm (exponential), 5% anywhere in the dish |
| Delays | 1 ms + distance at 150 µm/ms, up to 31 ms |
| Plasticity | short-term depression (Tsodyks & Markram 1997), recovery 5 s; this ends each burst |
| Input | spontaneous kicks at 0.1 Hz per neuron |
| Array | 8×8 at 200 µm pitch without the corners (60 electrodes), recording radius 50 µm, stimulation radius 100 µm |
| Time | fixed 1 ms steps, deterministic for a given seed |

A culture is defined by its seed, so `hotaru.raskell.io/#seed=1&n=4096` grows
the same dish for everyone.

## Layout

- `core/`: the simulation, in Rust. `cargo test` checks determinism, wiring
  and that cultures burst. With `--features web` it exports a small C ABI
  (`core/src/web.rs`) for the page.
- `core/examples/probe.rs`: runs a culture headless and prints its bursts, for
  tuning (`make probe ARGS="seed=3 w_exc=5.5"`).
- `web/`: the page. `hotaru.js` wraps the wasm module; `app.js` draws the dish
  (WebGL2), the electrode heatmap, the raster and the population rate.

## Running it

```sh
make serve   # builds web/hotaru.wasm and serves http://localhost:8646/
make test
```

Needs Rust 1.93+ with the `wasm32-unknown-unknown` target; `wasm-opt` is used
when installed.

## Roadmap

- **v0.1, the culture** (this release): dish, spikes, network bursts,
  electrode heatmap, stimulation.
- **v0.2, the closed loop**: spike-timing-dependent plasticity, Pong through
  sensory and motor electrodes, DishBrain-style feedback, and a control
  culture with random feedback beside it.
- **v0.3, experiments**: plasticity on/off, lesions, excitation/inhibition
  balance, shareable runs, and a write-up of stimulation avoidance
  (Sinapayen, Masumori & Ikegami 2017).
- **Later**: [WASM-AD](https://github.com/raskell-io/wasm-ad) carts as
  environments for the culture.

Hotaru is a simplified model for exploration. It makes no claims about real
brains, or about whether real cultures learn.

## License

MIT

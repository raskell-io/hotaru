<p align="center"><img src="web/favicon.svg" alt="" width="72"></p>

<h1 align="center">Hotaru <sub>蛍</sub></h1>

<p align="center"><b>A neuron culture on a multi-electrode array, simulated in your browser. Can it learn Pong?</b></p>

<p align="center">
  <a href="https://hotaru.raskell.io"><b>Run the experiment</b></a> ·
  <a href="https://hotaru.raskell.io/culture.html">Explore a culture</a> ·
  <a href="#results-so-far">Results so far</a> ·
  <a href="#roadmap">Roadmap</a>
</p>

Thousands of spiking neurons grow at random positions in a 2 mm dish on top
of a 60-electrode array. Left alone they fire sparsely, then burst together
every few seconds in waves that sweep across the dish, the rhythm real
cultures show on multi-electrode arrays.

Hotaru wires two identical cultures to Pong, following the DishBrain protocol
(Kagan et al. 2022). The culture senses the ball through stimulation, moves
the paddle with its own activity, and gets feedback: one culture gets a
predictable pulse after a hit and noise after a miss, the control gets the
same pulses at random (or no feedback at all). The page plays both side by
side and plots their rallies over time.

## The closed loop

```text
  col  1 2 3 4 5 6 7 8
  row 1  U U . S S . U U     S  sensory: the row of the ball's height,
  ...    U U . S S . U U        pulsed at 4 Hz (far) to 40 Hz (at the paddle)
  row 4  U U . S S . U U     U  motor up: more spikes move the paddle up
  row 5  D D . S S . D D     D  motor down
  ...    D D . S S . D D     .  not used
  row 8  D D . S S . D D
```

| Event | DishBrain feedback | Random control | Silent control |
|---|---|---|---|
| Hit | all sensory electrodes, 100 Hz for 100 ms | either stimulus, coin flip | nothing |
| Miss | random electrodes at random times, 5 Hz for 4 s | either stimulus, coin flip | nothing |

After a miss the ball waits 4 s before the next serve, in every condition.
The paddle follows the smoothed difference between the down and up regions.
Synapses from excitatory neurons are plastic while the game runs (STDP).

## The model

| | |
|---|---|
| Neurons | Izhikevich (2003): 80% excitatory (regular spiking, chattering), 20% fast-spiking inhibitory |
| Dish | 1 024–8 192 neurons, uniform in a disc of radius 1 000 µm |
| Synapses | 100 per neuron, mostly within ~150 µm (exponential), 5% anywhere in the dish |
| Delays | 1 ms + distance at 150 µm/ms, up to 31 ms |
| Short-term plasticity | synaptic depression (Tsodyks & Markram 1997), recovery 5 s; this ends each burst |
| Long-term plasticity | pair-based STDP on excitatory synapses, τ 20 ms, bounded at twice the initial mean weight; on while playing Pong |
| Input | spontaneous kicks at 0.1 Hz per neuron |
| Array | 8×8 at 200 µm pitch without the corners (60 electrodes), recording radius 50 µm, stimulation radius 100 µm |
| Time | fixed 1 ms steps, deterministic for a given seed |

A culture is defined by its seed, so `hotaru.raskell.io/#seed=1&n=4096` grows
the same dish for everyone.

## Results so far

Is a culture that gets DishBrain feedback better at Pong after 20 minutes
than at the start, and better than a control? We ran 16 culture pairs of
2 048 neurons for 20 simulated minutes per condition, and compared mean hits
per rally in the first five minutes with the last five:

| Condition | Change in hits per rally | Longer rallies at the end |
|---|---|---|
| DishBrain feedback | +0.05 ± 0.05 | 11 of 16 |
| Random feedback | −0.02 ± 0.04 | 8 of 16 |
| No feedback | +0.00 ± 0.05 | 7 of 16 |

(± is the standard error over cultures.) That is no reliable learning. With
4 096 neurons (−0.01, −0.02, +0.01) or with three times stronger STDP
(+0.05, −0.03, +0.06) the picture was the same. A first run with only six
cultures had suggested an effect (+0.17 ± 0.04, 6 of 6) that did not survive
more cultures, which is a reminder to use enough of them.

A likely reason: STDP links spikes about 20 ms apart, while the feedback
arrives seconds after the motor activity that earned it. Bridging that gap
is the question for the next version. Reproduce the table with:

```sh
make experiment ARGS="seeds=16 minutes=20 neurons=2048"
```

## Layout

- `core/`: the simulation, in Rust. `lib.rs` is the culture, `pong.rs` the
  game, `session.rs` the closed loop and its feedback. `cargo test` checks
  determinism, wiring, bursting and the game. With `--features web` it exports
  a small C ABI (`core/src/web.rs`) with one handle per culture.
- `core/examples/experiment.rs`: plays many cultures under each condition, in
  parallel, and prints the comparison above.
- `core/examples/probe.rs`: runs a culture headless and prints its bursts, for
  tuning (`make probe ARGS="seed=3 w_exc=5.5"`).
- `web/`: the pages. `index.html` + `app.js` is the Pong experiment,
  `culture.html` + `culture.js` the culture explorer with raster and
  electrode heatmap. `hotaru.js` wraps the wasm module, `dish.js` draws a
  culture with WebGL2.

## Running it

```sh
make serve   # builds web/hotaru.wasm and serves http://localhost:8646/
make test
```

Needs Rust 1.93+ with the `wasm32-unknown-unknown` target; `wasm-opt` is used
when installed.

## Roadmap

- **v0.1, the culture**: dish, spikes, network bursts, electrode heatmap,
  stimulation.
- **v0.2, the closed loop** (this release): STDP, Pong through sensory and
  motor electrodes, DishBrain feedback, a random or silent control beside it,
  and a headless experiment runner.
- **v0.3, experiments**: what bridges seconds of delay (eligibility traces,
  stimulation that stops when the culture acts, as in Sinapayen, Masumori &
  Ikegami 2017), plasticity on/off, lesions, and excitation/inhibition
  balance.
- **Later**: [WASM-AD](https://github.com/raskell-io/wasm-ad) carts as
  environments for the culture.

Hotaru is a simplified model of the DishBrain protocol. It makes no claims
about real brains, or about whether real cultures learn.

## License

MIT

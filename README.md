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
the paddle with its own activity, and gets feedback: a predictable pulse
after a hit and noise after a miss. The two cultures differ in one condition,
and the page plays them side by side and plots their rallies over time:

- **Quiet toward the ball** (the default): the ball input pauses while the
  paddle heads for the ball, against a control that pauses for a mirror image
  of the ball. This is the condition under which the cultures learn.
- **DishBrain feedback**, against the same pulses at random, or no feedback.

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
The paddle follows the difference between the down and up regions, each read
relative to its own mean over the last 20 s. Synapses from excitatory neurons
are plastic while the game runs (STDP).

With stimulation avoidance (Sinapayen, Masumori & Ikegami 2017), the sensory
pulses stop while the paddle is level with the ball or moving toward it, so
acting on the input is what silences it. The mirrored control applies the
same rule to the ball's mirror image (height 1 − y).

## The model

| | |
|---|---|
| Neurons | Izhikevich (2003): 80% excitatory (regular spiking, chattering), 20% fast-spiking inhibitory |
| Dish | 1 024–8 192 neurons, uniform in a disc of radius 1 000 µm |
| Synapses | 100 per neuron, at exponentially distributed distances (mean 500 µm), 5% anywhere in the dish |
| Delays | 1 ms + distance at 150 µm/ms |
| Short-term plasticity | synaptic depression (Tsodyks & Markram 1997), recovery 5 s; this ends each burst |
| Long-term plasticity | pair-based STDP on excitatory synapses, τ 20 ms, bounded at twice the initial mean weight; on while playing Pong. Optionally reward-modulated through eligibility traces |
| Input | spontaneous kicks at 0.1 Hz per neuron |
| Array | 8×8 at 200 µm pitch without the corners (60 electrodes), recording radius 50 µm, stimulation radius 100 µm |
| Time | fixed 1 ms steps, deterministic for a given seed; transcendental functions come from `libm`, so the browser and native runs are bit-identical |

A culture is defined by its seed, so `hotaru.raskell.io/#seed=1&n=4096` grows
the same dish for everyone, and plays the same game as `make experiment`.

## Results so far

Each condition ran on 16 cultures for 20 simulated minutes; the tables give
the change in mean hits per rally from the first five minutes to the last
five (± standard error over cultures), and in how many cultures rallies got
longer.

**DishBrain feedback alone does not teach.**

| Condition | 2 048 neurons | 4 096 neurons |
|---|---|---|
| DishBrain feedback | +0.15 ± 0.05 (11 of 16) | −0.00 ± 0.05 (6 of 16) |
| Random feedback | +0.07 ± 0.04 (13 of 16) | +0.09 ± 0.07 (9 of 16) |
| No feedback | +0.04 ± 0.06 (8 of 16) | −0.00 ± 0.07 (6 of 16) |

Turning STDP into reward-modulated plasticity (eligibility traces of 200 ms
to 1 s, a reward of hit or miss against the recent hit rate) did not change
that: +0.06 ± 0.06 with feedback against −0.04 random at the best setting.

**Why: the ball did not reach the paddle.** The sensory electrodes sit 400 to
600 µm from the motor ones. With connections mostly within 150 µm, as in v0.2,
the correlation of the ball's height with the paddle drive was about zero
(−0.04 to +0.04), so there was nothing to learn. With a mean connection length
of 500 µm it is about +0.1 at 2 048 neurons and +0.2 at 4 096 (`make
coupling`). The page shows it live as "Ball → paddle".

**Stimulation avoidance teaches.**

| Condition | 2 048 neurons | 4 096 neurons |
|---|---|---|
| Quiet toward the ball | +0.32 ± 0.10 (13 of 16) | +0.77 ± 0.25 (11 of 16) |
| Quiet toward the mirrored ball | +0.01 ± 0.06 (8 of 16) | −0.03 ± 0.07 (6 of 16) |
| No avoidance | +0.05 ± 0.06 (8 of 16) | +0.06 ± 0.05 (7 of 16) |

At 2 048 neurons the learning cultures end at 0.96 hits per rally against
0.58 and 0.52; at 4 096 at 1.65 against 0.55. With STDP switched off nothing
changes (+0.03, +0.01, +0.02), and with the short v0.2 wiring neither
(+0.03, −0.03, −0.05): the culture learns through plasticity, and only when
the ball's position reaches the motor regions. Single cultures vary, so one
pair on the page may not show it; the effect is in the average.

Reproduce with:

```sh
make experiment ARGS="seeds=16 minutes=20"              # feedback
make experiment ARGS="seeds=16 minutes=20 avoidance=1"  # avoidance
make experiment ARGS="seeds=16 avoidance=1 stdp_gain=0 stdp_loss=0"
make experiment ARGS="seeds=16 eligibility_ms=500 reward_gain=50"
```

Earlier findings: in v0.2 a first run with only six cultures had suggested
that DishBrain feedback helps (+0.17 ± 0.04, 6 of 6); it did not survive more
cultures, which is a reminder to use enough of them.

## Layout

- `core/`: the simulation, in Rust. `lib.rs` is the culture, `pong.rs` the
  game, `session.rs` the closed loop and its feedback. `cargo test` checks
  determinism, wiring, bursting and the game. With `--features web` it exports
  a small C ABI (`core/src/web.rs`) with one handle per culture.
- `core/examples/experiment.rs`: plays many cultures under each condition, in
  parallel, and prints the comparisons above.
- `core/examples/coupling.rs`: how much the ball's position reaches the motor
  regions (`make coupling ARGS="length_scale=150"`).
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
- **v0.2, the closed loop**: STDP, Pong through sensory and motor
  electrodes, DishBrain feedback, a random or silent control beside it, and a
  headless experiment runner.
- **v0.3, what makes it learn** (this release): a balanced motor readout,
  longer wiring so the ball reaches the motor regions, eligibility traces
  (no effect), and stimulation avoidance with a mirrored control (learns).
- **v0.4, more experiments**: lesions, excitation/inhibition balance, how
  learning scales with culture size, and whether what is learned persists
  when the condition is switched off.
- **Later**: [WASM-AD](https://github.com/raskell-io/wasm-ad) carts as
  environments for the culture.

Hotaru is a simplified model of the DishBrain protocol. It makes no claims
about real brains, or about whether real cultures learn.

## License

MIT

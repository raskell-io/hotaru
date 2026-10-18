//! Hotaru: a simulated culture of spiking neurons on a multi-electrode array.
//!
//! Neurons follow Izhikevich (2003): regular-spiking and chattering
//! excitatory cells, fast-spiking inhibitory cells. They sit at random
//! positions in a 2D dish and connect mostly to their neighbours, with
//! conduction delays proportional to distance. Excitatory synapses depress
//! with use (Tsodyks & Markram 1997), which is what turns recurrent
//! excitation into the burst-then-silence rhythm of real cultures.
//!
//! The simulation advances in fixed 1 ms steps and is fully deterministic
//! for a given seed and [`Params`].

pub mod math;
pub mod mea;
pub mod pong;
pub mod rng;
pub mod session;
#[cfg(feature = "web")]
mod web;

use mea::Electrode;
use rng::Rng;

/// Ring buffer length for delayed synaptic input, in ms. Delays are 1..DELAYS.
const DELAYS: usize = 32;
/// Upper bound on spikes kept between two calls to [`Culture::clear_output`].
pub const SPIKE_CAP: usize = 1 << 17;
const NONE: u32 = u32::MAX;

#[derive(Clone, Debug)]
pub struct Params {
    pub neurons: usize,
    /// Outgoing synapses per neuron.
    pub synapses: usize,
    /// Dish radius in µm.
    pub radius: f32,
    /// Mean length of local connections, µm.
    pub length_scale: f32,
    /// Fraction of connections drawn uniformly across the dish.
    pub long_range: f32,
    pub inhibitory: f32,
    /// Mean excitatory and inhibitory synaptic weights (input current).
    pub w_exc: f32,
    pub w_inh: f32,
    /// Axonal conduction velocity, µm per ms.
    pub velocity: f32,
    /// Spontaneous input: each neuron receives a kick at this rate (Hz).
    pub noise_rate: f32,
    pub noise_current: f32,
    /// Fraction of synaptic resources one spike uses, and their recovery time.
    pub depression: f32,
    pub recovery_ms: f32,
    /// An electrode hears neurons within this radius and stimulates within
    /// `stim_radius`, µm.
    pub record_radius: f32,
    pub stim_radius: f32,
    pub stim_current: f32,
    /// Spike-timing-dependent plasticity on synapses from excitatory
    /// neurons: pre before post strengthens, post before pre weakens.
    pub plasticity: bool,
    pub stdp_tau: f32,
    pub stdp_gain: f32,
    pub stdp_loss: f32,
    /// Upper bound for a plastic weight, as a multiple of `w_exc`.
    pub w_max: f32,
    /// With a positive value, STDP does not change weights directly but tags
    /// synapses with an eligibility trace that fades over this many ms;
    /// `Culture::reward` then turns the tags into weight changes. Zero
    /// applies STDP directly.
    pub eligibility_ms: f32,
    /// Weight change per unit of reward and eligibility.
    pub reward_gain: f32,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            neurons: 4096,
            synapses: 100,
            radius: 1000.0,
            length_scale: 500.0,
            long_range: 0.05,
            inhibitory: 0.2,
            w_exc: 6.0,
            w_inh: -6.0,
            velocity: 150.0,
            noise_rate: 0.1,
            noise_current: 20.0,
            depression: 0.2,
            recovery_ms: 5000.0,
            record_radius: 50.0,
            stim_radius: 100.0,
            stim_current: 30.0,
            plasticity: false,
            stdp_tau: 20.0,
            stdp_gain: 0.10,
            stdp_loss: 0.12,
            w_max: 2.0,
            eligibility_ms: 0.0,
            reward_gain: 1.0,
        }
    }
}

pub struct Culture {
    params: Params,
    time: u32,
    rng: Rng,
    noise_threshold: u32,

    /// Neuron positions in µm, interleaved x, y.
    positions: Vec<f32>,
    /// 1 for inhibitory neurons, 0 for excitatory.
    kinds: Vec<u8>,
    a: Vec<f32>,
    b: Vec<f32>,
    c: Vec<f32>,
    d: Vec<f32>,
    v: Vec<f32>,
    u: Vec<f32>,
    /// Available synaptic resources of each neuron's outgoing synapses, 0..1.
    resources: Vec<f32>,
    last_spike: Vec<u32>,

    // Synapses in CSR form, grouped by presynaptic neuron.
    syn_start: Vec<u32>,
    syn_target: Vec<u32>,
    syn_delay: Vec<u8>,
    syn_weight: Vec<f32>,
    /// Synapse indices grouped by postsynaptic neuron, for plasticity.
    in_start: Vec<u32>,
    in_synapse: Vec<u32>,
    /// Presynaptic neuron of each entry in `in_synapse`.
    in_pre: Vec<u32>,
    /// Pending input current, DELAYS slots of one value per neuron.
    ring: Vec<f32>,
    fired: Vec<u32>,
    /// STDP traces: recent spiking as presynaptic and postsynaptic partner.
    pre_trace: Vec<f32>,
    post_trace: Vec<f32>,
    /// Eligibility of each synapse and when it was last brought up to date;
    /// empty unless `eligibility_ms` is positive.
    eligibility: Vec<f32>,
    eligibility_at: Vec<u32>,

    electrodes: Vec<Electrode>,
    electrode_positions: Vec<f32>,
    /// The electrode each neuron is recorded by, or NONE.
    recorded_by: Vec<u32>,
    stim_start: Vec<u32>,
    stim_targets: Vec<u32>,
    pending_stim: Vec<u32>,

    // Output since the last clear_output.
    spikes: Vec<u32>,
    dropped: u32,
    counts: Vec<u32>,
    /// Spikes each electrode heard during the most recent step.
    step_counts: Vec<u32>,
    glow: Vec<f32>,
}

impl Culture {
    pub fn new(seed: u64, params: Params) -> Self {
        let mut rng = Rng::new(seed);
        let n = params.neurons;
        let r = params.radius;

        let mut positions = Vec::with_capacity(2 * n);
        for _ in 0..n {
            // Uniform in the disc.
            let rho = r * rng.f32().sqrt();
            let theta = std::f32::consts::TAU * rng.f32();
            positions.push(rho * math::cos(theta));
            positions.push(rho * math::sin(theta));
        }

        let (mut kinds, mut a, mut b, mut c, mut d) =
            (vec![0u8; n], vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        for i in 0..n {
            let x = rng.f32();
            if rng.f32() < params.inhibitory {
                kinds[i] = 1;
                a[i] = 0.02 + 0.08 * x;
                b[i] = 0.25 - 0.05 * x;
                c[i] = -65.0;
                d[i] = 2.0;
            } else {
                // x² skews towards regular spiking, with some chattering cells.
                a[i] = 0.02;
                b[i] = 0.2;
                c[i] = -65.0 + 15.0 * x * x;
                d[i] = 8.0 - 6.0 * x * x;
            }
        }
        let v = vec![-65.0; n];
        let u = (0..n).map(|i| b[i] * -65.0).collect();

        let grid = Grid::new(&positions, r, params.length_scale / 3.0);
        let k = params.synapses;
        let mut syn_start = Vec::with_capacity(n + 1);
        let mut syn_target = Vec::with_capacity(n * k);
        let mut syn_delay = Vec::with_capacity(n * k);
        let mut syn_weight = Vec::with_capacity(n * k);
        for i in 0..n {
            syn_start.push(syn_target.len() as u32);
            let (px, py) = (positions[2 * i], positions[2 * i + 1]);
            for _ in 0..k {
                let j = if rng.f32() < params.long_range {
                    loop {
                        let j = rng.below(n as u32) as usize;
                        if j != i {
                            break j;
                        }
                    }
                } else {
                    grid.sample_near(&mut rng, px, py, params.length_scale, i)
                };
                let dist = math::hypot(positions[2 * j] - px, positions[2 * j + 1] - py);
                let delay = (1.0 + dist / params.velocity).round() as usize;
                syn_target.push(j as u32);
                syn_delay.push(delay.clamp(1, DELAYS - 1) as u8);
                syn_weight.push(if kinds[i] == 1 {
                    params.w_inh * (0.75 + 0.5 * rng.f32())
                } else {
                    params.w_exc * (0.75 + 0.5 * rng.f32())
                });
            }
        }
        syn_start.push(syn_target.len() as u32);

        let mut in_start = vec![0u32; n + 1];
        for &j in &syn_target {
            in_start[j as usize + 1] += 1;
        }
        for k in 1..=n {
            in_start[k] += in_start[k - 1];
        }
        let mut fill = in_start.clone();
        let mut in_synapse = vec![0u32; syn_target.len()];
        let mut in_pre = vec![0u32; syn_target.len()];
        for i in 0..n {
            let span = syn_start[i] as usize..syn_start[i + 1] as usize;
            for (k, &j) in span.clone().zip(&syn_target[span]) {
                let j = j as usize;
                in_synapse[fill[j] as usize] = k as u32;
                in_pre[fill[j] as usize] = i as u32;
                fill[j] += 1;
            }
        }

        let electrodes = mea::layout();
        let electrode_positions = electrodes.iter().flat_map(|e| [e.x, e.y]).collect();
        let mut recorded_by = vec![NONE; n];
        let mut stim_start = Vec::with_capacity(electrodes.len() + 1);
        let mut stim_targets = Vec::new();
        for (ei, e) in electrodes.iter().enumerate() {
            stim_start.push(stim_targets.len() as u32);
            for i in 0..n {
                let dist = math::hypot(positions[2 * i] - e.x, positions[2 * i + 1] - e.y);
                if dist <= params.record_radius {
                    recorded_by[i] = ei as u32;
                }
                if dist <= params.stim_radius {
                    stim_targets.push(i as u32);
                }
            }
        }
        stim_start.push(stim_targets.len() as u32);

        // Staggered resources, below the burst threshold: a culture that starts
        // fully rested fires one oversized burst.
        let resources = (0..n).map(|_| 0.3 + 0.3 * rng.f32()).collect();
        let noise_p = (params.noise_rate / 1000.0).clamp(0.0, 1.0);
        let tags = if params.eligibility_ms > 0.0 { syn_weight.len() } else { 0 };
        Culture {
            noise_threshold: (noise_p as f64 * u32::MAX as f64) as u32,
            time: 0,
            rng,
            positions,
            kinds,
            a,
            b,
            c,
            d,
            v,
            u,
            resources,
            last_spike: vec![NONE; n],
            syn_start,
            syn_target,
            syn_delay,
            syn_weight,
            in_start,
            in_synapse,
            in_pre,
            ring: vec![0.0; DELAYS * n],
            fired: Vec::with_capacity(n),
            pre_trace: vec![0.0; n],
            post_trace: vec![0.0; n],
            eligibility: vec![0.0; tags],
            eligibility_at: vec![0; tags],
            counts: vec![0; electrodes.len()],
            step_counts: vec![0; electrodes.len()],
            electrodes,
            electrode_positions,
            recorded_by,
            stim_start,
            stim_targets,
            pending_stim: Vec::new(),
            spikes: Vec::with_capacity(2 * SPIKE_CAP),
            dropped: 0,
            glow: vec![0.0; n],
            params,
        }
    }

    /// Advance one millisecond.
    pub fn step(&mut self) {
        let n = self.params.neurons;
        let t = self.time;
        let slot = (t as usize % DELAYS) * n;

        for e in self.pending_stim.drain(..) {
            let (s, end) = (self.stim_start[e as usize], self.stim_start[e as usize + 1]);
            for &i in &self.stim_targets[s as usize..end as usize] {
                self.ring[slot + i as usize] += self.params.stim_current;
            }
        }

        let recover = 1.0 / self.params.recovery_ms;
        self.fired.clear();
        for i in 0..n {
            let mut input = std::mem::take(&mut self.ring[slot + i]);
            if self.rng.next_u32() < self.noise_threshold {
                input += self.params.noise_current;
            }
            let (mut v, mut u) = (self.v[i], self.u[i]);
            // Two half steps for v, as in Izhikevich (2003), for stability.
            for _ in 0..2 {
                v += 0.5 * (0.04 * v * v + 5.0 * v + 140.0 - u + input);
            }
            u += self.a[i] * (self.b[i] * v - u);
            if v >= 30.0 {
                v = self.c[i];
                u += self.d[i];
                self.fired.push(i as u32);
            }
            self.v[i] = v;
            self.u[i] = u;
            self.resources[i] += (1.0 - self.resources[i]) * recover;
        }

        self.step_counts.iter_mut().for_each(|c| *c = 0);
        if self.params.plasticity {
            self.plasticity();
        }

        for &i in &self.fired {
            let i = i as usize;
            let efficacy = if self.kinds[i] == 0 {
                let r = self.resources[i];
                self.resources[i] -= self.params.depression * r;
                r
            } else {
                1.0
            };
            let (s, end) = (self.syn_start[i] as usize, self.syn_start[i + 1] as usize);
            for k in s..end {
                let at = ((t as usize + self.syn_delay[k] as usize) % DELAYS) * n;
                self.ring[at + self.syn_target[k] as usize] += self.syn_weight[k] * efficacy;
            }

            self.last_spike[i] = t;
            if self.spikes.len() < 2 * SPIKE_CAP {
                self.spikes.extend_from_slice(&[t, i as u32]);
            } else {
                self.dropped += 1;
            }
            if let Some(e) = self.electrode_of(i) {
                self.counts[e] += 1;
                self.step_counts[e] += 1;
            }
        }
        self.time = t + 1;
    }

    fn electrode_of(&self, neuron: usize) -> Option<usize> {
        let e = self.recorded_by[neuron];
        (e != NONE).then_some(e as usize)
    }

    /// Pair-based STDP with exponential traces, applied to the neurons that
    /// fired this step. Delays are ignored; most are one or two ms.
    fn plasticity(&mut self) {
        let p = &self.params;
        let t = self.time;
        let decay = math::exp(-1.0 / p.stdp_tau);
        let w_max = p.w_max * p.w_exc;
        let tagging = p.eligibility_ms > 0.0;
        for (pre, post) in self.pre_trace.iter_mut().zip(self.post_trace.iter_mut()) {
            *pre *= decay;
            *post *= decay;
        }
        for &f in &self.fired {
            let f = f as usize;
            // f fired after its excitatory inputs: strengthen them.
            let (s, e) = (self.in_start[f] as usize, self.in_start[f + 1] as usize);
            for (&k, &pre) in self.in_synapse[s..e].iter().zip(&self.in_pre[s..e]) {
                let (k, pre) = (k as usize, pre as usize);
                if self.kinds[pre] == 0 {
                    let dw = p.stdp_gain * self.pre_trace[pre];
                    if tagging {
                        tag(&mut self.eligibility, &mut self.eligibility_at, k, dw, t, p.eligibility_ms);
                    } else {
                        self.syn_weight[k] = (self.syn_weight[k] + dw).min(w_max);
                    }
                }
            }
            // f fired after its targets: weaken its outgoing synapses.
            if self.kinds[f] == 0 {
                let (s, e) = (self.syn_start[f] as usize, self.syn_start[f + 1] as usize);
                for k in s..e {
                    let dw = -p.stdp_loss * self.post_trace[self.syn_target[k] as usize];
                    if tagging {
                        tag(&mut self.eligibility, &mut self.eligibility_at, k, dw, t, p.eligibility_ms);
                    } else {
                        self.syn_weight[k] = (self.syn_weight[k] + dw).max(0.0);
                    }
                }
            }
        }
        for &f in &self.fired {
            self.pre_trace[f as usize] += 1.0;
            self.post_trace[f as usize] += 1.0;
        }
    }

    /// Turn the eligibility of every excitatory synapse into a weight change:
    /// tagged synapses grow with positive `reward` and shrink with negative.
    /// Does nothing unless `eligibility_ms` is positive.
    pub fn reward(&mut self, reward: f32) {
        let p = &self.params;
        if p.eligibility_ms <= 0.0 {
            return;
        }
        let w_max = p.w_max * p.w_exc;
        for i in 0..p.neurons {
            if self.kinds[i] != 0 {
                continue;
            }
            for k in self.syn_start[i] as usize..self.syn_start[i + 1] as usize {
                tag(&mut self.eligibility, &mut self.eligibility_at, k, 0.0, self.time, p.eligibility_ms);
                let w = self.syn_weight[k] + p.reward_gain * reward * self.eligibility[k];
                self.syn_weight[k] = w.clamp(0.0, w_max);
            }
        }
    }

    /// Mean weight of the synapses from excitatory neurons.
    pub fn mean_excitatory_weight(&self) -> f32 {
        let (mut sum, mut count) = (0.0f64, 0usize);
        for i in 0..self.params.neurons {
            if self.kinds[i] == 0 {
                let (s, e) = (self.syn_start[i] as usize, self.syn_start[i + 1] as usize);
                sum += self.syn_weight[s..e].iter().map(|&w| w as f64).sum::<f64>();
                count += e - s;
            }
        }
        (sum / count.max(1) as f64) as f32
    }

    /// Spikes heard by each electrode during the most recent step.
    pub fn step_counts(&self) -> &[u32] {
        &self.step_counts
    }

    pub fn run(&mut self, ms: u32) {
        for _ in 0..ms {
            self.step();
        }
    }

    /// Deliver a stimulation pulse through an electrode on the next step.
    pub fn stimulate(&mut self, electrode: usize) {
        if electrode < self.electrodes.len() {
            self.pending_stim.push(electrode as u32);
        }
    }

    /// Forget the spikes and electrode counts gathered so far.
    pub fn clear_output(&mut self) {
        self.spikes.clear();
        self.dropped = 0;
        self.counts.iter_mut().for_each(|c| *c = 0);
    }

    /// Brightness of each neuron, 1 at a spike and decaying with `tau_ms`.
    pub fn update_glow(&mut self, tau_ms: f32) -> &[f32] {
        let k = -1.0 / tau_ms.max(1e-3);
        for (g, &last) in self.glow.iter_mut().zip(&self.last_spike) {
            *g = if last == NONE {
                0.0
            } else {
                // last_spike is set during step t and time is then t + 1.
                math::exp((self.time - 1 - last) as f32 * k)
            };
        }
        &self.glow
    }

    pub fn params(&self) -> &Params {
        &self.params
    }
    pub fn time(&self) -> u32 {
        self.time
    }
    pub fn positions(&self) -> &[f32] {
        &self.positions
    }
    pub fn kinds(&self) -> &[u8] {
        &self.kinds
    }
    pub fn synapse_count(&self) -> usize {
        self.syn_target.len()
    }
    pub fn electrodes(&self) -> &[Electrode] {
        &self.electrodes
    }
    pub fn electrode_positions(&self) -> &[f32] {
        &self.electrode_positions
    }
    /// Spikes since the last clear, as (time ms, neuron) pairs.
    pub fn spikes(&self) -> &[u32] {
        &self.spikes
    }
    pub fn dropped(&self) -> u32 {
        self.dropped
    }
    /// Spikes recorded by each electrode since the last clear.
    pub fn counts(&self) -> &[u32] {
        &self.counts
    }
}

/// Uniform buckets over the dish, for drawing a neighbour at a given distance.
struct Grid {
    cell: f32,
    side: usize,
    origin: f32,
    start: Vec<u32>,
    members: Vec<u32>,
}

impl Grid {
    fn new(positions: &[f32], radius: f32, cell: f32) -> Self {
        let side = (2.0 * radius / cell).ceil() as usize + 1;
        let n = positions.len() / 2;
        let index = |i: usize| {
            let cx = ((positions[2 * i] + radius) / cell) as usize;
            let cy = ((positions[2 * i + 1] + radius) / cell) as usize;
            cy.min(side - 1) * side + cx.min(side - 1)
        };
        let mut start = vec![0u32; side * side + 1];
        for i in 0..n {
            start[index(i) + 1] += 1;
        }
        for k in 1..start.len() {
            start[k] += start[k - 1];
        }
        let mut fill = start.clone();
        let mut members = vec![0u32; n];
        for i in 0..n {
            let c = index(i);
            members[fill[c] as usize] = i as u32;
            fill[c] += 1;
        }
        Grid { cell, side, origin: -radius, start, members }
    }

    /// A neuron at an exponentially distributed distance from (x, y),
    /// never `exclude`.
    fn sample_near(&self, rng: &mut Rng, x: f32, y: f32, scale: f32, exclude: usize) -> usize {
        loop {
            let dist = -scale * math::ln(1.0 - rng.f32());
            let theta = std::f32::consts::TAU * rng.f32();
            let cx = ((x + dist * math::cos(theta) - self.origin) / self.cell).floor();
            let cy = ((y + dist * math::sin(theta) - self.origin) / self.cell).floor();
            if cx < 0.0 || cy < 0.0 || cx >= self.side as f32 || cy >= self.side as f32 {
                continue;
            }
            let c = cy as usize * self.side + cx as usize;
            let (s, e) = (self.start[c], self.start[c + 1]);
            if s == e {
                continue;
            }
            let j = self.members[(s + rng.below(e - s)) as usize] as usize;
            if j != exclude {
                return j;
            }
        }
    }
}

/// Decay synapse `k`'s eligibility to time `t`, then add `dw`.
fn tag(eligibility: &mut [f32], at: &mut [u32], k: usize, dw: f32, t: u32, tau_ms: f32) {
    let age = t - at[k];
    if age > 0 {
        eligibility[k] *= math::exp(-(age as f32) / tau_ms);
        at[k] = t;
    }
    eligibility[k] += dw;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> Params {
        Params { neurons: 1024, ..Params::default() }
    }

    #[test]
    fn same_seed_same_culture() {
        let mut a = Culture::new(7, small());
        let mut b = Culture::new(7, small());
        a.run(2000);
        b.run(2000);
        assert!(!a.spikes().is_empty());
        assert_eq!(a.spikes(), b.spikes());
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Culture::new(1, small());
        let mut b = Culture::new(2, small());
        a.run(1000);
        b.run(1000);
        assert_ne!(a.spikes(), b.spikes());
    }

    #[test]
    fn bursts_between_quiet_stretches() {
        let n = 4096;
        let mut c = Culture::new(11, Params { neurons: n, ..Params::default() });
        let (mut bursts, mut above, mut total) = (0, false, 0);
        for _ in 0..4000 {
            c.clear_output();
            c.run(10);
            let spikes = c.spikes().len() / 2;
            total += spikes;
            // 20 Hz per neuron over a 10 ms bin.
            let burst = spikes * 5 >= n;
            bursts += (burst && !above) as u32;
            above = burst;
        }
        let rate = total as f32 / n as f32 / 40.0;
        assert!((3..=60).contains(&bursts), "{bursts} bursts in 40 s");
        assert!((0.3..5.0).contains(&rate), "mean rate {rate} Hz");
    }

    #[test]
    fn synapses_and_delays_in_range() {
        let c = Culture::new(3, small());
        assert_eq!(c.synapse_count(), 1024 * 100);
        assert!(c.syn_delay.iter().all(|&d| (1..DELAYS as u8).contains(&d)));
        assert!(c.syn_target.iter().all(|&j| (j as usize) < 1024));
        for i in 0..1024 {
            let (s, e) = (c.syn_start[i] as usize, c.syn_start[i + 1] as usize);
            assert!(!c.syn_target[s..e].contains(&(i as u32)), "self-synapse at {i}");
        }
    }

    #[test]
    fn stimulation_drives_spikes_near_the_electrode() {
        let mut c = Culture::new(5, Params { noise_rate: 0.0, ..small() });
        c.run(50);
        assert!(c.spikes().is_empty(), "silent without noise or input");
        c.stimulate(27);
        c.run(5);
        let e = c.electrodes()[27];
        let near = c.spikes().chunks(2).any(|s| {
            let i = s[1] as usize;
            (c.positions[2 * i] - e.x).hypot(c.positions[2 * i + 1] - e.y) <= 100.0
        });
        assert!(near);
    }

    #[test]
    fn eligibility_waits_for_reward() {
        let p = Params { plasticity: true, eligibility_ms: 1000.0, ..small() };
        let mut c = Culture::new(6, p);
        let w0 = c.syn_weight.clone();
        c.run(3000);
        assert_eq!(c.syn_weight, w0, "tags alone change no weight");
        c.reward(1.0);
        assert!(c.syn_weight != w0);

        // Punishment moves every synapse the other way.
        let mut d = Culture::new(6, Params { plasticity: true, eligibility_ms: 1000.0, ..small() });
        d.run(3000);
        d.reward(-1.0);
        let opposite = (0..w0.len()).all(|k| (c.syn_weight[k] - w0[k]) * (d.syn_weight[k] - w0[k]) <= 0.0);
        assert!(opposite);
    }

    #[test]
    fn reward_without_eligibility_does_nothing() {
        let mut c = Culture::new(7, Params { plasticity: true, ..small() });
        c.run(500);
        let w = c.syn_weight.clone();
        c.reward(1.0);
        assert_eq!(c.syn_weight, w);
    }
}

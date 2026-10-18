import { loadHotaru } from './hotaru.js';
import { createDish, FIREFLY, ICE, CHAMPAGNE, MARGIN } from './dish.js';

const GLOW_TAU = 45;          // ms of simulated time a flash takes to fade
const FRAME_BUDGET = 12;      // wall ms per frame both simulations may use
const ROLLING_MS = 120_000;   // window of the hits-per-rally chart
const RECENT_MS = 300_000;
const SIZES = [1024, 2048, 4096];
/** Each experiment pairs a culture under test (first) with a control. */
const EXPERIMENTS = {
  avoid: {
    modes: ['avoid', 'mirror'],
    titles: ['Quiet toward the ball', 'Control: mirrored ball'],
    notes: [
      'The ball input goes quiet while the paddle heads for the ball. DishBrain feedback, too.',
      'The same rule for the ball mirrored top to bottom, so quiet says nothing about the game.',
    ],
    legend: ['Quiet toward the ball', 'Control, mirrored ball'],
    short: ['for the ball culture', 'for the mirrored control'],
  },
  random: {
    modes: ['feedback', 'random'],
    titles: ['DishBrain feedback', 'Control'],
    notes: ['Predictable pulse after a hit, noise after a miss.', 'Same pulses, at random.'],
    legend: ['DishBrain feedback', 'Control, random feedback'],
    short: ['with feedback', 'for the control'],
  },
  silent: {
    modes: ['feedback', 'silent'],
    titles: ['DishBrain feedback', 'Control'],
    notes: ['Predictable pulse after a hit, noise after a miss.', 'No feedback at all.'],
    legend: ['DishBrain feedback', 'Control, no feedback'],
    short: ['with feedback', 'for the control'],
  },
};
const COMPARE_AFTER = 10;     // rallies each before the verdict compares them

const UP = [190, 225, 120];
const DOWN = [120, 190, 225];

const $ = (id) => document.getElementById(id);
const dpr = () => Math.min(window.devicePixelRatio || 1, 2);
const rgba = (c, a) => `rgba(${c[0]},${c[1]},${c[2]},${a})`;

const hotaru = await loadHotaru();

// --- one culture playing --------------------------------------------------------

class Player {
  constructor(root) {
    this.root = root;
    this.q = (role) => root.querySelector(`[data-role="${role}"]`);
    this.dish = createDish(this.q('neurons'));
    this.overlay = this.q('overlay');
    this.octx = this.overlay.getContext('2d');
    this.pong = this.q('pong');
    this.pctx = this.pong.getContext('2d');
    this.stat = (name) => root.querySelector(`[data-stat="${name}"]`);
  }

  start(seed, n, mode) {
    this.culture?.free();
    this.culture = hotaru.create(seed, n, mode);
    this.dish.upload(this.culture);
    this.startWeight = this.culture.weight();
    this.game = this.culture.game();
    this.flash = null;
    this.lastHits = 0;
    this.lastMisses = 0;
    this.stat('weight').textContent = this.startWeight.toFixed(2);
  }

  step(ms) {
    this.culture.step(ms);
    const g = this.culture.game();
    if (g.hits > this.lastHits) this.flash = { kind: 'hit', at: performance.now() };
    else if (g.misses > this.lastMisses) this.flash = { kind: 'miss', at: performance.now() };
    this.lastHits = g.hits;
    this.lastMisses = g.misses;
    this.game = g;
  }

  draw(now) {
    this.dish.draw(this.culture.glow(GLOW_TAU));
    this.drawOverlay();
    this.drawPong(now);
    const chip = this.q('chip');
    const g = this.game;
    const label = g.stimulus === 'predictable' ? 'Predictable pulse'
      : g.stimulus === 'unpredictable' ? 'Noise'
        : g.waiting ? 'Waiting to serve' : g.quiet ? `Rally ${g.rally} · quiet` : `Rally ${g.rally}`;
    if (chip.textContent !== label) chip.textContent = label;
    chip.dataset.state = g.stimulus;
    this.q('dish').dataset.state = g.stimulus;
  }

  drawOverlay() {
    const c = this.culture;
    const w = this.overlay.width;
    const s = w / 2 / (c.radius * MARGIN);
    const ctx = this.octx;
    ctx.clearRect(0, 0, w, w);
    const g = this.game;
    const row = Math.min(7, Math.floor(g.ball[1] * 8)) + 1;
    const side = Math.max(4 * dpr(), 34 * s);
    for (let e = 0; e < c.electrodeCount; e++) {
      const role = c.roles[e];
      if (role === 'unused') continue;
      const x = w / 2 + c.electrodes[2 * e] * s - side / 2;
      const y = w / 2 + c.electrodes[2 * e + 1] * s - side / 2;
      if (role === 'sensory') {
        const lit = g.stimulus === 'predictable'
          || (!g.waiting && !g.quiet && g.stimulus === 'none' && c.electrodeNames[e].row === row);
        ctx.fillStyle = rgba(CHAMPAGNE, lit ? 0.55 : 0.08);
        ctx.fillRect(x, y, side, side);
        ctx.strokeStyle = rgba(CHAMPAGNE, lit ? 0.95 : 0.45);
      } else {
        const col = role === 'up' ? UP : DOWN;
        ctx.fillStyle = rgba(col, 0.1);
        ctx.fillRect(x, y, side, side);
        ctx.strokeStyle = rgba(col, 0.55);
      }
      ctx.lineWidth = dpr();
      ctx.strokeRect(x, y, side, side);
    }
  }

  drawPong(now) {
    const ctx = this.pctx;
    const { width: w, height: h } = this.pong;
    const g = this.game;
    const pad = 6 * dpr();
    const fw = w - 2 * pad;
    const fh = h - 2 * pad;
    const X = (x) => pad + x * fw;
    const Y = (y) => pad + y * fh;
    ctx.fillStyle = '#050806';
    ctx.fillRect(0, 0, w, h);

    // The row the culture is told about, unless avoidance holds it back.
    if (!g.waiting && !g.quiet) {
      const row = Math.min(7, Math.floor(g.ball[1] * 8));
      ctx.fillStyle = rgba(CHAMPAGNE, 0.06);
      ctx.fillRect(pad, Y(row / 8), fw, fh / 8);
    }
    if (!g.waiting && g.quiet) {
      ctx.fillStyle = 'rgba(141,154,147,0.7)';
      ctx.font = `${10 * dpr()}px "JetBrains Mono", monospace`;
      ctx.textAlign = 'right';
      ctx.fillText('input quiet', w - pad - 6 * dpr(), h - pad - 8 * dpr());
      ctx.textAlign = 'start';
    }

    // Hit and miss flashes.
    if (this.flash && now - this.flash.at < 400) {
      const k = 1 - (now - this.flash.at) / 400;
      ctx.fillStyle = this.flash.kind === 'hit' ? rgba(FIREFLY, 0.18 * k) : rgba([255, 120, 110], 0.16 * k);
      ctx.fillRect(pad, pad, fw, fh);
    }

    ctx.strokeStyle = 'rgba(141,154,147,0.35)';
    ctx.lineWidth = dpr();
    ctx.strokeRect(pad, pad, fw, fh);

    // Paddle, with the direction the motor regions push it.
    const ph = 0.25 * fh;
    const py = Y(g.paddle) - ph / 2;
    ctx.fillStyle = rgba(CHAMPAGNE, 0.95);
    ctx.fillRect(pad, py, 4 * dpr(), ph);
    ctx.fillStyle = rgba(g.drive < 0 ? UP : DOWN, 0.8);
    const len = Math.abs(g.drive) * ph * 0.6;
    ctx.fillRect(pad + 7 * dpr(), g.drive < 0 ? Y(g.paddle) - len : Y(g.paddle), 2 * dpr(), len);

    if (g.waiting) {
      ctx.fillStyle = 'rgba(141,154,147,0.8)';
      ctx.font = `${11 * dpr()}px "JetBrains Mono", monospace`;
      ctx.textAlign = 'center';
      ctx.fillText('serving soon', w / 2, h / 2);
      ctx.textAlign = 'start';
    } else {
      const [bx, by] = [X(g.ball[0]), Y(g.ball[1])];
      const r = 0.02 * fw;
      const glow = ctx.createRadialGradient(bx, by, 0, bx, by, r * 4);
      glow.addColorStop(0, rgba(FIREFLY, 0.5));
      glow.addColorStop(1, rgba(FIREFLY, 0));
      ctx.fillStyle = glow;
      ctx.fillRect(bx - r * 4, by - r * 4, r * 8, r * 8);
      ctx.fillStyle = rgba([251, 255, 224], 1);
      ctx.beginPath();
      ctx.arc(bx, by, r, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  summary() {
    const r = this.culture.rallies();
    const now = this.culture.time;
    let hits = 0;
    let aces = 0;
    let recentHits = 0;
    let recent = 0;
    for (let k = 0; k < r.length; k += 2) {
      hits += r[k + 1];
      if (r[k + 1] === 0) aces++;
      if (r[k] > now - RECENT_MS) { recentHits += r[k + 1]; recent++; }
    }
    const count = r.length / 2;
    const mean = count ? hits / count : NaN;
    let sq = 0;
    for (let k = 1; k < r.length; k += 2) sq += (r[k] - mean) ** 2;
    const variance = count > 1 ? sq / (count - 1) : NaN;
    return { count, mean, variance, aces: count ? aces / count : NaN, recent: recent ? recentHits / recent : NaN };
  }

  drawStats() {
    const s = this.summary();
    const fmt = (v, d = 2) => (Number.isFinite(v) ? v.toFixed(d) : '–');
    this.stat('rallies').textContent = String(s.count);
    this.stat('mean').textContent = fmt(s.mean);
    this.stat('recent').textContent = fmt(s.recent);
    this.stat('aces').textContent = Number.isFinite(s.aces) ? `${Math.round(100 * s.aces)}%` : '–';
    const k = this.game.coupling;
    this.stat('coupling').textContent = Number.isFinite(k) ? `${k >= 0 ? '+' : '−'}${Math.abs(k).toFixed(2)}` : '–';
  }

  drawWeight() {
    const el = this.stat('weight');
    el.textContent = this.culture.weight().toFixed(2);
    el.title = `Mean excitatory weight; it started at ${this.startWeight.toFixed(2)}`;
  }

  /** Rolling hits per rally at each rally end: [[t ms, mean], ...]. */
  curve() {
    const r = this.culture.rallies();
    const out = [];
    let lo = 0;
    let sum = 0;
    for (let k = 0; k < r.length; k += 2) {
      sum += r[k + 1];
      while (r[lo] <= r[k] - ROLLING_MS) { sum -= r[lo + 1]; lo += 2; }
      const count = (k - lo) / 2 + 1;
      if (count >= 3) out.push([r[k], sum / count]);
    }
    return out;
  }
}

const players = [new Player($('p-feedback')), new Player($('p-control'))];
let seed;
let size;
let experiment;
/** Simulated ms at which the two cultures first fired differently, or null. */
let splitAt = null;

function sameSpikes() {
  const [a, b] = players.map((p) => p.culture.spikes());
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

// --- verdict ----------------------------------------------------------------------

const clock = (ms) => {
  const t = Math.floor(ms / 1000);
  return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
};

/** One plain sentence on where the comparison stands. */
function drawVerdict() {
  const el = $('verdict');
  let state;
  let html;
  if (splitAt === null) {
    state = 'same';
    html = '<b>The two cultures are still exact copies.</b> Same neurons, same serves, and so far the same input, '
      + 'so they move in step. They split the first time their input differs.';
  } else {
    const [f, c] = players.map((p) => p.summary());
    const n = Math.min(f.count, c.count);
    if (n < COMPARE_AFTER) {
      state = 'split';
      html = `<b>They split at ${clock(splitAt)}</b>, when their input first differed. From here on the condition is `
        + `the only difference between them. A fair comparison needs about ${COMPARE_AFTER} rallies each `
        + `(${n} so far); the 5× or 10× speed gets there sooner.`;
    } else {
      const diff = f.mean - c.mean;
      const chance = 2 * Math.sqrt(f.variance / f.count + c.variance / c.count);
      const x = EXPERIMENTS[experiment];
      const avoid = experiment === 'avoid';
      const nums = `<b class="firefly">${f.mean.toFixed(2)}</b> hits per rally ${x.short[0]}, `
        + `<b class="ice">${c.mean.toFixed(2)}</b> ${x.short[1]}`;
      const after = `After ${f.count} and ${c.count} rallies: ${nums}.`;
      const margin = `(±${chance.toFixed(2)})`;
      if (Math.abs(diff) <= chance) {
        state = 'split';
        html = avoid
          ? `${after} The gap is within what chance alone produces ${margin} so far. Across 16 cultures it opened `
            + 'over about 20 simulated minutes; 10× speed gets there sooner.'
          : `${after} The gap is within what chance alone produces ${margin}, so <b>the feedback has not made a `
            + 'difference</b>. That is what we found across 16 cultures, too.';
      } else if (diff > 0) {
        state = 'ahead';
        html = avoid
          ? `${after} <b>The culture is learning:</b> it is ahead by more than chance alone would explain ${margin}. `
            + 'Across 16 cultures, 13 got better this way while the mirrored controls did not.'
          : `${after} <b>The feedback culture is ahead</b> by more than chance alone would explain ${margin}. `
            + 'Across 16 cultures such leads did not hold up; see whether this one does.';
      } else {
        state = 'split';
        html = `${after} <b>The control is ahead</b> by more than chance alone would explain ${margin}, so the `
          + 'condition is not helping this culture.';
      }
    }
  }
  if (el.dataset.html !== html) {
    el.innerHTML = html;
    el.dataset.html = html;
  }
  el.dataset.state = state;
}

// --- chart ------------------------------------------------------------------------

const chart = $('chart');
const cctx = chart.getContext('2d');

function drawChart() {
  const { width: w, height: h } = chart;
  cctx.clearRect(0, 0, w, h);
  const curves = players.map((p) => p.curve());
  const now = players[0].culture.time;
  const span = Math.max(5 * 60_000, now);
  let top = 1;
  for (const c of curves) for (const [, v] of c) top = Math.max(top, v);
  top = Math.ceil(top * 1.15 * 2) / 2;
  const left = 34 * dpr();
  const bottom = 18 * dpr();
  const X = (t) => left + (t / span) * (w - left - 6 * dpr());
  const Y = (v) => (h - bottom) - (v / top) * (h - bottom - 8 * dpr());

  cctx.font = `${10.5 * dpr()}px "JetBrains Mono", monospace`;
  cctx.fillStyle = 'rgba(141,154,147,0.75)';
  cctx.strokeStyle = 'rgba(141,154,147,0.14)';
  cctx.lineWidth = dpr();
  cctx.textBaseline = 'middle';
  for (let v = 0; v <= top + 1e-6; v += 0.5) {
    cctx.beginPath();
    cctx.moveTo(left, Y(v));
    cctx.lineTo(w, Y(v));
    cctx.stroke();
    cctx.fillText(v.toFixed(1), 4 * dpr(), Y(v));
  }
  cctx.textBaseline = 'top';
  const stepMin = span > 40 * 60_000 ? 10 : span > 12 * 60_000 ? 5 : 1;
  for (let m = 0; m * 60_000 <= span; m += stepMin) {
    cctx.fillText(`${m}m`, X(m * 60_000) - 6 * dpr(), h - bottom + 4 * dpr());
  }

  curves.forEach((c, i) => {
    if (c.length < 2) return;
    cctx.strokeStyle = rgba(i === 0 ? FIREFLY : ICE, 0.95);
    cctx.lineWidth = 2 * dpr();
    cctx.beginPath();
    c.forEach(([t, v], k) => (k ? cctx.lineTo(X(t), Y(v)) : cctx.moveTo(X(t), Y(v))));
    cctx.stroke();
  });
  if (curves.every((c) => c.length < 2)) {
    cctx.fillStyle = 'rgba(141,154,147,0.7)';
    cctx.textAlign = 'center';
    cctx.fillText('a line appears after a few rallies', w / 2, h / 2 - 10 * dpr());
    cctx.textAlign = 'start';
  }
}

// --- lifecycle ----------------------------------------------------------------------

function readHash() {
  const p = new URLSearchParams(location.hash.slice(1));
  const s = Number.parseInt(p.get('seed'), 10);
  const n = Number.parseInt(p.get('n'), 10);
  return {
    seed: Number.isFinite(s) && s >= 0 ? s >>> 0 : randomSeed(),
    n: SIZES.includes(n) ? n : 2048,
    // `control` is the v0.2 name for the two feedback experiments.
    experiment: [p.get('exp'), p.get('control')].find((e) => e in EXPERIMENTS) ?? 'avoid',
  };
}

function randomSeed() {
  return crypto.getRandomValues(new Uint32Array(1))[0] % 1_000_000;
}

function begin(s, n, e) {
  seed = s;
  size = n;
  experiment = e;
  const x = EXPERIMENTS[experiment];
  players.forEach((p, i) => {
    p.start(seed, size, x.modes[i]);
    p.q('title').textContent = x.titles[i];
    p.q('note').textContent = x.notes[i];
  });
  $('size').value = String(size);
  $('experiment').value = experiment;
  $('s-seed').textContent = String(seed);
  $('legend-learner').textContent = x.legend[0];
  $('legend-control').textContent = x.legend[1];
  carry = 0;
  splitAt = null;
  history.replaceState(null, '', `#seed=${seed}&n=${size}&exp=${experiment}`);
}

// --- frame loop -----------------------------------------------------------------------

function fit(canvas, aspect) {
  const r = canvas.getBoundingClientRect();
  const w = Math.max(1, Math.round(r.width * dpr()));
  const h = aspect ? Math.round(w / aspect) : Math.max(1, Math.round(r.height * dpr()));
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
}

new ResizeObserver(() => {
  for (const p of players) {
    fit(p.q('neurons'), 1);
    fit(p.overlay, 1);
    fit(p.pong, 1);
  }
  fit(chart);
}).observe(document.body);

let playing = true;
let speed = 1;
let carry = 0;
let costPerMs = 0.05;
let slowFrames = 0;
let lastWall = performance.now();
let lastStats = 0;
let lastWeight = 0;

function frame(now) {
  const dt = Math.min(100, now - lastWall);
  lastWall = now;
  if (playing) {
    const want = dt * speed + carry;
    const cap = Math.max(1, Math.floor(FRAME_BUDGET / costPerMs));
    let ms = Math.floor(want);
    if (ms > cap) {
      ms = cap;
      carry = 0;
      if (speed < 1000) slowFrames = Math.min(slowFrames + 1, 60);
    } else {
      carry = want - ms;
      slowFrames = Math.max(slowFrames - 1, 0);
    }
    if (ms > 0) {
      const t0 = performance.now();
      for (const p of players) p.step(ms);
      if (splitAt === null && !sameSpikes()) splitAt = players[0].culture.time;
      costPerMs = costPerMs * 0.9 + ((performance.now() - t0) / ms) * 0.1;
    }
    $('slow').hidden = slowFrames < 30;
  }
  for (const p of players) p.draw(now);
  if (now - lastStats > 250) {
    lastStats = now;
    for (const p of players) p.drawStats();
    drawChart();
    drawVerdict();
    $('s-time').textContent = clock(players[0].culture.time);
  }
  if (now - lastWeight > 2000) {
    lastWeight = now;
    for (const p of players) p.drawWeight();
  }
  requestAnimationFrame(frame);
}

// --- controls ---------------------------------------------------------------------------

function setPlaying(p) {
  playing = p;
  $('play').textContent = p ? 'Pause' : 'Play';
  $('play').setAttribute('aria-pressed', String(p));
}

$('play').addEventListener('click', () => setPlaying(!playing));
$('new').addEventListener('click', () => begin(randomSeed(), size, experiment));
$('size').addEventListener('change', (ev) => begin(seed, Number(ev.target.value), experiment));
$('experiment').addEventListener('change', (ev) => begin(seed, size, ev.target.value));
$('speed').addEventListener('change', (ev) => { speed = Number(ev.target.value); carry = 0; slowFrames = 0; });
$('share').addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(location.href);
    $('share').textContent = 'Copied';
  } catch {
    $('share').textContent = 'Copy failed';
  }
  setTimeout(() => { $('share').textContent = 'Copy link'; }, 1500);
});
window.addEventListener('keydown', (ev) => {
  if (ev.code === 'Space' && !(ev.target instanceof HTMLSelectElement) && !(ev.target instanceof HTMLButtonElement)) {
    ev.preventDefault();
    setPlaying(!playing);
  }
});
window.addEventListener('hashchange', () => {
  const h = readHash();
  if (h.seed !== seed || h.n !== size || h.experiment !== experiment) begin(h.seed, h.n, h.experiment);
});

const start = readHash();
begin(start.seed, start.n, start.experiment);
requestAnimationFrame(frame);

import { loadHotaru } from './hotaru.js';
import { createDish, FIREFLY, ICE, CHAMPAGNE, MARGIN } from './dish.js';

const GLOW_TAU = 45;          // ms of simulated time a flash takes to fade
const FRAME_BUDGET = 12;      // wall ms per frame both simulations may use
const ROLLING_MS = 120_000;   // window of the hits-per-rally chart
const RECENT_MS = 300_000;
const SIZES = [1024, 2048, 4096];
const CONTROLS = { random: 'Control, random feedback', silent: 'Control, no feedback' };

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
        : g.waiting ? 'Waiting to serve' : `Rally ${g.rally}`;
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
        const lit = g.stimulus === 'predictable' || (!g.waiting && g.stimulus === 'none' && c.electrodeNames[e].row === row);
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

    // The row the culture is told about.
    if (!g.waiting) {
      const row = Math.min(7, Math.floor(g.ball[1] * 8));
      ctx.fillStyle = rgba(CHAMPAGNE, 0.06);
      ctx.fillRect(pad, Y(row / 8), fw, fh / 8);
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
    return { count, mean: count ? hits / count : NaN, aces: count ? aces / count : NaN, recent: recent ? recentHits / recent : NaN };
  }

  drawStats() {
    const s = this.summary();
    const fmt = (v, d = 2) => (Number.isFinite(v) ? v.toFixed(d) : '–');
    this.stat('rallies').textContent = String(s.count);
    this.stat('mean').textContent = fmt(s.mean);
    this.stat('recent').textContent = fmt(s.recent);
    this.stat('aces').textContent = Number.isFinite(s.aces) ? `${Math.round(100 * s.aces)}%` : '–';
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
let control;

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
    control: p.get('control') in CONTROLS ? p.get('control') : 'random',
  };
}

function randomSeed() {
  return crypto.getRandomValues(new Uint32Array(1))[0] % 1_000_000;
}

function begin(s, n, c) {
  seed = s;
  size = n;
  control = c;
  players[0].start(seed, size, 'feedback');
  players[1].start(seed, size, control);
  $('size').value = String(size);
  $('control').value = control;
  $('s-seed').textContent = String(seed);
  $('legend-control').textContent = CONTROLS[control];
  carry = 0;
  history.replaceState(null, '', `#seed=${seed}&n=${size}&control=${control}`);
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
      costPerMs = costPerMs * 0.9 + ((performance.now() - t0) / ms) * 0.1;
    }
    $('slow').hidden = slowFrames < 30;
  }
  for (const p of players) p.draw(now);
  if (now - lastStats > 250) {
    lastStats = now;
    for (const p of players) p.drawStats();
    drawChart();
    const t = Math.floor(players[0].culture.time / 1000);
    $('s-time').textContent = `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
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
$('new').addEventListener('click', () => begin(randomSeed(), size, control));
$('size').addEventListener('change', (ev) => begin(seed, Number(ev.target.value), control));
$('control').addEventListener('change', (ev) => begin(seed, size, ev.target.value));
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
  if (h.seed !== seed || h.n !== size || h.control !== control) begin(h.seed, h.n, h.control);
});

const start = readHash();
begin(start.seed, start.n, start.control);
requestAnimationFrame(frame);

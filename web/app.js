import { loadHotaru } from './hotaru.js';

const WINDOW_MS = 3000;    // raster and rate history
const BIN_MS = 10;
const BINS = WINDOW_MS / BIN_MS;
const BURST_HZ = 20;       // a 10 ms bin at this population rate is a burst
const GLOW_TAU = 45;       // ms of simulated time a flash takes to fade
const FRAME_BUDGET = 10;   // wall ms per frame the simulation may use
const SPIKE_CAP = 1 << 19;
const SIZES = [1024, 2048, 4096, 8192];

const FIREFLY = [227, 249, 138];
const ICE = [159, 231, 255];
const CHAMPAGNE = [217, 199, 156];

const $ = (id) => document.getElementById(id);
const dpr = () => Math.min(window.devicePixelRatio || 1, 2);

const hotaru = await loadHotaru();

let culture;
let rowOf;                  // raster row rank of each neuron
let spikeT, spikeN, spikeHead, spikeLen;
let bins, binIndex, aboveBurst, burstTimes;
let meaRate;                // smoothed spikes/s per electrode
let stimFlashes = [];       // { e, at } in wall-clock ms
let playing = true;
let speed = 1;
let carry = 0;
let costPerMs = 0.02;       // measured wall ms per simulated ms
let slowFrames = 0;
let hover = -1;

// --- culture lifecycle ------------------------------------------------------

function readHash() {
  const p = new URLSearchParams(location.hash.slice(1));
  const seed = Number.parseInt(p.get('seed'), 10);
  const n = Number.parseInt(p.get('n'), 10);
  return {
    seed: Number.isFinite(seed) && seed >= 0 ? seed >>> 0 : randomSeed(),
    n: SIZES.includes(n) ? n : 4096,
  };
}

function randomSeed() {
  return crypto.getRandomValues(new Uint32Array(1))[0] % 1_000_000;
}

function writeHash() {
  history.replaceState(null, '', `#seed=${culture.seed}&n=${culture.n}`);
}

function grow(seed, n) {
  culture = hotaru.create(seed, n);
  const order = Array.from({ length: culture.n }, (_, i) => i)
    .sort((a, b) => culture.positions[2 * a] - culture.positions[2 * b]);
  rowOf = new Uint32Array(culture.n);
  order.forEach((i, rank) => { rowOf[i] = rank; });

  spikeT = new Uint32Array(SPIKE_CAP);
  spikeN = new Uint32Array(SPIKE_CAP);
  spikeHead = 0;
  spikeLen = 0;
  bins = new Uint32Array(BINS);
  binIndex = 0;
  aboveBurst = false;
  burstTimes = [];
  meaRate = new Float32Array(culture.electrodeCount);
  stimFlashes = [];
  carry = 0;

  dish.upload(culture);
  $('size').value = String(culture.n);
  $('s-neurons').textContent = culture.n.toLocaleString('en');
  $('s-synapses').textContent = culture.synapses.toLocaleString('en');
  $('s-seed').textContent = String(culture.seed);
  writeHash();
}

// --- recording --------------------------------------------------------------

function record(ms) {
  const spikes = culture.spikes();
  for (let k = 0; k < spikes.length; k += 2) {
    const t = spikes[k];
    advanceBins(Math.floor(t / BIN_MS));
    bins[binIndex % BINS]++;
    const at = (spikeHead + spikeLen) % SPIKE_CAP;
    spikeT[at] = t;
    spikeN[at] = spikes[k + 1];
    if (spikeLen < SPIKE_CAP) spikeLen++;
    else spikeHead = (spikeHead + 1) % SPIKE_CAP;
  }
  const now = culture.time;
  advanceBins(Math.floor((now - 1) / BIN_MS));
  while (spikeLen > 0 && spikeT[spikeHead] + WINDOW_MS < now) {
    spikeHead = (spikeHead + 1) % SPIKE_CAP;
    spikeLen--;
  }

  const counts = culture.counts();
  const keep = Math.exp(-ms / 150);
  for (let e = 0; e < meaRate.length; e++) {
    meaRate[e] = meaRate[e] * keep + (counts[e] * 1000 / ms) * (1 - keep);
  }
}

// Move to bin `b`, judging each completed bin for bursts on the way.
function advanceBins(b) {
  while (binIndex < b) {
    const hz = bins[binIndex % BINS] / culture.n / (BIN_MS / 1000);
    const burst = hz >= BURST_HZ;
    if (burst && !aboveBurst) burstTimes.push(binIndex * BIN_MS);
    aboveBurst = burst;
    binIndex++;
    bins[binIndex % BINS] = 0;
  }
}

// --- dish: WebGL2 points with additive glow ---------------------------------

const dish = (() => {
  const canvas = $('neurons');
  const gl = canvas.getContext('webgl2', { premultipliedAlpha: true, antialias: false });
  let n = 0;
  let radius = 1000;

  if (!gl) {
    // Without WebGL2, draw only the neurons that are lit.
    const ctx = canvas.getContext('2d');
    let positions;
    let kinds;
    return {
      upload(c) { positions = c.positions; kinds = c.kinds; n = c.n; radius = c.radius; },
      draw(glow) {
        const { width: w } = canvas;
        ctx.clearRect(0, 0, w, w);
        const s = w / 2 / (radius * 1.04);
        for (let i = 0; i < n; i++) {
          const g = glow[i];
          if (g < 0.03) continue;
          const [r, gg, b] = kinds[i] ? ICE : FIREFLY;
          ctx.fillStyle = `rgba(${r},${gg},${b},${g})`;
          ctx.fillRect(w / 2 + positions[2 * i] * s - 1.5, w / 2 + positions[2 * i + 1] * s - 1.5, 3, 3);
        }
      },
    };
  }

  const vs = `#version 300 es
    in vec2 a_pos;
    in float a_kind;
    in float a_glow;
    uniform float u_unit;
    uniform float u_size;
    out float v_glow;
    out float v_kind;
    void main() {
      gl_Position = vec4(a_pos.x * u_unit, -a_pos.y * u_unit, 0.0, 1.0);
      gl_PointSize = u_size * (1.0 + 3.0 * a_glow);
      v_glow = a_glow;
      v_kind = a_kind;
    }`;
  const fs = `#version 300 es
    precision mediump float;
    in float v_glow;
    in float v_kind;
    uniform vec3 u_exc;
    uniform vec3 u_inh;
    out vec4 o;
    void main() {
      vec2 p = gl_PointCoord * 2.0 - 1.0;
      float r2 = dot(p, p);
      if (r2 > 1.0) discard;
      float core = exp(-r2 * 9.0);
      float halo = exp(-r2 * 2.5) * 0.4;
      vec3 hot = mix(u_exc, u_inh, v_kind);
      vec3 dim = vec3(0.10, 0.15, 0.12) * core;
      vec3 c = dim + hot * (core * 1.3 + halo) * v_glow;
      o = vec4(c, max(c.r, max(c.g, c.b)));
    }`;
  const compile = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    return s;
  };
  const prog = gl.createProgram();
  gl.attachShader(prog, compile(gl.VERTEX_SHADER, vs));
  gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog));
  gl.useProgram(prog);
  const u = (name) => gl.getUniformLocation(prog, name);
  gl.uniform3f(u('u_exc'), ...FIREFLY.map((c) => c / 255));
  gl.uniform3f(u('u_inh'), ...ICE.map((c) => c / 255));

  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  const attrib = (name, size, type = gl.FLOAT) => {
    const buf = gl.createBuffer();
    const loc = gl.getAttribLocation(prog, name);
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, size, type, false, 0, 0);
    return buf;
  };
  const posBuf = attrib('a_pos', 2);
  const kindBuf = attrib('a_kind', 1, gl.UNSIGNED_BYTE);
  const glowBuf = attrib('a_glow', 1);

  gl.enable(gl.BLEND);
  gl.blendFunc(gl.ONE, gl.ONE);
  gl.clearColor(0, 0, 0, 0);

  return {
    upload(c) {
      n = c.n;
      radius = c.radius;
      gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
      gl.bufferData(gl.ARRAY_BUFFER, c.positions, gl.STATIC_DRAW);
      gl.bindBuffer(gl.ARRAY_BUFFER, kindBuf);
      gl.bufferData(gl.ARRAY_BUFFER, c.kinds, gl.STATIC_DRAW);
      gl.bindBuffer(gl.ARRAY_BUFFER, glowBuf);
      gl.bufferData(gl.ARRAY_BUFFER, n * 4, gl.DYNAMIC_DRAW);
    },
    draw(glow) {
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.bindBuffer(gl.ARRAY_BUFFER, glowBuf);
      gl.bufferSubData(gl.ARRAY_BUFFER, 0, glow);
      gl.uniform1f(u('u_unit'), 1 / (radius * 1.04));
      // About 12 µm across at rest, at least 2 device pixels.
      gl.uniform1f(u('u_size'), Math.max(2, (canvas.width / (2 * radius * 1.04)) * 12));
      gl.drawArrays(gl.POINTS, 0, n);
    },
  };
})();

// --- overlay: electrodes, stimulation rings, hover --------------------------

const overlay = $('overlay');
const octx = overlay.getContext('2d');

function dishScale() {
  return overlay.width / 2 / (culture.radius * 1.04);
}

function drawOverlay(nowWall) {
  const w = overlay.width;
  const s = dishScale();
  octx.clearRect(0, 0, w, w);
  const side = Math.max(4 * dpr(), 30 * s);
  for (let e = 0; e < culture.electrodeCount; e++) {
    const x = w / 2 + culture.electrodes[2 * e] * s;
    const y = w / 2 + culture.electrodes[2 * e + 1] * s;
    const a = Math.min(1, 0.28 + meaRate[e] / 400 + (e === hover ? 0.5 : 0));
    octx.strokeStyle = `rgba(${CHAMPAGNE},${a})`;
    octx.lineWidth = dpr();
    octx.strokeRect(x - side / 2, y - side / 2, side, side);
  }
  stimFlashes = stimFlashes.filter((f) => nowWall - f.at < 700);
  for (const f of stimFlashes) {
    const k = (nowWall - f.at) / 700;
    const x = w / 2 + culture.electrodes[2 * f.e] * s;
    const y = w / 2 + culture.electrodes[2 * f.e + 1] * s;
    octx.strokeStyle = `rgba(${CHAMPAGNE},${0.9 * (1 - k)})`;
    octx.lineWidth = 1.5 * dpr();
    octx.beginPath();
    octx.arc(x, y, (100 * s) * (0.4 + k), 0, Math.PI * 2);
    octx.stroke();
  }
}

function electrodeAt(clientX, clientY) {
  const rect = overlay.getBoundingClientRect();
  const s = rect.width / 2 / (culture.radius * 1.04);
  const x = (clientX - rect.left - rect.width / 2) / s;
  const y = (clientY - rect.top - rect.height / 2) / s;
  let best = -1;
  let bestD = 90;
  for (let e = 0; e < culture.electrodeCount; e++) {
    const d = Math.hypot(culture.electrodes[2 * e] - x, culture.electrodes[2 * e + 1] - y);
    if (d < bestD) { best = e; bestD = d; }
  }
  return best;
}

function stimulate(e) {
  culture.stimulate(e);
  stimFlashes.push({ e, at: performance.now() });
}

function electrodeName(e) {
  const { col, row } = culture.electrodeNames[e];
  return `electrode ${col}${row}`;
}

overlay.addEventListener('pointermove', (ev) => {
  hover = electrodeAt(ev.clientX, ev.clientY);
  overlay.style.cursor = hover >= 0 ? 'pointer' : 'default';
  const tip = $('tip');
  if (hover < 0) { tip.hidden = true; return; }
  const rect = overlay.getBoundingClientRect();
  const s = rect.width / 2 / (culture.radius * 1.04);
  tip.hidden = false;
  tip.textContent = `${electrodeName(hover)} · ${meaRate[hover].toFixed(0)} spikes/s`;
  tip.style.left = `${rect.width / 2 + culture.electrodes[2 * hover] * s}px`;
  tip.style.top = `${rect.height / 2 + culture.electrodes[2 * hover + 1] * s}px`;
});
overlay.addEventListener('pointerleave', () => { hover = -1; $('tip').hidden = true; });
overlay.addEventListener('click', (ev) => {
  const e = electrodeAt(ev.clientX, ev.clientY);
  if (e >= 0) stimulate(e);
});

// --- electrode array heatmap ------------------------------------------------

const mea = $('mea');
const mctx = mea.getContext('2d');

function meaCell(e) {
  const { col, row } = culture.electrodeNames[e];
  const cell = mea.width / 8;
  return { x: (col - 1) * cell, y: (row - 1) * cell, cell };
}

function heat(v) {
  // Dark → firefly → near white.
  const lo = [18, 28, 23];
  const hi = [251, 255, 224];
  const mix = (a, b, t) => a.map((c, i) => Math.round(c + (b[i] - c) * t));
  return v < 0.7 ? mix(lo, FIREFLY, v / 0.7) : mix(FIREFLY, hi, (v - 0.7) / 0.3);
}

function drawMea(nowWall) {
  const w = mea.width;
  mctx.clearRect(0, 0, w, w);
  const pad = Math.max(2, w / 80);
  mctx.font = `${Math.round(w / 34)}px "JetBrains Mono", monospace`;
  mctx.textBaseline = 'top';
  for (let e = 0; e < culture.electrodeCount; e++) {
    const { x, y, cell } = meaCell(e);
    const v = Math.min(1, Math.log1p(meaRate[e]) / Math.log1p(400));
    mctx.fillStyle = `rgb(${heat(v)})`;
    mctx.beginPath();
    mctx.roundRect(x + pad, y + pad, cell - 2 * pad, cell - 2 * pad, cell / 8);
    mctx.fill();
    const flash = stimFlashes.find((f) => f.e === e);
    if (flash || e === hover) {
      const k = flash ? 1 - (nowWall - flash.at) / 700 : 0.6;
      mctx.strokeStyle = `rgba(${CHAMPAGNE},${k})`;
      mctx.lineWidth = 2 * dpr();
      mctx.stroke();
    }
    mctx.fillStyle = v > 0.55 ? 'rgba(11,18,14,0.6)' : 'rgba(141,154,147,0.55)';
    const { col, row } = culture.electrodeNames[e];
    mctx.fillText(`${col}${row}`, x + pad * 3, y + pad * 3);
  }
}

function meaAt(ev) {
  const rect = mea.getBoundingClientRect();
  const col = Math.floor(((ev.clientX - rect.left) / rect.width) * 8) + 1;
  const row = Math.floor(((ev.clientY - rect.top) / rect.height) * 8) + 1;
  return culture.electrodeNames.findIndex((n) => n.col === col && n.row === row);
}

mea.addEventListener('pointermove', (ev) => { hover = meaAt(ev); });
mea.addEventListener('pointerleave', () => { hover = -1; });
mea.addEventListener('click', (ev) => {
  const e = meaAt(ev);
  if (e >= 0) stimulate(e);
});

// --- raster and population rate ---------------------------------------------

const raster = $('raster');
const rctx = raster.getContext('2d');
let rasterImage;
let rasterPixels;

function drawRaster() {
  const { width: w, height: h } = raster;
  if (!rasterImage || rasterImage.width !== w || rasterImage.height !== h) {
    rasterImage = rctx.createImageData(w, h);
    rasterPixels = new Uint32Array(rasterImage.data.buffer);
  }
  rasterPixels.fill(0xff060805);
  const start = culture.time - WINDOW_MS;
  const exc = 0xff000000 | (FIREFLY[2] << 16) | (FIREFLY[1] << 8) | FIREFLY[0];
  const inh = 0xff000000 | (ICE[2] << 16) | (ICE[1] << 8) | ICE[0];
  for (let k = 0; k < spikeLen; k++) {
    const at = (spikeHead + k) % SPIKE_CAP;
    const x = Math.floor(((spikeT[at] - start) / WINDOW_MS) * w);
    if (x < 0 || x >= w) continue;
    const i = spikeN[at];
    const y = Math.floor((rowOf[i] * h) / culture.n);
    rasterPixels[y * w + x] = culture.kinds[i] ? inh : exc;
  }
  rctx.putImageData(rasterImage, 0, 0);
}

const rate = $('rate');
const pctx = rate.getContext('2d');

function drawRate() {
  const { width: w, height: h } = rate;
  pctx.clearRect(0, 0, w, h);
  const hz = new Float32Array(BINS);
  let peak = 0;
  // Oldest bin first; the newest, still filling, bin is left out.
  for (let k = 0; k < BINS - 1; k++) {
    const b = binIndex - (BINS - 1) + k;
    hz[k] = b < 0 ? 0 : bins[b % BINS] / culture.n / (BIN_MS / 1000);
    peak = Math.max(peak, hz[k]);
  }
  const top = Math.max(50, peak * 1.15);
  const yOf = (v) => h - (v / top) * (h - 4 * dpr());
  const step = w / (BINS - 2);

  pctx.setLineDash([4 * dpr(), 4 * dpr()]);
  pctx.strokeStyle = 'rgba(217,199,156,0.35)';
  pctx.lineWidth = dpr();
  pctx.beginPath();
  pctx.moveTo(0, yOf(BURST_HZ));
  pctx.lineTo(w, yOf(BURST_HZ));
  pctx.stroke();
  pctx.setLineDash([]);

  pctx.beginPath();
  pctx.moveTo(0, h);
  for (let k = 0; k < BINS - 1; k++) pctx.lineTo(k * step, yOf(hz[k]));
  pctx.lineTo(w, h);
  pctx.closePath();
  pctx.fillStyle = 'rgba(227,249,138,0.16)';
  pctx.fill();
  pctx.beginPath();
  for (let k = 0; k < BINS - 1; k++) pctx.lineTo(k * step, yOf(hz[k]));
  pctx.strokeStyle = 'rgba(227,249,138,0.9)';
  pctx.lineWidth = 1.5 * dpr();
  pctx.stroke();

  pctx.font = `${11 * dpr()}px "JetBrains Mono", monospace`;
  pctx.fillStyle = 'rgba(141,154,147,0.8)';
  pctx.textBaseline = 'top';
  pctx.fillText(`${Math.round(top)} Hz`, 4 * dpr(), 2 * dpr());
  pctx.textBaseline = 'bottom';
  pctx.fillText(`burst ${BURST_HZ} Hz`, w - 92 * dpr(), yOf(BURST_HZ) - 2 * dpr());
}

// --- stats ------------------------------------------------------------------

function drawStats() {
  const now = culture.time;
  $('s-time').textContent = `${(now / 1000).toFixed(1)} s`;
  // Mean over the last second of completed bins.
  let sum = 0;
  const last = Math.min(100, binIndex);
  for (let k = 1; k <= last; k++) sum += bins[(binIndex - k) % BINS];
  $('s-rate').textContent = last ? `${(sum / culture.n / (last * BIN_MS / 1000)).toFixed(2)} Hz` : '–';
  const minute = burstTimes.filter((t) => t > now - 60000).length;
  const span = Math.min(60, now / 1000);
  $('s-bursts').textContent = span >= 5
    ? `${burstTimes.length} · ${(minute * 60 / span).toFixed(0)}/min`
    : String(burstTimes.length);
}

// --- sizing and the frame loop ------------------------------------------------

function fit(canvas, square) {
  const r = canvas.getBoundingClientRect();
  const w = Math.max(1, Math.round(r.width * dpr()));
  const h = square ? w : Math.max(1, Math.round(r.height * dpr()));
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
}

new ResizeObserver(() => {
  fit($('neurons'), true);
  fit(overlay, true);
  fit(mea, true);
  fit(raster, false);
  fit(rate, false);
}).observe(document.body);

let lastWall = performance.now();

function frame(nowWall) {
  const dt = Math.min(100, nowWall - lastWall);
  lastWall = nowWall;

  if (playing) {
    const want = dt * speed + carry;
    const cap = Math.max(1, Math.floor(FRAME_BUDGET / costPerMs));
    let ms = Math.floor(want);
    if (ms > cap) {
      ms = cap;
      carry = 0;
      slowFrames = Math.min(slowFrames + 1, 60);
    } else {
      carry = want - ms;
      slowFrames = Math.max(slowFrames - 1, 0);
    }
    if (ms > 0) {
      const t0 = performance.now();
      culture.step(ms);
      costPerMs = costPerMs * 0.9 + ((performance.now() - t0) / ms) * 0.1;
      record(ms);
    }
    $('slow').hidden = slowFrames < 30;
  }

  dish.draw(culture.glow(GLOW_TAU));
  drawOverlay(nowWall);
  drawMea(nowWall);
  drawRaster();
  drawRate();
  drawStats();
  requestAnimationFrame(frame);
}

// --- controls -----------------------------------------------------------------

function setPlaying(p) {
  playing = p;
  $('play').textContent = p ? 'Pause' : 'Play';
  $('play').setAttribute('aria-pressed', String(p));
}

$('play').addEventListener('click', () => setPlaying(!playing));
$('new').addEventListener('click', () => grow(randomSeed(), culture.n));
$('size').addEventListener('change', (ev) => grow(culture.seed, Number(ev.target.value)));
$('speed').addEventListener('change', (ev) => { speed = Number(ev.target.value); carry = 0; });
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
  const { seed, n } = readHash();
  if (seed !== culture.seed || n !== culture.n) grow(seed, n);
});

const start = readHash();
grow(start.seed, start.n);
requestAnimationFrame(frame);

// Draws a culture's neurons as glowing points: WebGL2 with additive blending,
// or a plain 2D fallback that only draws the neurons that are lit.

export const FIREFLY = [227, 249, 138];
export const ICE = [159, 231, 255];
export const CHAMPAGNE = [217, 199, 156];

/** The dish edge sits 4% outside the culture. */
export const MARGIN = 1.04;

export function createDish(canvas) {
  const gl = canvas.getContext('webgl2', { premultipliedAlpha: true, antialias: false });
  let n = 0;
  let radius = 1000;

  if (!gl) {
    const ctx = canvas.getContext('2d');
    let positions;
    let kinds;
    return {
      upload(c) { positions = c.positions; kinds = c.kinds; n = c.n; radius = c.radius; },
      draw(glow) {
        const { width: w } = canvas;
        ctx.clearRect(0, 0, w, w);
        const s = w / 2 / (radius * MARGIN);
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
      gl.uniform1f(u('u_unit'), 1 / (radius * MARGIN));
      // About 12 µm across at rest, at least 2 device pixels.
      gl.uniform1f(u('u_size'), Math.max(2, (canvas.width / (2 * radius * MARGIN)) * 12));
      gl.drawArrays(gl.POINTS, 0, n);
    },
  };
}

// Raw WebGL2: two instanced programs (points, segments).  One draw call per tile
// layer and primitive; colors, dash patterns and widths come from small uniform
// tables indexed by code, so a theme switch touches uniforms, never buffers.

export const POINT_FLOATS = 4; // x, y, radius px, code (+256 if cluster, +512 if dimmed)
export const EDGE_FLOATS = 6; // x0, y0, x1, y1, state code (+16 if dimmed), weight

const POINT_VS = `#version 300 es
precision highp float;
layout(location=0) in vec2 a_corner;
layout(location=1) in vec4 a_inst; // x, y, radius, code
uniform vec2 u_center;
uniform float u_scale;
uniform vec2 u_viewport;
uniform float u_radiusScale;
uniform vec3 u_palette[32];
out vec2 v_uv;
out float v_radius;
out vec3 v_color;
flat out int v_cluster;
flat out int v_dim;
void main() {
  int code = int(a_inst.w + 0.5);
  v_dim = (code >> 9) & 1;
  v_cluster = (code >> 8) & 1;
  int idx = v_cluster == 1 ? 31 : (code & 31);
  v_color = u_palette[idx];
  float r = a_inst.z * u_radiusScale + 1.0;
  v_radius = r;
  v_uv = a_corner * r;
  vec2 screen = (a_inst.xy - u_center) * u_scale + 0.5 * u_viewport + a_corner * r;
  gl_Position = vec4(screen / u_viewport * 2.0 - 1.0, 0.0, 1.0);
  gl_Position.y = -gl_Position.y;
}`;

const POINT_FS = `#version 300 es
precision highp float;
in vec2 v_uv;
in float v_radius;
in vec3 v_color;
flat in int v_cluster;
flat in int v_dim;
uniform float u_alpha;
uniform vec3 u_outline;
out vec4 o;
void main() {
  float d = length(v_uv);
  float edge = v_radius - 1.0;
  float aa = clamp(edge + 0.5 - d, 0.0, 1.0);
  if (aa <= 0.0) discard;
  float a = u_alpha * (v_dim == 1 ? 0.18 : 1.0);
  if (v_cluster == 1) {
    // Region: translucent disc with a solid ring, so it never reads as one entity.
    float ring = clamp(1.5 - abs(d - (edge - 1.5)), 0.0, 1.0);
    o = vec4(v_color, (0.16 + 0.75 * ring) * aa * a);
  } else {
    float rim = clamp(d - (edge - 1.2), 0.0, 1.0);
    o = vec4(mix(v_color, u_outline, rim * 0.6), aa * a);
  }
  o.rgb *= o.a;
}`;

const EDGE_VS = `#version 300 es
precision highp float;
layout(location=0) in vec2 a_corner; // x: 0..1 along, y: -1..1 across
layout(location=1) in vec4 a_ends;
layout(location=2) in vec2 a_style; // state code, weight
uniform vec2 u_center;
uniform float u_scale;
uniform vec2 u_viewport;
uniform vec3 u_stateColor[8];
uniform vec3 u_stateDash[8]; // period px, duty, width px
out float v_along;
out float v_across;
out float v_halfw;
out vec3 v_color;
out vec2 v_dash;
flat out int v_dim;
void main() {
  int code = int(a_style.x + 0.5);
  v_dim = (code >> 4) & 1;
  int st = code & 7;
  vec3 dash = u_stateDash[st];
  v_color = u_stateColor[st];
  v_dash = dash.xy;
  float w = dash.z * a_style.y;
  vec2 p0 = (a_ends.xy - u_center) * u_scale + 0.5 * u_viewport;
  vec2 p1 = (a_ends.zw - u_center) * u_scale + 0.5 * u_viewport;
  vec2 d = p1 - p0;
  float len = max(length(d), 1e-4);
  vec2 n = vec2(-d.y, d.x) / len;
  float hw = 0.5 * w + 1.0;
  v_halfw = 0.5 * w;
  v_along = a_corner.x * len;
  v_across = a_corner.y * hw;
  vec2 screen = p0 + d * a_corner.x + n * a_corner.y * hw;
  gl_Position = vec4(screen / u_viewport * 2.0 - 1.0, 0.0, 1.0);
  gl_Position.y = -gl_Position.y;
}`;

const EDGE_FS = `#version 300 es
precision highp float;
in float v_along;
in float v_across;
in float v_halfw;
in vec3 v_color;
in vec2 v_dash;
flat in int v_dim;
uniform float u_alpha;
uniform float u_edgeAlpha;
out vec4 o;
void main() {
  if (v_dash.x > 0.0 && fract(v_along / v_dash.x) > v_dash.y) discard;
  float aa = clamp(v_halfw + 0.5 - abs(v_across), 0.0, 1.0);
  float a = aa * u_alpha * u_edgeAlpha * (v_dim == 1 ? 0.15 : 1.0);
  if (a <= 0.0) discard;
  o = vec4(v_color * a, a);
}`;

function compile(gl: WebGL2RenderingContext, type: number, src: string): WebGLShader {
  const s = gl.createShader(type)!;
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(`shader: ${gl.getShaderInfoLog(s)}`);
  return s;
}

function program(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
  const p = gl.createProgram()!;
  gl.attachShader(p, compile(gl, gl.VERTEX_SHADER, vs));
  gl.attachShader(p, compile(gl, gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(`link: ${gl.getProgramInfoLog(p)}`);
  return p;
}

type Uniforms = Record<string, WebGLUniformLocation | null>;

function uniforms(gl: WebGL2RenderingContext, p: WebGLProgram, names: string[]): Uniforms {
  return Object.fromEntries(names.map((n) => [n, gl.getUniformLocation(p, n)]));
}

/** GPU buffers of one tile (or of the benchmark's synthetic layer). */
export interface Layer {
  pointVao: WebGLVertexArrayObject;
  pointBuf: WebGLBuffer;
  nPoints: number;
  edgeVao: WebGLVertexArrayObject;
  edgeBuf: WebGLBuffer;
  nEdges: number;
}

export interface Frame {
  cx: number;
  cy: number;
  scale: number;
  width: number; // CSS px
  height: number;
  dpr: number;
}

export interface Theme {
  palette: Float32Array; // 32 x rgb: entity types, index 31 = regions
  stateColor: Float32Array; // 8 x rgb
  stateDash: Float32Array; // 8 x (period, duty, width)
  outline: [number, number, number];
  background: [number, number, number];
}

export class Renderer {
  readonly gl: WebGL2RenderingContext;
  private readonly pointProg: WebGLProgram;
  private readonly edgeProg: WebGLProgram;
  private readonly pu: Uniforms;
  private readonly eu: Uniforms;
  private readonly quad: WebGLBuffer;
  private readonly strip: WebGLBuffer;
  private theme: Theme | null = null;

  constructor(canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl2", { antialias: false, alpha: false, premultipliedAlpha: true, powerPreference: "high-performance" });
    if (!gl) throw new Error("WebGL2 is not available in this browser");
    this.gl = gl;
    this.pointProg = program(gl, POINT_VS, POINT_FS);
    this.edgeProg = program(gl, EDGE_VS, EDGE_FS);
    this.pu = uniforms(gl, this.pointProg, ["u_center", "u_scale", "u_viewport", "u_radiusScale", "u_palette", "u_alpha", "u_outline"]);
    this.eu = uniforms(gl, this.edgeProg, ["u_center", "u_scale", "u_viewport", "u_stateColor", "u_stateDash", "u_alpha", "u_edgeAlpha"]);
    this.quad = gl.createBuffer()!;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.quad);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    this.strip = gl.createBuffer()!;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.strip);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([0, -1, 1, -1, 0, 1, 1, 1]), gl.STATIC_DRAW);
  }

  setTheme(theme: Theme): void {
    this.theme = theme;
  }

  /** Upload one layer.  `points`: POINT_FLOATS per point; `edges`: EDGE_FLOATS per edge. */
  createLayer(points: Float32Array, edges: Float32Array): Layer {
    const gl = this.gl;
    const mk = (corner: WebGLBuffer, data: Float32Array, attrs: [number, number][], stride: number) => {
      const vao = gl.createVertexArray()!;
      gl.bindVertexArray(vao);
      gl.bindBuffer(gl.ARRAY_BUFFER, corner);
      gl.enableVertexAttribArray(0);
      gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
      const buf = gl.createBuffer()!;
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
      let off = 0;
      for (const [loc, size] of attrs) {
        gl.enableVertexAttribArray(loc);
        gl.vertexAttribPointer(loc, size, gl.FLOAT, false, stride * 4, off * 4);
        gl.vertexAttribDivisor(loc, 1);
        off += size;
      }
      gl.bindVertexArray(null);
      return { vao, buf };
    };
    const p = mk(this.quad, points, [[1, 4]], POINT_FLOATS);
    const e = mk(this.strip, edges, [[1, 4], [2, 2]], EDGE_FLOATS);
    return {
      pointVao: p.vao, pointBuf: p.buf, nPoints: points.length / POINT_FLOATS,
      edgeVao: e.vao, edgeBuf: e.buf, nEdges: edges.length / EDGE_FLOATS,
    };
  }

  /** Replace a layer's point data (focus dimming) without reallocating the VAO. */
  updatePoints(layer: Layer, points: Float32Array): void {
    const gl = this.gl;
    gl.bindBuffer(gl.ARRAY_BUFFER, layer.pointBuf);
    gl.bufferData(gl.ARRAY_BUFFER, points, gl.STATIC_DRAW);
    layer.nPoints = points.length / POINT_FLOATS;
  }

  updateEdges(layer: Layer, edges: Float32Array): void {
    const gl = this.gl;
    gl.bindBuffer(gl.ARRAY_BUFFER, layer.edgeBuf);
    gl.bufferData(gl.ARRAY_BUFFER, edges, gl.STATIC_DRAW);
    layer.nEdges = edges.length / EDGE_FLOATS;
  }

  deleteLayer(layer: Layer): void {
    const gl = this.gl;
    gl.deleteVertexArray(layer.pointVao);
    gl.deleteVertexArray(layer.edgeVao);
    gl.deleteBuffer(layer.pointBuf);
    gl.deleteBuffer(layer.edgeBuf);
  }

  begin(f: Frame): void {
    const gl = this.gl;
    const w = Math.round(f.width * f.dpr), h = Math.round(f.height * f.dpr);
    if (gl.canvas.width !== w || gl.canvas.height !== h) {
      gl.canvas.width = w;
      gl.canvas.height = h;
    }
    gl.viewport(0, 0, w, h);
    const bg = this.theme?.background ?? [1, 1, 1];
    gl.clearColor(bg[0], bg[1], bg[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
  }

  /** Common uniforms in device pixels. */
  private setView(u: Uniforms, f: Frame): void {
    const gl = this.gl;
    gl.uniform2f(u.u_center, f.cx, f.cy);
    gl.uniform1f(u.u_scale, f.scale * f.dpr);
    gl.uniform2f(u.u_viewport, f.width * f.dpr, f.height * f.dpr);
  }

  drawEdges(layers: Iterable<[Layer, number]>, f: Frame, edgeAlpha: number): void {
    const gl = this.gl;
    const th = this.theme;
    if (!th) return;
    gl.useProgram(this.edgeProg);
    this.setView(this.eu, f);
    gl.uniform3fv(this.eu.u_stateColor, th.stateColor);
    const dash = th.stateDash.slice();
    for (let i = 0; i < 8; i++) {
      dash[i * 3] *= f.dpr;
      dash[i * 3 + 2] *= f.dpr;
    }
    gl.uniform3fv(this.eu.u_stateDash, dash);
    gl.uniform1f(this.eu.u_edgeAlpha, edgeAlpha);
    for (const [l, alpha] of layers) {
      if (l.nEdges === 0 || alpha <= 0) continue;
      gl.uniform1f(this.eu.u_alpha, alpha);
      gl.bindVertexArray(l.edgeVao);
      gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, l.nEdges);
    }
    gl.bindVertexArray(null);
  }

  drawPoints(layers: Iterable<[Layer, number]>, f: Frame): void {
    const gl = this.gl;
    const th = this.theme;
    if (!th) return;
    gl.useProgram(this.pointProg);
    this.setView(this.pu, f);
    gl.uniform1f(this.pu.u_radiusScale, f.dpr);
    gl.uniform3fv(this.pu.u_palette, th.palette);
    gl.uniform3f(this.pu.u_outline, ...th.outline);
    for (const [l, alpha] of layers) {
      if (l.nPoints === 0 || alpha <= 0) continue;
      gl.uniform1f(this.pu.u_alpha, alpha);
      gl.bindVertexArray(l.pointVao);
      gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, l.nPoints);
    }
    gl.bindVertexArray(null);
  }

  get renderer(): string {
    const gl = this.gl;
    const ext = gl.getExtension("WEBGL_debug_renderer_info");
    return ext ? String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL)) : String(gl.getParameter(gl.RENDERER));
  }
}

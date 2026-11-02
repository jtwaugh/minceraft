// GL half of the textured-model drawer. models.js packs the model faces of the
// tileset export into one atlas and builds box vertices; this uploads the atlas
// (nearest-filtered, on its own texture unit so the tile arrays on units 0 and 1
// stay bound) and draws models with a small textured-box shader. The shader is
// unshaded and unfogged: no fog uniforms are shared with the tile programs, and
// the `tint` colour is multiplied into the texture (hurt-red, later).

import { packAtlas, boxVertices } from './models.js';

export const MODEL_UNIT = 2;

/**
 * Merges boxes ({ lo, size, faces }, see models.js boxVertices) into one mesh:
 * { vertices: Float32Array (x y z u v), indices: Uint16Array }. No GL.
 */
export function mergeBoxes(boxes, uvOf) {
  const vertices = new Float32Array(boxes.length * 24 * 5);
  const indices = new Uint16Array(boxes.length * 36);
  boxes.forEach((box, n) => {
    const b = boxVertices(box, uvOf);
    vertices.set(b.vertices, n * 24 * 5);
    indices.set(b.indices.map((i) => i + n * 24), n * 36);
  });
  return { vertices, indices };
}

/** The tint a model is drawn with: [r, g, b] scaled by the frame's daylight (1 leaves it unchanged). */
export const daylitTint = (tint, daylight = 1) => tint.map((c) => c * daylight);

/**
 * Creates the drawer for the tileset export `exportJson` (may have no models;
 * every face is then grey). Returns { atlas, addModel(boxes) -> handle,
 * drawModel(mvp, modelMatrix, handle, tint, daylight) } where tint is [r, g, b], daylight
 * scales it (default 1), and the
 * matrices are column-major Float32Array(16)s.
 */
export function createModelDrawer(gl, exportJson, textureNames = []) {
  const atlas = packAtlas(exportJson, textureNames);
  const sh = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    return s;
  };
  const p = gl.createProgram();
  gl.attachShader(p, sh(gl.VERTEX_SHADER, `#version 300 es
    layout(location=0) in vec3 pos;
    layout(location=1) in vec2 uv;
    uniform mat4 mvp; uniform mat4 model;
    out vec2 vuv;
    void main() { vuv = uv; gl_Position = mvp * model * vec4(pos, 1.0); }`));
  gl.attachShader(p, sh(gl.FRAGMENT_SHADER, `#version 300 es
    precision mediump float;
    in vec2 vuv;
    uniform sampler2D atlas; uniform vec3 tint;
    out vec4 o;
    void main() {
      vec4 t = texture(atlas, vuv);
      if (t.a < 0.5) discard;
      o = vec4(t.rgb * tint, 1.0);
    }`));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  const loc = Object.fromEntries(['mvp', 'model', 'atlas', 'tint'].map((n) => [n, gl.getUniformLocation(p, n)]));

  const tex = gl.createTexture();
  gl.activeTexture(gl.TEXTURE0 + MODEL_UNIT);
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, atlas.width, atlas.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, atlas.data);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.activeTexture(gl.TEXTURE0);

  function addModel(boxes) {
    const { vertices, indices } = mergeBoxes(boxes, atlas.uvOf);
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
    gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 20, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 2, gl.FLOAT, false, 20, 12);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, gl.createBuffer());
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, indices, gl.STATIC_DRAW);
    gl.bindVertexArray(null);
    return { vao, count: indices.length };
  }

  function drawModel(mvp, modelMatrix, handle, tint = [1, 1, 1], daylight = 1) {
    gl.useProgram(p);
    gl.activeTexture(gl.TEXTURE0 + MODEL_UNIT);
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.activeTexture(gl.TEXTURE0);
    gl.uniform1i(loc.atlas, MODEL_UNIT);
    gl.uniformMatrix4fv(loc.mvp, false, mvp);
    gl.uniformMatrix4fv(loc.model, false, modelMatrix);
    gl.uniform3fv(loc.tint, daylitTint(tint, daylight));
    gl.bindVertexArray(handle.vao);
    gl.drawElements(gl.TRIANGLES, handle.count, gl.UNSIGNED_SHORT, 0);
    gl.bindVertexArray(null);
  }

  return { atlas, addModel, drawModel };
}

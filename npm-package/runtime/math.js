/**
 * Elysium Math — extended math operations.
 *
 * Backend (compiled binary): uses C math.h when possible, stubs otherwise.
 * Client-side / Node: wraps native Math.* and custom vector operations.
 *
 * Usage:
 *   const math = require('elysium-lang/runtime/math');
 *   math.sqrt(9);       // "3"
 *   math.pow(2, 3);     // "8"
 *   math.dot("[1,2]", "[3,4]");  // "11"
 *   math.cosineSimilarity("[1,0,0]", "[0,1,0]");  // "0"
 */

// --- Scalar Math ---

function sqrt(x) { return String(Math.sqrt(Number(x))); }
function pow(x, y) { return String(Math.pow(Number(x), Number(y))); }
function abs(x) { return String(Math.abs(Number(x))); }
function floor(x) { return String(Math.floor(Number(x))); }
function ceil(x) { return String(Math.ceil(Number(x))); }
function round(x) { return String(Math.round(Number(x))); }
function sin(x) { return String(Math.sin(Number(x))); }
function cos(x) { return String(Math.cos(Number(x))); }
function tan(x) { return String(Math.tan(Number(x))); }
function log(x) { return String(Math.log(Number(x))); }
function log2(x) { return String(Math.log2(Number(x))); }
function log10(x) { return String(Math.log10(Number(x))); }
function exp(x) { return String(Math.exp(Number(x))); }
function max(x, y) { return String(Math.max(Number(x), Number(y))); }
function min(x, y) { return String(Math.min(Number(x), Number(y))); }

// --- Array / Vector Math ---
// All array params are JSON array strings: "[1.0, 2.0, 3.0]"

function parseVec(s) {
  try { return JSON.parse(String(s)); } catch (_) { return []; }
}

/**
 * Sum of array elements.
 * math.sum(arr) → String (Float as string)
 * arr: JSON array string, e.g., "[1,2,3]"
 */
function sum(arr) {
  const vec = parseVec(arr);
  const s = vec.reduce((a, b) => a + (Number(b) || 0), 0);
  return String(s);
}

/**
 * Mean of array elements.
 * math.mean(arr) → String
 */
function mean(arr) {
  const vec = parseVec(arr);
  if (vec.length === 0) return '0';
  const s = vec.reduce((a, b) => a + (Number(b) || 0), 0);
  return String(s / vec.length);
}

/**
 * Dot product of two vectors.
 * math.dot(a, b) → String
 */
function dot(a, b) {
  const va = parseVec(a);
  const vb = parseVec(b);
  const len = Math.min(va.length, vb.length);
  let sum = 0;
  for (let i = 0; i < len; i++) {
    sum += (Number(va[i]) || 0) * (Number(vb[i]) || 0);
  }
  return String(sum);
}

/**
 * Cosine similarity between two vectors. Returns -1 to 1 as string.
 * math.cosineSimilarity(a, b) → String
 */
function cosineSimilarity(a, b) {
  const va = parseVec(a);
  const vb = parseVec(b);
  const len = Math.min(va.length, vb.length);
  let dotProd = 0, normA = 0, normB = 0;
  for (let i = 0; i < len; i++) {
    const ai = Number(va[i]) || 0;
    const bi = Number(vb[i]) || 0;
    dotProd += ai * bi;
    normA += ai * ai;
    normB += bi * bi;
  }
  if (normA === 0 || normB === 0) return '0';
  return String(dotProd / (Math.sqrt(normA) * Math.sqrt(normB)));
}

/**
 * Euclidean distance between two vectors.
 * math.euclidean(a, b) → String
 */
function euclidean(a, b) {
  const va = parseVec(a);
  const vb = parseVec(b);
  const len = Math.min(va.length, vb.length);
  let sum = 0;
  for (let i = 0; i < len; i++) {
    const diff = (Number(va[i]) || 0) - (Number(vb[i]) || 0);
    sum += diff * diff;
  }
  return String(Math.sqrt(sum));
}

module.exports = {
  sqrt, pow, abs, floor, ceil, round,
  sin, cos, tan,
  log, log2, log10, exp,
  max, min,
  sum, mean,
  dot, cosineSimilarity, euclidean,
};

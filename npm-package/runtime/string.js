/**
 * Elysium String — string utilities that mirror the compiler's __string_* builtins.
 *
 * Backend (compiled binary): uses C strlen for length/isEmpty; stubs for the rest.
 * Client-side / Node: maps to native JavaScript String.prototype methods.
 *
 * Usage:
 *   const str = require('elysium-lang/runtime/string');
 *   str.length('hello');      // 5
 *   str.toUpper('hello');     // 'HELLO'
 *   str.isEmpty('');          // true
 */

// String → Int
function length(s) { return typeof s === 'string' ? s.length : 0; }
function charCodeAt(s, idx) { return typeof s === 'string' ? s.charCodeAt(idx) : NaN; }
function indexOf(s, search) { return typeof s === 'string' ? s.indexOf(search) : -1; }
function lastIndexOf(s, search) { return typeof s === 'string' ? s.lastIndexOf(search) : -1; }
function search(s, pattern) { return typeof s === 'string' ? s.search(pattern) : -1; }

// String → Bool
function isEmpty(s) { return typeof s === 'string' ? s.length === 0 : true; }
function startsWith(s, prefix) { return typeof s === 'string' ? s.startsWith(prefix) : false; }
function endsWith(s, suffix) { return typeof s === 'string' ? s.endsWith(suffix) : false; }
function contains(s, sub) { return typeof s === 'string' ? s.includes(sub) : false; }
function includes(s, sub) { return typeof s === 'string' ? s.includes(sub) : false; }

// String → String
function toUpper(s) { return typeof s === 'string' ? s.toUpperCase() : ''; }
function toLower(s) { return typeof s === 'string' ? s.toLowerCase() : ''; }
function trim(s) { return typeof s === 'string' ? s.trim() : ''; }
function trimStart(s) { return typeof s === 'string' ? s.trimStart() : ''; }
function trimEnd(s) { return typeof s === 'string' ? s.trimEnd() : ''; }
function toString(s) { return s == null ? '' : String(s); }
function charAt(s, idx) { return typeof s === 'string' ? s.charAt(idx) : ''; }
function slice(s, start, end) { return typeof s === 'string' ? s.slice(start, end) : ''; }
function substring(s, start, end) { return typeof s === 'string' ? s.substring(start, end) : ''; }
function replace(s, search, replacement) { return typeof s === 'string' ? s.replace(search, replacement) : ''; }
function concat(s, other) { return typeof s === 'string' ? s.concat(other) : ''; }
function padStart(s, len, pad) { return typeof s === 'string' ? s.padStart(len, pad || ' ') : ''; }
function padEnd(s, len, pad) { return typeof s === 'string' ? s.padEnd(len, pad || ' ') : ''; }
function repeat(s, count) { return typeof s === 'string' ? s.repeat(count) : ''; }
function split(s, separator) { return typeof s === 'string' ? s.split(separator) : []; }
function match(s, pattern) { return typeof s === 'string' ? s.match(pattern) : null; }

// Crypto: (String) → String — hash, encode, HMAC
function sha256(s) { return typeof s !== 'string' ? '' : (typeof require === 'function' ? (() => { try { const c = require('crypto'); return c.createHash('sha256').update(s).digest('hex'); } catch(_) { return ''; } })() : ''); }
function md5(s) { return typeof s !== 'string' ? '' : (typeof require === 'function' ? (() => { try { const c = require('crypto'); return c.createHash('md5').update(s).digest('hex'); } catch(_) { return ''; } })() : ''); }
function base64Encode(s) { return typeof s !== 'string' ? '' : (typeof Buffer !== 'undefined' ? Buffer.from(s).toString('base64') : (typeof btoa !== 'undefined' ? btoa(s) : '')); }
function base64Decode(s) { return typeof s !== 'string' ? '' : (typeof Buffer !== 'undefined' ? Buffer.from(s, 'base64').toString('utf8') : (typeof atob !== 'undefined' ? atob(s) : '')); }
function hexEncode(s) { return typeof s !== 'string' ? '' : (typeof Buffer !== 'undefined' ? Buffer.from(s).toString('hex') : (() => { let h = ''; for (let i = 0; i < s.length; i++) { const c = s.charCodeAt(i).toString(16); h += c.length === 1 ? '0' + c : c; } return h; })()); }
function hexDecode(s) { return typeof s !== 'string' ? '' : (typeof Buffer !== 'undefined' ? Buffer.from(s, 'hex').toString('utf8') : (() => { let r = ''; for (let i = 0; i < s.length; i += 2) r += String.fromCharCode(parseInt(s.substr(i, 2), 16)); return r; })()); }
function hmac(s, key) { return typeof s !== 'string' || typeof key !== 'string' ? '' : (typeof require === 'function' ? (() => { try { const c = require('crypto'); return c.createHmac('sha256', key).update(s).digest('hex'); } catch(_) { return ''; } })() : ''); }

// () → String — generate a UUID v4
function uuid() {
  // Modern browsers and Node 19+ have crypto.randomUUID
  if (typeof globalThis !== 'undefined' && globalThis.crypto && typeof globalThis.crypto.randomUUID === 'function') {
    return globalThis.crypto.randomUUID();
  }
  // Node 15+ has crypto.randomUUID via require
  try {
    const nodeCrypto = require('crypto');
    if (typeof nodeCrypto.randomUUID === 'function') {
      return nodeCrypto.randomUUID();
    }
  } catch (_) {}
  // Fallback: manual v4 UUID generation
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
    const r = Math.random() * 16 | 0;
    const v = c === 'x' ? r : (r & 0x3 | 0x8);
    return v.toString(16);
  });
}

module.exports = {
  length,
  charCodeAt,
  indexOf,
  lastIndexOf,
  search,
  isEmpty,
  startsWith,
  endsWith,
  contains,
  includes,
  toUpper,
  toLower,
  trim,
  trimStart,
  trimEnd,
  toString,
  charAt,
  slice,
  substring,
  replace,
  concat,
  padStart,
  padEnd,
  repeat,
  split,
  match,
  sha256,
  md5,
  base64Encode,
  base64Decode,
  hexEncode,
  hexDecode,
  hmac,
  uuid,
};

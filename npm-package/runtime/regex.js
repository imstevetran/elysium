/**
 * Elysium Regex — compiled regex pattern matching for Elysium.
 *
 * Backend (compiled binary): emits a stub — regex is JS-runtime-only.
 * Client-side / Node: wraps native JavaScript RegExp.
 *
 * Usage:
 *   const regex = require('elysium-lang/runtime/regex');
 *   regex.test('\\d+', 'abc123');   // true
 *   regex.match('\\d+', 'abc123');  // "123"
 *   regex.replace('foo', 'hello foo world', 'bar');  // "hello bar world"
 *   regex.split(',', 'a,b,c');      // ['a', 'b', 'c']
 *   regex.search('\\d+', 'abc123'); // 3
 */

function _compile(pattern) {
  return new RegExp(pattern);
}

/**
 * Test if a pattern matches anywhere in the string.
 * regex.test(pattern, str) → Bool
 */
function test(pattern, str) {
  if (typeof pattern !== 'string' || typeof str !== 'string') return false;
  try {
    return _compile(pattern).test(str);
  } catch (_) {
    return false;
  }
}

/**
 * Returns the first match as a string, or empty string.
 * regex.match(pattern, str) → String
 */
function match(pattern, str) {
  if (typeof pattern !== 'string' || typeof str !== 'string') return '';
  try {
    const m = str.match(_compile(pattern));
    return m ? m[0] : '';
  } catch (_) {
    return '';
  }
}

/**
 * Returns the index of the first match, or -1.
 * regex.search(pattern, str) → Int
 */
function search(pattern, str) {
  if (typeof pattern !== 'string' || typeof str !== 'string') return -1;
  try {
    return str.search(_compile(pattern));
  } catch (_) {
    return -1;
  }
}

/**
 * Replace first occurrence of pattern with replacement.
 * regex.replace(pattern, str, replacement) → String
 */
function replace(pattern, str, replacement) {
  if (typeof pattern !== 'string' || typeof str !== 'string') return str || '';
  try {
    return str.replace(_compile(pattern), replacement != null ? String(replacement) : '');
  } catch (_) {
    return str;
  }
}

/**
 * Split string by pattern.
 * regex.split(pattern, str) → Array (returns as comma-joined string for now)
 */
function split(pattern, str) {
  if (typeof pattern !== 'string' || typeof str !== 'string') return '';
  try {
    return str.split(_compile(pattern));
  } catch (_) {
    return [str];
  }
}

module.exports = {
  test,
  match,
  search,
  replace,
  split,
};

/**
 * Elysium Env — environment variable access.
 *
 * Backend (compiled binary): emits a stub.
 * Client-side / Node: wraps process.env.
 *
 * Usage:
 *   const env = require('elysium-lang/runtime/env');
 *   env.get("OPENAI_API_KEY");   // "sk-..." or ""
 *   env.set("MY_VAR", "value");
 */

/**
 * Get an environment variable value. Returns empty string if not set.
 * env.get(key) → String
 */
function get(key) {
  if (typeof process !== 'undefined' && process.env) {
    const val = process.env[String(key)];
    return val !== undefined ? String(val) : '';
  }
  return '';
}

/**
 * Set an environment variable.
 * env.set(key, value) → String (returns key for chaining)
 */
function set(key, value) {
  if (typeof process !== 'undefined' && process.env) {
    try {
      process.env[String(key)] = String(value);
    } catch (_) {
      // Some environments don't allow modifying process.env
    }
  }
  return String(key);
}

module.exports = { get, set };

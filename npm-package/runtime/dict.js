/**
 * Elysium Dict — mutable key-value dictionary (string handles).
 *
 * Backend (compiled binary): emits a stub — dict requires JS runtime.
 * Client-side / Node: manages dictionaries in memory using string handles.
 *
 * Usage:
 *   const dict = require('elysium-lang/runtime/dict');
 *   let d = dict.create();            // "dict_1"
 *   dict.set(d, "name", "Alice");
 *   dict.get(d, "name");              // "Alice"
 *   dict.has(d, "name");              // "true"
 *   dict.keys(d);                     // '["name"]'
 *   dict.length(d);                   // "1"
 *   dict.delete(d, "name");
 *   dict.clear(d);
 */

let counter = 0;
const stores = {};

function nextId() {
  counter++;
  return `dict_${counter}`;
}

/**
 * Create a new mutable dictionary. Returns a string handle.
 * dict.create() → String
 */
function create() {
  const id = nextId();
  stores[id] = {};
  return id;
}

/**
 * Set a key-value pair in the dictionary.
 * dict.set(handle, key, value) → handle (for chaining)
 */
function set(handle, key, value) {
  const store = stores[handle];
  if (!store) return `error: dict ${handle} not found`;
  store[String(key)] = String(value);
  return handle;
}

/**
 * Get a value by key from the dictionary. Returns empty string if missing.
 * dict.get(handle, key) → String
 */
function get(handle, key) {
  const store = stores[handle];
  if (!store) return '';
  const val = store[String(key)];
  return val !== undefined ? String(val) : '';
}

/**
 * Check if a key exists in the dictionary.
 * dict.has(handle, key) → String ("true" or "false")
 */
function has(handle, key) {
  const store = stores[handle];
  if (!store) return 'false';
  return store.hasOwnProperty(String(key)) ? 'true' : 'false';
}

/**
 * Delete a key from the dictionary.
 * dict.delete(handle, key) → handle
 */
function deleteKey(handle, key) {
  const store = stores[handle];
  if (!store) return `error: dict ${handle} not found`;
  delete store[String(key)];
  return handle;
}

/**
 * Return all keys as a JSON array string.
 * dict.keys(handle) → String (JSON array)
 */
function keys(handle) {
  const store = stores[handle];
  if (!store) return '[]';
  return JSON.stringify(Object.keys(store));
}

/**
 * Return the number of key-value pairs.
 * dict.length(handle) → String (number as string for Elysium)
 */
function length(handle) {
  const store = stores[handle];
  if (!store) return '0';
  return String(Object.keys(store).length);
}

/**
 * Clear all key-value pairs.
 * dict.clear(handle) → handle
 */
function clear(handle) {
  const store = stores[handle];
  if (!store) return `error: dict ${handle} not found`;
  stores[handle] = {};
  return handle;
}

module.exports = {
  create,
  set,
  get,
  has,
  delete: deleteKey,
  keys,
  length,
  clear,
};

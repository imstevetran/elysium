/**
 * Elysium JSON — JSON parsing and serialization.
 *
 * Backend (compiled binary): emits a stub — JSON requires JS runtime.
 * Client-side / Node: wraps native JSON.parse/JSON.stringify.
 *
 * For structured access, parsed values are stored in a dict-like registry
 * and accessed via string handles.
 *
 * Usage:
 *   const json = require('elysium-lang/runtime/json');
 *   let h = json.parse('{"name":"Alice","age":30}'); // "json_1"
 *   json.get(h, "name");         // "Alice"
 *   json.stringify(h);           // '{"name":"Alice","age":30}'
 *   json.free(h);
 */

let counter = 0;
const parsed = {};

function nextId() {
  counter++;
  return `json_${counter}`;
}

/**
 * Parse a JSON string into a stored value. Returns a string handle.
 * For simple values (string, number, bool, null), the handle points to
 * a wrapper { __type: 'literal', value: <parsed> }.
 * For objects/arrays, the handle points to the parsed structure.
 *
 * json.parse(str) → String (handle)
 */
function parse(str) {
  const id = nextId();
  try {
    const val = JSON.parse(String(str));
    parsed[id] = val;
  } catch (_) {
    parsed[id] = { __error: 'json parse error', __raw: String(str) };
  }
  return id;
}

/**
 * Parse a JSON string for inline use (no handle). Returns the JSON
 * value directly as a string. For extracting top-level values from
 * simple JSON:
 *   json.parseInline('"hello"')  → "hello"
 *   json.parseInline('42')        → "42"
 *   json.parseInline('true')      → "true"
 *   json.parseInline('[1,2,3]')   → "[1,2,3]" (passthrough for arrays/objects)
 *
 * json.parseInline(str) → String
 */
function parseInline(str) {
  try {
    const val = JSON.parse(String(str));
    if (val === null) return '';
    if (typeof val === 'object') {
      // Arrays and objects: return as handle for get/stringify
      const id = nextId();
      parsed[id] = val;
      return id;
    }
    return String(val);
  } catch (_) {
    return String(str);
  }
}

/**
 * Get a value from a parsed JSON structure by key path.
 * Supports:
 *   json.get(handle, "name")                  → top-level key
 *   json.get(handle, "address.city")          → nested with dot notation
 *   json.get(handle, "choices.0.message.content") → array index with dot notation
 *
 * Returns the JSON string representation of the value.
 * For simple values (string, number, bool), returns the JS string representation.
 * For objects/arrays, returns the JSON string.
 *
 * json.get(handle, path) → String
 */
function get(handle, path) {
  const val = parsed[handle];
  if (val === undefined) return '';

  const keys = String(path).split('.');
  let current = val;

  for (const key of keys) {
    if (current === null || current === undefined) return '';
    if (Array.isArray(current)) {
      const idx = parseInt(key, 10);
      if (isNaN(idx) || idx < 0 || idx >= current.length) return '';
      current = current[idx];
    } else if (typeof current === 'object') {
      if (!current.hasOwnProperty(key)) return '';
      current = current[key];
    } else {
      return '';
    }
  }

  if (current === null || current === undefined) return '';
  if (typeof current === 'object') {
    return JSON.stringify(current);
  }
  return String(current);
}

/**
 * Serialize a stored JSON value back to a JSON string.
 * json.stringify(handle) → String
 */
function stringify(handle) {
  const val = parsed[handle];
  if (val === undefined) return '{}';
  try {
    return JSON.stringify(val);
  } catch (_) {
    return '{}';
  }
}

/**
 * Free a stored JSON value by handle.
 * json.free(handle) → String (handle for chaining, or error)
 */
function free(handle) {
  delete parsed[handle];
  return handle;
}

/**
 * Build a JSON object string from a sequence of key-value pairs.
 * Keys and values are provided as strings.
 * Values that are already valid JSON (arrays, objects, numbers, booleans)
 * are passed through as-is.
 *
 * json.buildObject("name", "Alice", "age", "30") → '{"name":"Alice","age":30}'
 * json.buildObject("role", "user", "content", "Hello") → '{"role":"user","content":"Hello"}'
 */
function buildObject() {
  const args = Array.from(arguments);
  const obj = {};
  for (let i = 0; i < args.length; i += 2) {
    if (i + 1 >= args.length) break;
    const key = String(args[i]);
    let val = String(args[i + 1]);
    // Check if the value is a JSON-structured string (starts with {, [, or is JSON literal)
    const trimmed = val.trim();
    if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
      try { obj[key] = JSON.parse(trimmed); } catch (_) { obj[key] = val; }
    } else if (trimmed === 'true' || trimmed === 'false') {
      obj[key] = trimmed === 'true';
    } else if (!isNaN(Number(trimmed)) && trimmed !== '') {
      obj[key] = Number(trimmed);
    } else {
      obj[key] = val;
    }
  }
  return JSON.stringify(obj);
}

/**
 * Build an OpenAPI-compatible chat message object string.
 *
 * json.buildMessage(role, content) → '{"role":"user","content":"Hello"}'
 */
function buildMessage(role, content) {
  return JSON.stringify({ role: String(role), content: String(content) });
}

/**
 * Build a JSON array as a string.
 * Supports nested JSON strings (objects/arrays) parsed automatically.
 * json.buildArray("a", "b", "c") → '["a","b","c"]'
 */
function buildArray() {
  const items = Array.from(arguments).map(a => {
    const val = String(a);
    const trimmed = val.trim();
    if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
      try { return JSON.parse(trimmed); } catch (_) { return val; }
    } else if (trimmed === 'true' || trimmed === 'false') {
      return trimmed === 'true';
    } else if (!isNaN(Number(trimmed)) && trimmed !== '') {
      return Number(trimmed);
    }
    return val;
  });
  return JSON.stringify(items);
}

module.exports = {
  parse,
  parseInline,
  get,
  stringify,
  free,
  buildObject,
  buildMessage,
  buildArray,
};

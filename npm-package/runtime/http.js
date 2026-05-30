/**
 * Elysium HTTP — HTTP client with custom headers.
 *
 * Backend (compiled binary): emits a stub — HTTP requires JS runtime.
 * Client-side / Node: uses native fetch.
 *
 * Usage:
 *   const http = require('elysium-lang/runtime/http');
 *   let resp = http.request("POST", "https://api.example.com/data",
 *     '{"Content-Type":"application/json","Authorization":"Bearer key"}',
 *     '{"key":"value"}');
 *   // resp = '{"status":200,"body":"{\\"result\\":\\"ok\\"}","headers":{...}}'
 */

// Detect environment
const isNode = typeof process !== 'undefined' && process.versions && process.versions.node;
const isBrowser = typeof window !== 'undefined' && typeof window.document !== 'undefined';

function getFetch() {
  if (isBrowser) return window.fetch.bind(window);
  if (isNode) {
    try { return require('node-fetch'); } catch (_) { return globalThis.fetch; }
  }
  return globalThis.fetch;
}

function parseHeaders(headersStr) {
  try { return JSON.parse(String(headersStr)); } catch (_) { return {}; }
}

function parseBody(bodyStr) {
  if (!bodyStr) return null;
  // If it's a dict handle (e.g., "json_1"), try to read it as a literal JSON
  // Otherwise send it as a string body
  return String(bodyStr);
}

/**
 * Make an HTTP request with full control over method, URL, headers, and body.
 *
 * @param {string} method - HTTP method: GET, POST, PUT, DELETE, etc.
 * @param {string} url - Request URL
 * @param {string} headersJson - JSON string of headers object
 * @param {string} body - Request body (string)
 * @returns {Promise<string>} JSON response: {"status":200,"body":"...","headers":{}}
 *
 * http.request(method, url, headersJson, body) → String
 */
async function request(method, url, headersJson, body) {
  const fetchImpl = getFetch();
  if (!fetchImpl) {
    return JSON.stringify({ status: 0, body: '[http] no fetch available', headers: {} });
  }

  const headers = parseHeaders(headersJson);
  const fetchOpts = {
    method: String(method).toUpperCase(),
    headers,
  };

  if (body && fetchOpts.method !== 'GET' && fetchOpts.method !== 'HEAD') {
    fetchOpts.body = String(body);
  }

  try {
    const response = await fetchImpl(String(url), fetchOpts);
    const contentType = response.headers.get('content-type') || '';
    let responseBody;
    if (contentType.includes('application/json')) {
      responseBody = await response.text();
    } else {
      responseBody = await response.text();
    }

    // Collect response headers
    const respHeaders = {};
    response.headers.forEach((value, key) => {
      respHeaders[key] = value;
    });

    return JSON.stringify({
      status: response.status,
      body: responseBody,
      headers: respHeaders,
    });
  } catch (err) {
    return JSON.stringify({
      status: 0,
      body: `[http error] ${err.message}`,
      headers: {},
    });
  }
}

/**
 * Synchronous stub version (for when async isn't available).
 * Returns a mock response.
 */
function requestSync(method, url, headersJson, body) {
  return JSON.stringify({
    status: 0,
    body: '[http] requestSync: use JS runtime with async support',
    headers: {},
  });
}

module.exports = { request, requestSync };

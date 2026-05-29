/**
 * Elysium Transport — networking utilities for HTTP, WebSocket, and MQTT.
 *
 * Backend (compiled binary): prints a stub message — real transport relies on JS runtime.
 * Client-side / Node: maps to native fetch, WebSocket, and MQTT.js.
 *
 * Usage:
 *   const transport = require('elysium-lang/runtime/transport');
 *   const data = transport.get('https://api.example.com/data');
 *   const ws = transport.wsConnect('wss://echo.example.com');
 *   transport.wsSend(ws, 'hello');
 *   transport.wsClose(ws);
 *   const client = transport.mqttConnect('mqtt://broker.example.com', 'clientId');
 *   transport.mqttPublish(client, 'topic/hello', 'payload');
 *   transport.mqttSubscribe(client, 'topic/#');
 *   transport.mqttDisconnect(client);
 */

// Detect environment
const isNode = typeof process !== 'undefined' && process.versions && process.versions.node;
const isBrowser = typeof window !== 'undefined' && typeof window.document !== 'undefined';

// --- HTTP utilities (axios-like) ---

const httpDefaults = {
  headers: { 'Content-Type': 'application/json' },
  timeout: 30000,
};

/**
 * Make an HTTP request using fetch or a polyfill.
 * Returns parsed JSON or raw text.
 */
async function httpRequest(method, url, body, options) {
  const opts = Object.assign({}, httpDefaults, options || {});
  const fetchOpts = {
    method,
    headers: opts.headers,
  };
  if (body && method !== 'GET' && method !== 'HEAD') {
    fetchOpts.body = typeof body === 'string' ? body : JSON.stringify(body);
  }
  if (opts.timeout) {
    const controller = typeof AbortController !== 'undefined' ? new AbortController() : null;
    if (controller) {
      fetchOpts.signal = controller.signal;
      setTimeout(() => controller.abort(), opts.timeout);
    }
  }

  let fetchImpl;
  if (isBrowser) {
    fetchImpl = window.fetch.bind(window);
  } else if (isNode) {
    // Use native fetch in Node 18+, or node-fetch
    try {
      fetchImpl = require('node-fetch');
    } catch (_) {
      fetchImpl = globalThis.fetch;
    }
  } else {
    fetchImpl = globalThis.fetch;
  }

  if (!fetchImpl) {
    throw new Error('No fetch implementation available. Install node-fetch for Node.js < 18.');
  }

  const response = await fetchImpl(url, fetchOpts);
  const contentType = response.headers.get('content-type') || '';
  if (contentType.includes('application/json')) {
    return response.json();
  }
  return response.text();
}

const http = {
  get(url, options) {
    return httpRequest('GET', url, null, options);
  },
  post(url, body, options) {
    return httpRequest('POST', url, body, options);
  },
  put(url, body, options) {
    return httpRequest('PUT', url, body, options);
  },
  delete(url, options) {
    return httpRequest('DELETE', url, null, options);
  },
};

// --- WebSocket utilities ---

class WsConnection {
  constructor(ws) {
    this._ws = ws;
    this._listeners = {};
  }

  on(event, cb) {
    this._listeners[event] = cb;
    if (event === 'message' && this._ws) {
      this._ws.onmessage = (msg) => cb(msg.data);
    }
    if (event === 'open' && this._ws) {
      this._ws.onopen = () => cb();
    }
    if (event === 'close' && this._ws) {
      this._ws.onclose = () => cb();
    }
    if (event === 'error' && this._ws) {
      this._ws.onerror = (err) => cb(err);
    }
  }

  send(data) {
    if (this._ws && this._ws.readyState === 1) {
      this._ws.send(typeof data === 'string' ? data : JSON.stringify(data));
    }
  }

  close() {
    if (this._ws) {
      this._ws.close();
    }
  }
}

const ws = {
  connect(url) {
    let WebSocketImpl;
    if (isBrowser) {
      WebSocketImpl = window.WebSocket;
    } else if (isNode) {
      try {
        WebSocketImpl = require('ws');
      } catch (_) {
        WebSocketImpl = globalThis.WebSocket;
      }
    } else {
      WebSocketImpl = globalThis.WebSocket;
    }
    if (!WebSocketImpl) {
      throw new Error('No WebSocket implementation available. Install the "ws" package for Node.js.');
    }
    const nativeWs = new WebSocketImpl(url);
    return new WsConnection(nativeWs);
  },
  send(connection, data) {
    connection.send(data);
  },
  close(connection) {
    connection.close();
  },
};

// --- MQTT utilities ---

const mqtt = {
  connect(brokerUrl, clientId) {
    let mqttImpl;
    try {
      mqttImpl = require('mqtt');
    } catch (_) {
      throw new Error('MQTT requires the "mqtt" package. Install with: npm install mqtt');
    }
    const client = mqttImpl.connect(brokerUrl, {
      clientId: clientId || 'elysium_' + Math.random().toString(36).slice(2, 10),
      clean: true,
    });
    return client;
  },
  publish(client, topic, message) {
    client.publish(topic, typeof message === 'string' ? message : JSON.stringify(message));
  },
  subscribe(client, topic) {
    client.subscribe(topic);
  },
  disconnect(client) {
    client.end(true);
  },
};

// --- Utility: status codes ---

const status = {
  OK: 200,
  CREATED: 201,
  ACCEPTED: 202,
  NO_CONTENT: 204,
  MOVED_PERMANENTLY: 301,
  FOUND: 302,
  BAD_REQUEST: 400,
  UNAUTHORIZED: 401,
  FORBIDDEN: 403,
  NOT_FOUND: 404,
  CONFLICT: 409,
  INTERNAL_SERVER_ERROR: 500,
  BAD_GATEWAY: 502,
  SERVICE_UNAVAILABLE: 503,
};

module.exports = {
  // HTTP
  get: http.get,
  post: http.post,
  put: http.put,
  delete: http.delete,

  // WebSocket
  wsConnect: ws.connect,
  wsSend: ws.send,
  wsClose: ws.close,

  // MQTT
  mqttConnect: mqtt.connect,
  mqttPublish: mqtt.publish,
  mqttSubscribe: mqtt.subscribe,
  mqttDisconnect: mqtt.disconnect,

  // Status codes
  status,
};

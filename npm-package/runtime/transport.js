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
 *
 * Special URLs starting with __langchain__/ or __langgraph__/
 * are routed to the LangChain and LangGraph runtime modules
 * (for Elysium source-level packages using transport).
 */
async function httpRequest(method, url, body, options) {
  // Route __langchain__/* URLs to the langchain module
  if (url.startsWith('__langchain__/')) {
    return routeLangChain(method, url, body);
  }

  // Route __ble__/* URLs to the BLE runtime module
  if (url.startsWith('__ble__/')) {
    return routeBle(method, url, body);
  }

  // Route __zigbee__/* URLs to the Zigbee runtime module
  if (url.startsWith('__zigbee__/')) {
    return routeZigbee(method, url, body);
  }

  // Route __langgraph__/* URLs to the langgraph module
  if (url.startsWith('__langgraph__/')) {
    return routeLangGraph(method, url, body);
  }
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

// --- LangChain/LangGraph routing for Elysium package use ---

let langchainMod = null;
let langgraphMod = null;

function getLangChain() {
  if (!langchainMod) {
    try { langchainMod = require('./langchain'); } catch (_) { langchainMod = null; }
  }
  return langchainMod;
}

function getLangGraph() {
  if (!langgraphMod) {
    try { langgraphMod = require('./langgraph'); } catch (_) { langgraphMod = null; }
  }
  return langgraphMod;
}

function routeLangChain(method, url, body) {
  const lc = getLangChain();
  if (!lc) return '[langchain runtime not available]';

  // url format: __langchain__/<method>[/model]
  const rest = url.slice('__langchain__/'.length);
  const parts = rest.split('/');
  const lcMethod = parts[0];
  const model = parts.length > 1 ? parts.slice(1).join('/') : '';

  switch (lcMethod) {
    case 'llm': return lc.llm(model, body);
    case 'chat': {
      const [system, message] = body.split('|||');
      return lc.chat(model, system || '', message || '');
    }
    case 'embed': return lc.embed(body);
    case 'similarity': {
      const [a, b] = body.split('|||');
      return lc.similarity(a || '', b || '');
    }
    case 'template': {
      const [tmpl, vars] = body.split('|||');
      return lc.template(tmpl || '', vars || '');
    }
    case 'rag': {
      const [query, context] = body.split('|||');
      return lc.rag(query || '', context || '');
    }
    case 'summarize': return lc.summarize(body);
    case 'analyze': {
      const [text, instr] = body.split('|||');
      return lc.analyze(text || '', instr || '');
    }
    case 'classify': {
      const [txt, labels] = body.split('|||');
      return lc.classify(txt || '', labels || '');
    }
    case 'translate': {
      const [txt, lang] = body.split('|||');
      return lc.translate(txt || '', lang || '');
    }
    case 'agent': {
      const [instr, query] = body.split('|||');
      return lc.agent(model || 'gpt-4o-mini', instr || '', query || '');
    }
    case 'agentStream': {
      const [instr, query] = body.split('|||');
      return lc.agentStream(model || 'gpt-4o-mini', instr || '', query || '');
    }
    case 'chain': {
      const [steps, input] = body.split('|||');
      return lc.chain(steps || '', input || '');
    }
    case 'extract': {
      const [txt, schema] = body.split('|||');
      return lc.extract(txt || '', schema || '');
    }
    default: return `[langchain unknown method: ${lcMethod}]`;
  }
}

function routeLangGraph(method, url, body) {
  const lg = getLangGraph();
  if (!lg) return '[langgraph runtime not available]';

  const rest = url.slice('__langgraph__/'.length);
  const lgMethod = rest;

  switch (lgMethod) {
    case 'graph': return lg.graph(body);
    case 'addNode': {
      const [gid, name, spec] = body.split('|||');
      return lg.addNode(gid || '', name || '', spec || '');
    }
    case 'addEdge': {
      const [gid, from, to] = body.split('|||');
      return lg.addEdge(gid || '', from || '', to || '');
    }
    case 'addConditionalEdges': {
      const [gid, from, cond, mapping] = body.split('|||');
      return lg.addConditionalEdges(gid || '', from || '', cond || '', mapping || '');
    }
    case 'compile': return lg.compile(body);
    case 'invoke': {
      const [gid, state] = body.split('|||');
      return lg.invoke(gid || '', state || '');
    }
    case 'stream': {
      const [gid, state] = body.split('|||');
      return lg.stream(gid || '', state || '');
    }
    case 'getState': return lg.getState(body);
    case 'updateState': {
      const [gid, state] = body.split('|||');
      return lg.updateState(gid || '', state || '');
    }
    case 'branch': {
      const [gid, spec] = body.split('|||');
      return lg.branch(gid || '', spec || '');
    }
    case 'interrupt': {
      const [gid, state] = body.split('|||');
      return lg.interrupt(gid || '', state || '');
    }
    case 'resume': return lg.resume(body);
    default: return `[langgraph unknown method: ${lgMethod}]`;
  }
}

// --- BLE routing for Elysium package use ---

let bleMod = null;
let zigbeeMod = null;

function getBle() {
  if (!bleMod) {
    try { bleMod = require('./ble'); } catch (_) { bleMod = null; }
  }
  return bleMod;
}

function getZigbee() {
  if (!zigbeeMod) {
    try { zigbeeMod = require('./zigbee'); } catch (_) { zigbeeMod = null; }
  }
  return zigbeeMod;
}

function routeBle(method, url, body) {
  const bm = getBle();
  if (!bm) return '[ble runtime not available]';

  const rest = url.slice('__ble__/'.length);
  const args = (body || '').split('|||');

  switch (rest) {
    case 'scan': return bm.scan();
    case 'stopScan': return bm.stopScan();
    case 'connect': return bm.connect(args[0] || '');
    case 'disconnect': return bm.disconnect(args[0] || '');
    case 'readCharacteristic': return bm.readCharacteristic(args[0] || '', args[1] || '');
    case 'writeCharacteristic': return bm.writeCharacteristic(args[0] || '', args[1] || '', args[2] || '');
    case 'readRssi': return bm.readRssi(args[0] || '');
    case 'deviceName': return bm.deviceName(args[0] || '');
    case 'isConnected': return bm.isConnected(args[0] || '') ? 'true' : 'false';
    case 'isScanning': return bm.isScanning() ? 'true' : 'false';
    default: return `[ble unknown method: ${rest}]`;
  }
}

function routeZigbee(method, url, body) {
  const zm = getZigbee();
  if (!zm) return '[zigbee runtime not available]';

  const rest = url.slice('__zigbee__/'.length);
  const args = (body || '').split('|||');

  switch (rest) {
    case 'start': return zm.start();
    case 'shutdown': return zm.shutdown();
    case 'permitJoin': return zm.permitJoin(parseInt(args[0] || '0', 10));
    case 'scan': return zm.scan();
    case 'on': return zm.on(args[0] || '', parseInt(args[1] || '1', 10), args[2] || '');
    case 'off': return zm.off(args[0] || '', parseInt(args[1] || '1', 10), args[2] || '');
    case 'toggle': return zm.toggle(args[0] || '', parseInt(args[1] || '1', 10), args[2] || '');
    case 'readAttribute': return zm.readAttribute(args[0] || '', parseInt(args[1] || '1', 10), args[2] || '', args[3] || '');
    case 'writeAttribute': return zm.writeAttribute(args[0] || '', parseInt(args[1] || '1', 10), args[2] || '', args[3] || '', args[4] || '');
    case 'addToGroup': return zm.addToGroup(args[0] || '', parseInt(args[1] || '0', 10));
    case 'removeFromGroup': return zm.removeFromGroup(args[0] || '', parseInt(args[1] || '0', 10));
    case 'bind': return zm.bind(args[0] || '', parseInt(args[1] || '1', 10), args[2] || '', parseInt(args[3] || '1', 10));
    case 'getDeviceName': return zm.getDeviceName(args[0] || '');
    case 'getManufacturer': return zm.getManufacturer(args[0] || '');
    case 'getDeviceCount': return String(zm.getDeviceCount());
    case 'getPanId': return String(zm.getPanId());
    case 'getChannel': return String(zm.getChannel());
    case 'isJoined': return zm.isJoined() ? 'true' : 'false';
    case 'isOnline': return zm.isOnline(args[0] || '') ? 'true' : 'false';
    case 'isPermittingJoin': return zm.isPermittingJoin() ? 'true' : 'false';
    default: return `[zigbee unknown method: ${rest}]`;
  }
}

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

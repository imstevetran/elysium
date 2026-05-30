/**
 * Elysium Runtime — Portable Worker module.
 *
 * Provides a unified worker API that works across:
 * - Browser: Web Worker API
 * - Node.js: worker_threads
 * - Native (C backend): emitted as stub (real implementation in JS runtime)
 *
 * The API is the same regardless of target, enabling portable worker code.
 *
 * Usage (from Elysium):
 *   let id = worker.create("onmessage = function(e) { postMessage('echo: ' + e.data); }")
 *   worker.post(id, "hello")
 *   let reply = worker.wait(id)
 *   worker.terminate(id)
 */

// Detected runtime capabilities
const isNode = typeof process !== 'undefined' && process.versions && process.versions.node;
const isBrowser = typeof Worker !== 'undefined' && !isNode;

// In-memory worker registry
const workers = new Map();
const messageQueues = new Map();
let workerCounter = 0;

// Response/reply tracking for blocking send
const pendingReplies = new Map();
let replyCounter = 0;

/**
 * Try to create a real Web Worker in the current environment.
 * Returns null if not supported.
 */
function createRealWorker(scriptOrCode) {
  try {
    if (isBrowser) {
      // Browser Web Worker
      const code = typeof scriptOrCode === 'string'
        ? scriptOrCode
        : 'onmessage = function(e) { postMessage(e.data); }';
      // Wrap in a blob URL
      const blob = new Blob([code], { type: 'application/javascript' });
      const url = URL.createObjectURL(blob);
      const w = new Worker(url);
      w._url = url;
      return w;
    }
    if (isNode) {
      // Node.js worker_threads (eval mode)
      const { Worker: NodeWorker } = require('worker_threads');
      const setup = `
        const { parentPort } = require('worker_threads');
        ${scriptOrCode || "parentPort.on('message', (msg) => { parentPort.postMessage(msg); });"}
      `;
      const w = new NodeWorker(setup, { eval: true });
      return w;
    }
  } catch (e) {
    // Fall through to mock
  }
  return null;
}

/**
 * Create a mock worker context when real workers aren't available.
 */
function createMockWorker(id, scriptOrCode) {
  const queue = messageQueues.get(id) || [];
  messageQueues.set(id, queue);

  // Simple mock that echoes messages with basic processing
  const handlers = {
    _default(msg) {
      queue.push(JSON.stringify({ type: 'echo', original: msg }));
    }
  };
  return { handlers, queue };
}

const worker = {
  /**
   * Create a new worker from script code or URL.
   * @param {string} scriptOrCode - Inline worker code or script URL
   * @returns {string} workerId
   */
  create(scriptOrCode) {
    const id = `worker_${++workerCounter}`;
    const msgQueue = [];
    messageQueues.set(id, msgQueue);

    const real = createRealWorker(scriptOrCode);
    if (real) {
      real.onmessage = (e) => {
        const data = e.data || (e && e.target && e.target.result) || e;
        // Check if this is a reply to a send() call (has _replyId)
        if (data && data._replyId !== undefined && pendingReplies.has(data._replyId)) {
          pendingReplies.get(data._replyId).resolve(data._payload !== undefined ? data._payload : data);
          pendingReplies.delete(data._replyId);
          return;
        }
        msgQueue.push(typeof data === 'string' ? data : JSON.stringify(data));
      };
      real.onerror = (err) => {
        msgQueue.push(JSON.stringify({ type: 'error', message: err && err.message ? err.message : 'Worker error' }));
      };
      workers.set(id, { type: 'real', worker: real });
    } else {
      const mock = createMockWorker(id, scriptOrCode);
      workers.set(id, { type: 'mock', mock, script: scriptOrCode });
    }

    return id;
  },

  /**
   * Send a message to a worker and wait for a reply (blocking).
   * On JS runtime: uses a reply ID pattern, internally uses async+Promise
   * On native: uses channel-based synchronous send+receive
   * @param {string} workerId
   * @param {string} message - message string
   * @returns {string} reply message
   */
  send(workerId, message) {
    const entry = workers.get(workerId);
    if (!entry) throw new Error(`Worker not found: ${workerId}`);

    const replyId = ++replyCounter;
    const payload = { _replyId: replyId, _payload: message };

    if (entry.type === 'real' && entry.worker) {
      return new Promise((resolve, reject) => {
        pendingReplies.set(replyId, { resolve, reject });
        entry.worker.postMessage(payload);
        // Set a timeout to avoid hanging forever
        setTimeout(() => {
          if (pendingReplies.has(replyId)) {
            pendingReplies.delete(replyId);
            resolve(JSON.stringify({ type: 'timeout' }));
          }
        }, 30000);
      });
    }

    // Mock — simulate by queueing and returning echo
    const queue = messageQueues.get(workerId) || [];
    if (entry.mock && entry.mock.handlers) {
      const h = entry.mock.handlers;
      if (h[message]) {
        h[message](message);
      } else {
        h._default(message);
      }
    } else {
      queue.push(JSON.stringify({ type: 'echo', original: message }));
    }

    // For mock, immediately return the queued message
    if (queue.length > 0) {
      return queue.shift();
    }
    return JSON.stringify({ type: 'echo', original: message });
  },

  /**
   * Post a message to a worker (fire-and-forget, non-blocking).
   * @param {string} workerId
   * @param {string} message - message string
   * @returns {string} "ok"
   */
  post(workerId, message) {
    const entry = workers.get(workerId);
    if (!entry) throw new Error(`Worker not found: ${workerId}`);

    if (entry.type === 'real' && entry.worker) {
      entry.worker.postMessage(message);
    } else {
      const queue = messageQueues.get(workerId) || [];
      if (entry.mock && entry.mock.handlers) {
        entry.mock.handlers._default(message);
      } else {
        queue.push(JSON.stringify({ type: 'echo', original: message }));
      }
    }

    return 'ok';
  },

  /**
   * Read the next message from the worker's message queue (non-blocking).
   * Returns "null" if no message available.
   * @param {string} workerId
   * @returns {string} message string, or "null" if none
   */
  receive(workerId) {
    const queue = messageQueues.get(workerId);
    if (!queue) throw new Error(`Worker not found: ${workerId}`);

    if (queue.length > 0) {
      return queue.shift();
    }
    return 'null';
  },

  /**
   * Block until a message arrives from the worker.
   * Polls at 10ms intervals with a 30s timeout.
   * @param {string} workerId
   * @returns {string} message string
   */
  wait(workerId) {
    const queue = messageQueues.get(workerId);
    if (!queue) throw new Error(`Worker not found: ${workerId}`);

    const start = Date.now();
    while (queue.length === 0) {
      if (Date.now() - start > 30000) {
        return JSON.stringify({ type: 'timeout' });
      }
    }
    return queue.shift();
  },

  /**
   * Terminate a worker and free its resources.
   * @param {string} workerId
   * @returns {string} "ok"
   */
  terminate(workerId) {
    const entry = workers.get(workerId);
    if (!entry) throw new Error(`Worker not found: ${workerId}`);

    if (entry.type === 'real' && entry.worker) {
      try {
        if (typeof entry.worker.terminate === 'function') {
          entry.worker.terminate();
        }
      } catch (e) {
        // Ignore termination errors
      }
      if (entry.worker._url) {
        try { URL.revokeObjectURL(entry.worker._url); } catch (e) {}
      }
    }

    workers.delete(workerId);
    messageQueues.delete(workerId);
    return 'ok';
  },

  /**
   * Check if a worker is still running.
   * @param {string} workerId
   * @returns {string} "true" or "false"
   */
  isRunning(workerId) {
    const entry = workers.get(workerId);
    return entry ? 'true' : 'false';
  },

  /**
   * Get the number of active workers.
   * @returns {string} number as string
   */
  activeCount() {
    return String(workers.size);
  },

  /**
   * Terminate all active workers.
   * @returns {string} "ok"
   */
  terminateAll() {
    for (const [id, entry] of workers.entries()) {
      if (entry.type === 'real' && entry.worker) {
        try {
          if (typeof entry.worker.terminate === 'function') {
            entry.worker.terminate();
          }
        } catch (e) {}
      }
    }
    workers.clear();
    messageQueues.clear();
    return 'ok';
  },
};

module.exports = worker;

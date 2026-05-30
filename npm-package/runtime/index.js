/**
 * Elysium Runtime — index module.
 *
 * Re-exports all runtime modules: arc, task, channel, ui.
 *
 * Usage:
 *   const elysium = require('elysium-lang');
 *   const ref = new elysium.Ref(42);
 *   const scheduler = new elysium.Scheduler(4);
 *   const chan = new elysium.Channel({ capacity: 10 });
 */

const arc = require('./arc');
const task = require('./task');
const channel = require('./channel');
const ui = require('./ui');
const consoleMod = require('./console');
const fsMod = require('./fs');
const transportMod = require('./transport');
const stringMod = require('./string');
const regexMod = require('./regex');
const datetimeMod = require('./datetime');
const asyncMod = require('./async');
const parallelMod = require('./parallel');
const bleMod = require('./ble');
const zigbeeMod = require('./zigbee');
const langchainMod = require('./langchain');
const langgraphMod = require('./langgraph');
const authMod = require('./auth');
const workerMod = require('./worker');
const dictMod = require('./dict');
const jsonMod = require('./json');
const mathMod = require('./math');
const envMod = require('./env');
const httpMod = require('./http');
const isMod = require('./is');

module.exports = {
  // ARC
  Ref: arc.Ref,
  Weak: arc.Weak,
  Unowned: arc.Unowned,

  // Task scheduler
  Task: task.Task,
  Scheduler: task.Scheduler,

  // Channels
  Channel: channel.Channel,

  // UI
  View: ui.View,
  Style: ui.Style,
  ComponentState: ui.ComponentState,
  diff: ui.diff,
  Patch: ui.Patch,
  Axis: ui.Axis,

  // Console / unified logging
  console: consoleMod,

  // File system
  fs: fsMod,

  // Transport (HTTP, WebSocket, MQTT)
  transport: transportMod,

  // String utilities
  string: stringMod,

  // Regex utilities
  regex: regexMod,

  // DateTime
  datetime: datetimeMod,

  // BLE (Bluetooth Low Energy)
  ble: bleMod,

  // Zigbee (Home Automation)
  zigbee: zigbeeMod,

  // LangChain (LLM, Chat, RAG, Agents, AI)
  langchain: langchainMod,

  // LangGraph (Stateful Graph-Based Agent Orchestration)
  langgraph: langgraphMod,

  // Auth (Session, JWT, Passkey, OAuth2, Authorization, Multi-tenant)
  auth: authMod,

  // Worker
  worker: workerMod,

  // Dict (mutable key-value dictionary)
  dict: dictMod,

  // JSON (parsing and serialization)
  json: jsonMod,

  // Math (extended math operations)
  math: mathMod,

  // Env (environment variables)
  env: envMod,

  // HTTP (HTTP client with custom headers)
  http: httpMod,

  // Async/Await
  __await: asyncMod.__await,
  __async: asyncMod.__async,

  // Parallel
  __parallel: parallelMod.__parallel,

  // Runtime type check (instanceof)
  __is_instanceof: isMod.__is_instanceof,
};

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
};

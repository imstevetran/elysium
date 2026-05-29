/**
 * Async task scheduler for Elysium.
 * Mirrors elysium-rt/src/task.rs.
 *
 * Task       — Represents an async computation.
 * Scheduler  — Work-stealing thread pool (simulated with a task queue).
 */

const { EventEmitter } = require('events');

/**
 * A task represents an async computation.
 */
class Task {
  constructor(id) {
    this.id = id;
  }
}

/**
 * Task scheduler using a task queue.
 * In Node.js, we use setImmediate/nextTick to simulate concurrent execution.
 */
class Scheduler {
  constructor(numThreads) {
    this.tasks = [];
    this.threadCount = numThreads || 4;
    this.running = false;
    this.queue = [];
  }

  spawn(fn) {
    if (typeof fn !== 'function') {
      throw new Error('spawn requires a function');
    }

    // Queue the task and schedule execution
    this.queue.push(fn);
    if (!this.running) {
      this.running = true;
      this._processQueue();
    }
  }

  _processQueue() {
    if (this.queue.length === 0) {
      this.running = false;
      return;
    }

    const fn = this.queue.shift();
    // Use setImmediate to avoid blocking the event loop
    setImmediate(() => {
      try {
        fn();
      } catch (e) {
        console.error('Scheduler task error:', e);
      }
      // Process next task
      this._processQueue();
    });
  }
}

module.exports = { Task, Scheduler };

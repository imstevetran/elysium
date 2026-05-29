/**
 * Channel implementation for Elysium concurrency.
 * Mirrors elysium-rt/src/channel.rs.
 *
 * Channel<T> — Bounded or unbounded channel for communicating between tasks.
 */

const { EventEmitter } = require('events');

/**
 * A bounded or unbounded channel for communicating between tasks.
 */
class Channel {
  constructor(options = {}) {
    this.buffer = [];
    this.capacity = options.capacity || null;
    this.closed = false;
    this._emitter = new EventEmitter();
    this._emitter.setMaxListeners(0);
  }

  /**
   * Send a value into the channel.
   * If the channel is bounded and full, the method blocks (throws in JS).
   * In practice, use an async pattern or increase capacity.
   */
  send(value) {
    if (this.closed) {
      throw new Error('channel is closed');
    }
    if (this.capacity !== null && this.buffer.length >= this.capacity) {
      throw new Error('channel buffer is full');
    }
    this.buffer.push(value);
    this._emitter.emit('message');
  }

  /**
   * Receive a value from the channel.
   * Returns a Promise that resolves when a value is available.
   */
  async receive() {
    while (this.buffer.length === 0) {
      if (this.closed) {
        throw new Error('channel is empty and closed');
      }
      // Wait for a message
      await new Promise((resolve) => {
        const onMessage = () => {
          this._emitter.removeListener('message', onMessage);
          this._emitter.removeListener('close', onMessage);
          resolve();
        };
        this._emitter.on('message', onMessage);
        this._emitter.once('close', onMessage);
      });
    }
    return this.buffer.shift();
  }

  /**
   * Receive a value synchronously (throws if empty).
   */
  receiveSync() {
    if (this.buffer.length === 0) {
      throw new Error('channel is empty');
    }
    return this.buffer.shift();
  }

  /**
   * Close the channel. No more sends allowed. Drains remaining buffer on receive.
   */
  close() {
    this.closed = true;
    this._emitter.emit('close');
  }
}

module.exports = { Channel };

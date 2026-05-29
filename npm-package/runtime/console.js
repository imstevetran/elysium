/**
 * Elysium Console — unified logging for backend and browser.
 *
 * Backend (compiled binary): uses printf with [LEVEL] prefixes.
 * Client-side / Node: maps to native console.* API.
 *
 * Usage:
 *   const console = require('elysium-lang/runtime/console');
 *   console.debug('message');  // console.debug in browser, [DEBUG] prefix in compiled
 *   console.info('msg');       // console.info
 *   console.warn('msg');       // console.warn
 *   console.error('msg');      // console.error
 *   console.log('msg');        // console.log
 */

// Detect environment
const isBrowser = typeof window !== 'undefined' && typeof window.document !== 'undefined';
const isNode = typeof process !== 'undefined' && process.versions && process.versions.node;

const nativeConsole = isBrowser
  ? window.console
  : (isNode ? global.console : null);

// Fallback if no native console available
function noop() {}

function formatArgs(args) {
  return Array.from(args).map(a => {
    if (a === null) return 'nil';
    if (a === undefined) return 'nil';
    if (typeof a === 'object') {
      try { return JSON.stringify(a); } catch (_) { return String(a); }
    }
    return String(a);
  }).join(' ');
}

function createConsoleMethod(method, level) {
  const native = nativeConsole && nativeConsole[method]
    ? nativeConsole[method].bind(nativeConsole)
    : null;

  return function(...args) {
    const formatted = formatArgs(args);
    if (native) {
      native(formatted);
    } else if (typeof console !== 'undefined' && console[method]) {
      console[method](formatted);
    } else {
      // Fallback: print to stdout/stderr
      const prefix = level ? `[${level}] ` : '';
      const output = prefix + formatted + '\n';
      if (typeof process !== 'undefined' && process.stdout) {
        if (method === 'error' || method === 'warn') {
          process.stderr.write(output);
        } else {
          process.stdout.write(output);
        }
      }
    }
  };
}

const consoleObj = {
  debug: createConsoleMethod('debug', 'DEBUG'),
  info: createConsoleMethod('info', 'INFO'),
  warn: createConsoleMethod('warn', 'WARN'),
  error: createConsoleMethod('error', 'ERROR'),
  log: createConsoleMethod('log', 'LOG'),
};

module.exports = consoleObj;

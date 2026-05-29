/**
 * Elysium DateTime — unified date/time between backend and frontend.
 *
 * Backend (compiled binary): uses C time.h (time, ctime, localtime, mktime).
 * Client-side / Node: wraps native JavaScript Date.
 *
 * All APIs use unix timestamps (seconds since epoch, i64) as the universal bridge.
 *
 * Usage:
 *   const dt = require('elysium-lang/runtime/datetime');
 *   let now = dt.now();                    // unix timestamp
 *   let str = dt.fromTimestamp(now);       // "Sat May 29 12:00:00 2026\n"
 *   let year = dt.year(now);               // 2026
 */

// Helpers
function _ts(ts) { return typeof ts === 'number' ? ts * 1000 : Date.now(); }

/**
 * Current unix timestamp (seconds).
 * datetime.now() → Int
 */
function now() {
  return Math.floor(Date.now() / 1000);
}

/**
 * Convert unix timestamp to locale date string.
 * datetime.fromTimestamp(ts) → String
 */
function fromTimestamp(ts) {
  const d = new Date(_ts(ts));
  return d.toLocaleString();
}

/**
 * Format a timestamp using a strftime-like format string.
 * Supports: %Y %m %d %H %M %S %a %b
 * datetime.format(ts, format) → String
 */
function format(ts, fmt) {
  const d = new Date(_ts(ts));
  const pad = (n) => String(n).padStart(2, '0');
  const months = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'];
  const weekdays = ['Sun','Mon','Tue','Wed','Thu','Fri','Sat'];
  let result = '';
  for (let i = 0; i < fmt.length; i++) {
    if (fmt[i] === '%' && i + 1 < fmt.length) {
      const c = fmt[i + 1];
      i++;
      switch (c) {
        case 'Y': result += d.getFullYear(); break;
        case 'y': result += String(d.getFullYear()).slice(-2); break;
        case 'm': result += pad(d.getMonth() + 1); break;
        case 'd': result += pad(d.getDate()); break;
        case 'H': result += pad(d.getHours()); break;
        case 'M': result += pad(d.getMinutes()); break;
        case 'S': result += pad(d.getSeconds()); break;
        case 'a': result += weekdays[d.getDay()]; break;
        case 'b': result += months[d.getMonth()]; break;
        default: result += c; break;
      }
    } else {
      result += fmt[i];
    }
  }
  return result;
}

/**
 * Parse a date string.
 * datetime.parse(str, format) → Int (unix timestamp)
 */
function parse(str) {
  const ms = Date.parse(str);
  return isNaN(ms) ? 0 : Math.floor(ms / 1000);
}

/**
 * Extract year from unix timestamp.
 * datetime.year(ts) → Int
 */
function year(ts) {
  return new Date(_ts(ts)).getFullYear();
}

/**
 * Extract month (1-12) from unix timestamp.
 * datetime.month(ts) → Int
 */
function month(ts) {
  return new Date(_ts(ts)).getMonth() + 1;
}

/**
 * Extract day of month (1-31) from unix timestamp.
 * datetime.day(ts) → Int
 */
function day(ts) {
  return new Date(_ts(ts)).getDate();
}

/**
 * Extract hour (0-23) from unix timestamp.
 * datetime.hour(ts) → Int
 */
function hour(ts) {
  return new Date(_ts(ts)).getHours();
}

/**
 * Extract minute (0-59) from unix timestamp.
 * datetime.minute(ts) → Int
 */
function minute(ts) {
  return new Date(_ts(ts)).getMinutes();
}

/**
 * Extract second (0-59) from unix timestamp.
 * datetime.second(ts) → Int
 */
function second(ts) {
  return new Date(_ts(ts)).getSeconds();
}

/**
 * Extract day of week (0=Sunday, 6=Saturday).
 * datetime.weekday(ts) → Int
 */
function weekday(ts) {
  return new Date(_ts(ts)).getDay();
}

/**
 * Add days to a timestamp.
 * datetime.addDays(ts, days) → Int
 */
function addDays(ts, n) {
  return ts + (n * 86400);
}

/**
 * Add hours to a timestamp.
 * datetime.addHours(ts, hours) → Int
 */
function addHours(ts, n) {
  return ts + (n * 3600);
}

/**
 * Difference in seconds between two timestamps.
 * datetime.diffSeconds(ts1, ts2) → Int
 */
function diffSeconds(ts1, ts2) {
  return ts1 - ts2;
}

module.exports = {
  now,
  fromTimestamp,
  format,
  parse,
  year,
  month,
  day,
  hour,
  minute,
  second,
  weekday,
  addDays,
  addHours,
  diffSeconds,
};

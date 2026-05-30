/**
 * Elysium Zigbee Runtime — Zigbee Home Automation module.
 *
 * Provides a high-level API for Zigbee coordinator operations:
 * network management, device discovery, attribute read/write,
 * device control (on/off/toggle), group management, and binding.
 *
 * All functions are synchronous in stub mode. In production use,
 * pair with a Zigbee coordinator USB dongle (e.g., Texas Instruments
 * CC2531/CC2652, Elelabs, or Conbee II) via a bridge library like
 * zigbee-herdsman or a serial-API adapter.
 *
 * Exported API:
 *  - start()                         → nil     Start the Zigbee coordinator
 *  - shutdown()                      → nil     Shut down the coordinator
 *  - permitJoin(seconds: Int)        → nil     Permit joining for N seconds
 *  - scan()                          → nil     Scan for nearby Zigbee networks
 *  - on(deviceId, endpoint, cluster) → nil     Turn on a device/cluster
 *  - off(deviceId, endpoint, cluster)→ nil     Turn off a device/cluster
 *  - toggle(deviceId, endpoint, cluster)→ nil  Toggle a device/cluster
 *  - readAttribute(deviceId, endpoint, cluster, attr) → String  Read an attribute
 *  - writeAttribute(deviceId, endpoint, cluster, attr, value) → nil  Write an attribute
 *  - addToGroup(deviceId, groupId)   → nil     Add device to a group
 *  - removeFromGroup(deviceId, groupId) → nil   Remove device from a group
 *  - bind(sourceDev, sourceEp, destDev, destEp) → nil   Bind two devices
 *  - getDeviceName(deviceId)         → String  Get device name
 *  - getManufacturer(deviceId)       → String  Get manufacturer name
 *  - getDeviceCount()                → Int     Number of paired devices
 *  - getPanId()                      → Int     Get current PAN ID
 *  - getChannel()                    → Int     Get current channel
 *  - isJoined()                      → Bool    Is joined to a network
 *  - isOnline(deviceId)              → Bool    Is a device online
 *  - isPermittingJoin()              → Bool    Is join permitted
 */

let coordinator = null;
let devices = new Map();
let panId = 0;
let channel = 11;
let scanning = false;
let permitJoinActive = false;

// Try to load zigbee-herdsman for Node.js
const isNode = typeof process !== 'undefined' && process.versions && process.versions.node;
let herdsman = null;
if (isNode) {
  try {
    herdsman = require('zigbee-herdsman');
  } catch (_) {
    // not available, use stub
  }
}

// ---------------------------------------------------------------------------
// Network management
// ---------------------------------------------------------------------------

function start() {
  if (herdsman) {
    console.log('[zigbee] initializing coordinator via zigbee-herdsman...');
    try {
      // Simplified initialisation — real usage needs proper adapter config
      const Controller = herdsman.Controller;
      if (Controller) {
        coordinator = new Controller({ network: { panID: panId, channel }, databasePath: './zigbee.db' });
        coordinator.start();
        console.log('[zigbee] coordinator started');
      }
    } catch (err) {
      console.log('[zigbee] init error:', err.message);
    }
  } else {
    console.log('[zigbee] init() — stub (install zigbee-herdsman for real Zigbee)');
    panId = 0x1A62;
    channel = 15;
  }
}

function shutdown() {
  if (coordinator) {
    coordinator.stop();
    coordinator = null;
  }
  devices.clear();
  console.log('[zigbee] shutdown complete');
}

function permitJoin(seconds) {
  if (coordinator && coordinator.permitJoin) {
    coordinator.permitJoin(seconds);
    permitJoinActive = seconds > 0;
    console.log('[zigbee] permitJoin for', seconds, 'seconds');
  } else {
    permitJoinActive = seconds > 0;
    console.log('[zigbee] permitJoin(' + seconds + ') — stub');
  }
}

function scan() {
  if (herdsman && coordinator) {
    console.log('[zigbee] scanning for networks...');
    // Real scanning would use an energy scan or active channel scan
    scanning = true;
    setTimeout(() => { scanning = false; }, 5000);
  } else {
    scanning = true;
    console.log('[zigbee] scan() — stub (found mock network on ch' + channel + ')');
    setTimeout(() => { scanning = false; }, 2000);
  }
}

// ---------------------------------------------------------------------------
// Device control
// ---------------------------------------------------------------------------

function on(deviceId, endpoint, cluster) {
  const dev = devices.get(deviceId);
  if (dev && dev.endpoint) {
    dev.endpoint.command(cluster, 0x01, {});  // On/Off cluster, On command
    console.log('[zigbee] turned ON', deviceId, 'ep' + endpoint);
  } else {
    console.log('[zigbee] on(' + deviceId + ', ep' + endpoint + ', ' + cluster + ') — stub');
  }
}

function off(deviceId, endpoint, cluster) {
  const dev = devices.get(deviceId);
  if (dev && dev.endpoint) {
    dev.endpoint.command(cluster, 0x00, {});  // On/Off cluster, Off command
    console.log('[zigbee] turned OFF', deviceId, 'ep' + endpoint);
  } else {
    console.log('[zigbee] off(' + deviceId + ', ep' + endpoint + ', ' + cluster + ') — stub');
  }
}

function toggle(deviceId, endpoint, cluster) {
  const dev = devices.get(deviceId);
  if (dev && dev.endpoint) {
    dev.endpoint.command(cluster, 0x02, {});  // On/Off cluster, Toggle command
    console.log('[zigbee] toggled', deviceId, 'ep' + endpoint);
  } else {
    console.log('[zigbee] toggle(' + deviceId + ', ep' + endpoint + ', ' + cluster + ') — stub');
  }
}

// ---------------------------------------------------------------------------
// Attribute read/write
// ---------------------------------------------------------------------------

function readAttribute(deviceId, endpoint, cluster, attribute) {
  const dev = devices.get(deviceId);
  if (dev && dev.endpoint) {
    try {
      const result = dev.endpoint.read(cluster, attribute);
      console.log('[zigbee] read', deviceId, cluster, attribute, '=', result);
      return String(result);
    } catch (err) {
      console.log('[zigbee] read error:', err.message);
      return '';
    }
  }
  console.log('[zigbee] readAttribute(' + deviceId + ', ' + cluster + ', ' + attribute + ') — stub');
  return '';
}

function writeAttribute(deviceId, endpoint, cluster, attribute, value) {
  const dev = devices.get(deviceId);
  if (dev && dev.endpoint) {
    try {
      dev.endpoint.write(cluster, attribute, value);
      console.log('[zigbee] wrote', deviceId, cluster, attribute, '=', value);
    } catch (err) {
      console.log('[zigbee] write error:', err.message);
    }
  } else {
    console.log('[zigbee] writeAttribute(' + deviceId + ', ' + cluster + ', ' + attribute + ', ' + value + ') — stub');
  }
}

// ---------------------------------------------------------------------------
// Group management
// ---------------------------------------------------------------------------

function addToGroup(deviceId, groupId) {
  const dev = devices.get(deviceId);
  if (dev && dev.endpoint) {
    dev.endpoint.addToGroup(groupId);
    console.log('[zigbee] added', deviceId, 'to group', groupId);
  } else {
    console.log('[zigbee] addToGroup(' + deviceId + ', ' + groupId + ') — stub');
  }
}

function removeFromGroup(deviceId, groupId) {
  const dev = devices.get(deviceId);
  if (dev && dev.endpoint) {
    dev.endpoint.removeFromGroup(groupId);
    console.log('[zigbee] removed', deviceId, 'from group', groupId);
  } else {
    console.log('[zigbee] removeFromGroup(' + deviceId + ', ' + groupId + ') — stub');
  }
}

// ---------------------------------------------------------------------------
// Binding
// ---------------------------------------------------------------------------

function bind(sourceDevId, sourceEp, destDevId, destEp) {
  const src = devices.get(sourceDevId);
  const dst = devices.get(destDevId);
  if (src && dst && src.endpoint && dst.endpoint) {
    src.endpoint.bind('genOnOff', dst.endpoint);
    console.log('[zigbee] bound', sourceDevId, 'ep' + sourceEp, '→', destDevId, 'ep' + destEp);
  } else {
    console.log('[zigbee] bind(' + sourceDevId + ', ep' + sourceEp + ', ' + destDevId + ', ep' + destEp + ') — stub');
  }
}

// ---------------------------------------------------------------------------
// Info / status
// ---------------------------------------------------------------------------

function getDeviceName(deviceId) {
  const dev = devices.get(deviceId);
  if (dev) return dev.name || 'Unknown';
  console.log('[zigbee] getDeviceName(' + deviceId + ') — stub');
  return 'Zigbee Device ' + deviceId.substring(0, 8);
}

function getManufacturer(deviceId) {
  const dev = devices.get(deviceId);
  if (dev) return dev.manufacturer || 'Unknown';
  return 'Generic';
}

function getDeviceCount() {
  return devices.size;
}

function getPanId() {
  return panId;
}

function getChannel() {
  return channel;
}

function isJoined() {
  return panId !== 0;
}

function isOnline(deviceId) {
  const dev = devices.get(deviceId);
  if (dev) return dev.online !== false;
  return false;
}

function isPermittingJoin() {
  return permitJoinActive;
}

// Helper: register a mock device (used by example or pairing callback)
function _registerDevice(id, name, manufacturer) {
  devices.set(id, { name, manufacturer, online: true, endpoint: null });
  console.log('[zigbee] registered device:', id, name);
}

module.exports = {
  start,
  shutdown,
  permitJoin,
  scan,
  on,
  off,
  toggle,
  readAttribute,
  writeAttribute,
  addToGroup,
  removeFromGroup,
  bind,
  getDeviceName,
  getManufacturer,
  getDeviceCount,
  getPanId,
  getChannel,
  isJoined,
  isOnline,
  isPermittingJoin,
};

/**
 * Elysium BLE Runtime — Bluetooth Low Energy module.
 *
 * Uses Web Bluetooth API (browser) or noble-based adapter (Node.js).
 * All functions are synchronous in signature but may perform async ops internally.
 *
 * Exported API:
 *  - scan()                          → nil       Start scanning for BLE devices
 *  - stopScan()                      → nil       Stop scanning
 *  - connect(address: String)        → String    Connect to device, returns device ID
 *  - disconnect(deviceId: String)    → nil       Disconnect from device
 *  - readCharacteristic(dev, uuid)   → String    Read characteristic value as hex
 *  - writeCharacteristic(dev, uuid, value) → nil Write hex value to characteristic
 *  - readRssi(deviceId: String)      → String    Read RSSI as dBm string
 *  - deviceName(deviceId: String)    → String    Get device name
 *  - isConnected(deviceId: String)   → Bool      Check if device is connected
 *  - isScanning()                    → Bool      Check if currently scanning
 */

let noble = null;
let WebBluetooth = null;

// Environment detection
const isNode = typeof process !== 'undefined' && process.versions && process.versions.node;
const isBrowser = typeof window !== 'undefined' && typeof window.navigator !== 'undefined';

// Try to load noble (Node.js)
if (isNode) {
  try {
    noble = require('@abandonware/noble');
  } catch (_) {
    // noble not available, fall back to stub
  }
}

// Internal state
const devices = new Map();   // address → { peripheral, name, rssi }
let scanning = false;
let scanCallback = null;

function scan() {
  if (isBrowser && navigator.bluetooth) {
    // Web Bluetooth API
    console.log('[ble] Web Bluetooth: use navigator.bluetooth.requestDevice()');
    navigator.bluetooth.requestDevice({
      acceptAllDevices: true,
      optionalServices: []
    }).then(device => {
      const id = device.id;
      devices.set(id, { device, name: device.name || 'Unknown', rssi: null });
      console.log('[ble] connected to:', device.name);
    }).catch(err => {
      console.log('[ble] requestDevice error:', err.message);
    });
    scanning = true;
  } else if (noble) {
    noble.startScanning([], false);
    noble.on('discover', peripheral => {
      const address = peripheral.address || peripheral.id;
      devices.set(address, {
        peripheral,
        name: peripheral.advertisement ? peripheral.advertisement.localName : 'Unknown',
        rssi: peripheral.rssi
      });
      if (scanCallback) scanCallback(address);
      console.log('[ble] discovered:', address, peripheral.advertisement ? peripheral.advertisement.localName : '');
    });
    scanning = true;
    console.log('[ble] scanning started');
  } else {
    console.log('[ble] scan() — stub (no BLE backend available)');
    scanning = true;
  }
}

function stopScan() {
  if (noble) {
    noble.stopScanning();
    noble.removeAllListeners('discover');
  }
  scanning = false;
  scanCallback = null;
  console.log('[ble] scanning stopped');
}

function connect(address) {
  if (isBrowser && navigator.bluetooth) {
    const existing = devices.get(address);
    if (existing && existing.device && existing.device.gatt) {
      return existing.device.gatt.connect().then(server => {
        console.log('[ble] connected to GATT server:', address);
        return address;
      }).catch(err => {
        console.log('[ble] GATT connect error:', err.message);
        return address;
      });
    }
    console.log('[ble] connect() - use Web Bluetooth requestDevice');
    return address;
  }

  if (noble) {
    const entry = devices.get(address);
    if (entry && entry.peripheral) {
      entry.peripheral.connect(error => {
        if (error) {
          console.log('[ble] connect error:', error);
          return;
        }
        console.log('[ble] connected to:', address);
      });
    } else {
      console.log('[ble] unknown device:', address);
    }
  } else {
    console.log('[ble] connect(' + address + ') — stub');
  }
  return address;
}

function disconnect(address) {
  if (noble) {
    const entry = devices.get(address);
    if (entry && entry.peripheral) {
      entry.peripheral.disconnect(() => {
        console.log('[ble] disconnected:', address);
      });
    }
  } else if (isBrowser && navigator.bluetooth) {
    const entry = devices.get(address);
    if (entry && entry.device && entry.device.gatt) {
      entry.device.gatt.disconnect();
    }
  } else {
    console.log('[ble] disconnect(' + address + ')');
  }
}

function readCharacteristic(deviceId, uuid) {
  if (isBrowser && navigator.bluetooth) {
    const entry = devices.get(deviceId);
    if (entry && entry.device && entry.device.gatt) {
      // Need to connect first
      return entry.device.gatt.connect().then(server => {
        return server.getPrimaryService(uuid.split('-')[0] || uuid);
      }).then(service => {
        return service.getCharacteristic(uuid);
      }).then(char => {
        return char.readValue();
      }).then(value => {
        const bytes = new Uint8Array(value.buffer);
        return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
      }).catch(err => {
        console.log('[ble] readCharacteristic error:', err.message);
        return '';
      });
    }
  }
  console.log('[ble] readCharacteristic(' + deviceId + ', ' + uuid + ') — stub');
  return '';
}

function writeCharacteristic(deviceId, uuid, value) {
  if (isBrowser && navigator.bluetooth) {
    const entry = devices.get(deviceId);
    if (entry && entry.device && entry.device.gatt) {
      entry.device.gatt.connect().then(server => {
        return server.getPrimaryService(uuid.split('-')[0] || uuid);
      }).then(service => {
        return service.getCharacteristic(uuid);
      }).then(char => {
        const bytes = new Uint8Array(value.match(/.{1,2}/g).map(b => parseInt(b, 16)));
        return char.writeValue(bytes);
      }).catch(err => {
        console.log('[ble] writeCharacteristic error:', err.message);
      });
      return;
    }
  }
  console.log('[ble] writeCharacteristic(' + deviceId + ', ' + uuid + ', ' + value + ') — stub');
}

function readRssi(deviceId) {
  const entry = devices.get(deviceId);
  if (entry) {
    const rssi = entry.rssi !== undefined && entry.rssi !== null ? entry.rssi : entry.peripheral ? entry.peripheral.rssi : null;
    if (rssi !== null) {
      return String(rssi);
    }
  }
  console.log('[ble] readRssi(' + deviceId + ') — stub');
  return '-100';
}

function deviceName(deviceId) {
  const entry = devices.get(deviceId);
  if (entry) {
    return entry.name || 'Unknown';
  }
  return 'Unknown';
}

function isConnected(deviceId) {
  if (noble) {
    const entry = devices.get(deviceId);
    if (entry && entry.peripheral) {
      return entry.peripheral.state === 'connected';
    }
    return false;
  }
  if (isBrowser && navigator.bluetooth) {
    const entry = devices.get(deviceId);
    if (entry && entry.device && entry.device.gatt) {
      return entry.device.gatt.connected;
    }
    return false;
  }
  return false;
}

function isScanning() {
  return scanning;
}

module.exports = {
  scan,
  stopScan,
  connect,
  disconnect,
  readCharacteristic,
  writeCharacteristic,
  readRssi,
  deviceName,
  isConnected,
  isScanning,
};

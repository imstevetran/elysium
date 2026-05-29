/**
 * Elysium File System — unified filesystem access for backend and browser.
 *
 * Backend (compiled binary): uses C stdlib fopen/fread/fwrite/fclose/remove/access/mkdir.
 * Client-side / Node: maps to Node's native `fs` module.
 *
 * Usage:
 *   const fs = require('elysium-lang/runtime/fs');
 *   const content = fs.readFileSync('path/to/file.txt');
 *   fs.writeFileSync('path/to/file.txt', 'hello');
 *   const exists = fs.existsSync('path/to/file.txt');
 */

// Detect environment
const isNode = typeof process !== 'undefined' && process.versions && process.versions.node;

let nodeFs = null;
if (isNode) {
  try {
    nodeFs = require('fs');
  } catch (_) {
    nodeFs = null;
  }
}

const fsObj = {
  // Read entire file as string
  readFileSync(filePath) {
    if (nodeFs) {
      return nodeFs.readFileSync(filePath, 'utf-8');
    }
    throw new Error('fs.readFileSync is not available in this environment');
  },

  // Write string to file (overwrites)
  writeFileSync(filePath, content) {
    if (nodeFs) {
      return nodeFs.writeFileSync(filePath, content, 'utf-8');
    }
    throw new Error('fs.writeFileSync is not available in this environment');
  },

  // Append string to file
  appendFileSync(filePath, content) {
    if (nodeFs) {
      return nodeFs.appendFileSync(filePath, content, 'utf-8');
    }
    throw new Error('fs.appendFileSync is not available in this environment');
  },

  // Check if path exists
  existsSync(filePath) {
    if (nodeFs) {
      return nodeFs.existsSync(filePath);
    }
    throw new Error('fs.existsSync is not available in this environment');
  },

  // Check if path is a file
  isFileSync(filePath) {
    if (nodeFs) {
      try {
        return nodeFs.statSync(filePath).isFile();
      } catch (_) {
        return false;
      }
    }
    throw new Error('fs.isFileSync is not available in this environment');
  },

  // Check if path is a directory
  isDirectorySync(filePath) {
    if (nodeFs) {
      try {
        return nodeFs.statSync(filePath).isDirectory();
      } catch (_) {
        return false;
      }
    }
    throw new Error('fs.isDirectorySync is not available in this environment');
  },

  // Remove file
  unlinkSync(filePath) {
    if (nodeFs) {
      return nodeFs.unlinkSync(filePath);
    }
    throw new Error('fs.unlinkSync is not available in this environment');
  },

  // Create directory (recursive)
  mkdirSync(dirPath) {
    if (nodeFs) {
      return nodeFs.mkdirSync(dirPath, { recursive: true });
    }
    throw new Error('fs.mkdirSync is not available in this environment');
  },

  // Remove directory
  rmdirSync(dirPath) {
    if (nodeFs) {
      return nodeFs.rmdirSync(dirPath);
    }
    throw new Error('fs.rmdirSync is not available in this environment');
  },

  // Copy file
  copyFileSync(src, dst) {
    if (nodeFs) {
      return nodeFs.copyFileSync(src, dst);
    }
    throw new Error('fs.copyFileSync is not available in this environment');
  },

  // Rename / move
  renameSync(oldPath, newPath) {
    if (nodeFs) {
      return nodeFs.renameSync(oldPath, newPath);
    }
    throw new Error('fs.renameSync is not available in this environment');
  },
};

module.exports = fsObj;

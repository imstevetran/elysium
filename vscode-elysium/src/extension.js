/**
 * Elysium Language — VS Code Extension
 *
 * Provides CLI commands (run, build, check, test, init, install, publish, update)
 * and syntax highlighting for the Elysium programming language.
 */
const vscode = require('vscode');
const path = require('path');
const cp = require('child_process');
const fs = require('fs');

/**
 * Find the elysium binary. Checks, in order:
 *  1. Local node_modules/.bin/elysium
 *  2. Global npm prefix
 *  3. PATH
 *  4. Cargo-built binary at core/target/release/elysium
 *  5. Cargo-built debug binary
 */
function findBinary() {
  const binName = process.platform === 'win32' ? 'elysium.exe' : 'elysium';

  // 1. Local node_modules
  const local = path.join(vscode.workspace.rootPath || '', 'node_modules', '.bin', binName);
  if (fs.existsSync(local)) return local;

  // 2. Global npm prefix
  try {
    const prefix = cp.execSync('npm prefix -g', { encoding: 'utf8' }).trim();
    const global = path.join(prefix, 'bin', binName);
    if (fs.existsSync(global)) return global;
  } catch (_) {}

  // 3. PATH
  const which = cp.spawnSync('which', [binName], { encoding: 'utf8' });
  if (which.status === 0 && which.stdout.trim()) return which.stdout.trim();

  // 4. Cargo-built release
  const release = path.join(__dirname, '..', '..', 'core', 'target', 'release', binName);
  if (fs.existsSync(release)) return release;

  // 5. Cargo-built debug
  const debug = path.join(__dirname, '..', '..', 'core', 'target', 'debug', binName);
  if (fs.existsSync(debug)) return debug;

  return 'elysium'; // fallback to PATH
}

/**
 * Run the elysium CLI binary with given args.
 * Shows output in a VS Code terminal.
 */
function runElysium(args) {
  const terminal = vscode.window.createTerminal({
    name: `Elysium ${args[0] || ''}`,
    cwd: vscode.workspace.rootPath,
  });
  terminal.show();
  const binary = findBinary();
  terminal.sendText(`${binary} ${args.join(' ')}`);
}

/**
 * Get the active editor's file path.
 */
function getActiveFile() {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    vscode.window.showErrorMessage('No active editor — open an .ely or .elyx file first');
    return null;
  }
  return editor.document.fileName;
}

/**
 * Ask the user for a value via input box.
 */
async function promptInput(placeHolder) {
  const result = await vscode.window.showInputBox({ placeHolder });
  return result || '';
}

/** Activate the extension — register all commands. */
function activate(context) {
  // Register commands
  const commands = [
    {
      id: 'elysium.run',
      label: 'Elysium: Run',
      args: () => {
        const file = getActiveFile();
        if (file) runElysium(['run', file]);
      },
    },
    {
      id: 'elysium.build',
      label: 'Elysium: Build',
      args: () => {
        const file = getActiveFile();
        if (file) runElysium(['build', file]);
      },
    },
    {
      id: 'elysium.check',
      label: 'Elysium: Check',
      args: () => {
        const file = getActiveFile();
        if (file) runElysium(['check', file]);
      },
    },
    {
      id: 'elysium.test',
      label: 'Elysium: Test',
      args: () => runElysium(['test']),
    },
    {
      id: 'elysium.init',
      label: 'Elysium: Init Package',
      args: async () => {
        const name = await promptInput('Package name (e.g. my-package)');
        if (name) runElysium(['init', name]);
      },
    },
    {
      id: 'elysium.install',
      label: 'Elysium: Install Package',
      args: async () => {
        const pkg = await promptInput('Package name (e.g. langchain)');
        if (pkg) runElysium(['install', pkg]);
      },
    },
    {
      id: 'elysium.publish',
      label: 'Elysium: Publish Package',
      args: () => runElysium(['publish']),
    },
    {
      id: 'elysium.update',
      label: 'Elysium: Update Packages',
      args: () => runElysium(['update']),
    },
  ];

  for (const cmd of commands) {
    const disposable = vscode.commands.registerCommand(cmd.id, cmd.args);
    context.subscriptions.push(disposable);
  }

  console.log('Elysium extension activated — CLI commands registered');
}

function deactivate() {}

module.exports = { activate, deactivate };

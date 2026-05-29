/**
 * Reactive UI engine for Elysium 2.0.
 * Mirrors elysium-rt/src/ui.rs.
 *
 * View            — Immutable UI description (virtual DOM node).
 * Style           — Optional style properties.
 * ComponentState  — Reactive state container.
 * diff            — Virtual DOM diffing engine (returns patches).
 * Patch           — DOM mutation instructions.
 * Axis            — Scroll direction.
 */

/**
 * Scroll axis enum.
 */
const Axis = Object.freeze({
  Vertical: 'vertical',
  Horizontal: 'horizontal',
});

/**
 * Style properties for a View.
 */
class Style {
  constructor(options = {}) {
    this.color = options.color || null;
    this.fontSize = options.fontSize || null;
    this.bold = options.bold || null;
  }

  equals(other) {
    if (!(other instanceof Style)) return false;
    return (
      this.color === other.color &&
      this.fontSize === other.fontSize &&
      this.bold === other.bold
    );
  }
}

/**
 * View — an immutable description of a UI element.
 * Mirrors the Rust View enum.
 */
class View {
  /**
   * Create a Text view.
   */
  static text(content, style = new Style()) {
    return { type: 'text', content, style };
  }

  /**
   * Create a Button view.
   */
  static button(label) {
    return { type: 'button', label };
  }

  /**
   * Create a TextField view.
   */
  static textField(value = '') {
    return { type: 'textField', value };
  }

  /**
   * Create an Image view.
   */
  static image(src, width = 0, height = 0) {
    return { type: 'image', src, width, height };
  }

  /**
   * Create a Column (flex column) view.
   */
  static column(children, padding = 0) {
    return { type: 'column', children: children || [], padding };
  }

  /**
   * Create a Row (flex row) view.
   */
  static row(children) {
    return { type: 'row', children: children || [] };
  }

  /**
   * Create a ScrollView.
   */
  static scrollView(axis, child) {
    return { type: 'scrollView', axis, child };
  }

  /**
   * Create a ListView.
   */
  static listView(items) {
    return { type: 'listView', items: items || [] };
  }

  /**
   * Empty placeholder view.
   */
  static empty() {
    return { type: 'empty' };
  }

  /**
   * Deep equality check between two views.
   */
  static equals(a, b) {
    if (a === b) return true;
    if (!a || !b) return false;
    if (a.type !== b.type) return false;

    switch (a.type) {
      case 'text':
        return a.content === b.content && (a.style ? a.style.equals(b.style) : !b.style);
      case 'button':
        return a.label === b.label;
      case 'textField':
        return a.value === b.value;
      case 'image':
        return a.src === b.src && a.width === b.width && a.height === b.height;
      case 'column':
      case 'row':
        if (a.children.length !== b.children.length) return false;
        return a.children.every((child, i) => View.equals(child, b.children[i]));
      case 'scrollView':
        return a.axis === b.axis && View.equals(a.child, b.child);
      case 'listView':
        if (a.items.length !== b.items.length) return false;
        return a.items.every((item, i) => item === b.items[i]);
      case 'empty':
        return true;
      default:
        return false;
    }
  }
}

/**
 * A reactive component state container.
 */
class ComponentState {
  constructor(name) {
    this.name = name;
    this.state = new Map();
    this._listeners = new Map();
  }

  set(key, value) {
    this.state.set(key, value);
    // Notify listeners
    const listeners = this._listeners.get(key);
    if (listeners) {
      listeners.forEach((fn) => fn(value));
    }
  }

  get(key) {
    return this.state.get(key);
  }

  /**
   * Subscribe to changes on a specific state key.
   */
  onChange(key, fn) {
    if (!this._listeners.has(key)) {
      this._listeners.set(key, new Set());
    }
    this._listeners.get(key).add(fn);
    return () => this._listeners.get(key).delete(fn);
  }
}

/**
 * Patch — a single DOM mutation instruction.
 */
const Patch = Object.freeze({
  /**
   * Replace an old view with a new one.
   */
  replace(oldView, newView) {
    return { type: 'replace', old: oldView, new: newView };
  },

  /**
   * Update text content at a given path index.
   */
  updateText(index, newText) {
    return { type: 'updateText', index, newText };
  },

  /**
   * Update style at a given path index.
   */
  updateStyle(index, style) {
    return { type: 'updateStyle', index, style };
  },

  /**
   * Append a child view to a parent at the given index.
   */
  appendChild(parentIndex, child) {
    return { type: 'appendChild', parentIndex, child };
  },

  /**
   * Remove a child at the given index.
   */
  removeChild(index) {
    return { type: 'removeChild', index };
  },

  /**
   * No operation.
   */
  noop() {
    return { type: 'noop' };
  },
});

/**
 * Compute the difference between two View arrays and return a list of patches.
 * Mirrors diff() from elysium-rt/src/ui.rs.
 */
function diff(oldViews, newViews) {
  const patches = [];
  const maxLen = Math.max(oldViews.length, newViews.length);

  for (let i = 0; i < maxLen; i++) {
    if (i >= oldViews.length) {
      // New view added
      patches.push(Patch.appendChild(0, newViews[i]));
    } else if (i >= newViews.length) {
      // View removed
      patches.push(Patch.removeChild(i));
    } else {
      const oldView = oldViews[i];
      const newView = newViews[i];
      if (!View.equals(oldView, newView)) {
        patches.push(Patch.replace(oldView, newView));
      }
    }
  }

  return patches;
}

module.exports = { View, Style, ComponentState, diff, Patch, Axis };

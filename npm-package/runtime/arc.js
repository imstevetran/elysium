/**
 * Automatic Reference Counting (ARC) runtime for Elysium.
 * Mirrors elysium-rt/src/arc.rs.
 *
 * Ref<T>      — Reference-counted pointer with retain/release semantics.
 * Weak<T>     — Weak reference that does not keep the object alive.
 * Unowned<T>  — Borrowed reference (assumes object outlives reference).
 */

const innerMap = new WeakMap();
let nextId = 1;

/**
 * Internal allocation for a reference-counted value.
 */
class RefInner {
  constructor(value) {
    this.id = nextId++;
    this.refCount = 1;
    this.weakCount = 0;
    this.value = value;
  }
}

/**
 * A reference-counted pointer for Elysium objects.
 */
class Ref {
  constructor(value) {
    const inner = new RefInner(value);
    innerMap.set(this, inner);
  }

  static retain(ref) {
    const inner = innerMap.get(ref);
    if (inner) {
      inner.refCount += 1;
    }
  }

  static release(ref) {
    const inner = innerMap.get(ref);
    if (!inner) return;
    if (inner.refCount <= 1) {
      innerMap.delete(ref);
    } else {
      inner.refCount -= 1;
    }
  }

  borrow() {
    const inner = innerMap.get(this);
    return inner ? inner.value : undefined;
  }

  borrowMut() {
    const inner = innerMap.get(this);
    return inner ? inner.value : undefined;
  }

  isValid() {
    return innerMap.has(this);
  }
}

/**
 * A weak reference that does not keep the object alive.
 */
class Weak {
  constructor(ref) {
    const inner = innerMap.get(ref);
    if (inner) {
      inner.weakCount += 1;
      this.ref = ref;
    }
  }

  upgrade() {
    const inner = innerMap.get(this.ref);
    if (inner && inner.refCount > 0) {
      inner.refCount += 1;
      return this.ref;
    }
    return undefined;
  }
}

/**
 * An unowned reference (no retain/release, assumes object outlives reference).
 */
class Unowned {
  constructor(ref) {
    const inner = innerMap.get(ref);
    this.value = inner ? inner.value : undefined;
  }

  get() {
    return this.value;
  }
}

module.exports = { Ref, Weak, Unowned };

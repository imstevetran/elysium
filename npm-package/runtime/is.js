/**
 * Elysium Runtime Type Check (`is` operator).
 *
 * Checks if a value is an instance of a named class at runtime.
 * Falls back to checking constructor.name.
 *
 * Usage:
 *   __is_instanceof(obj, "MyClass") → Boolean
 */
function __is_instanceof(value, typeName) {
  if (value === null || value === undefined) return false;
  // Check constructor chain for a matching name
  let proto = Object.getPrototypeOf(value);
  while (proto) {
    if (proto.constructor && proto.constructor.name === typeName) {
      return true;
    }
    proto = Object.getPrototypeOf(proto);
  }
  return false;
}

module.exports = { __is_instanceof };

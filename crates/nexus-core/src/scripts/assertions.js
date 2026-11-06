// Chai-style assertion engine for `nx.expect` (port of `AssertionEngine` in assertions.ts).
//
// This is a fragment: `sandbox_runtime.js` splices it in at `/*__ASSERTIONS__*/` inside its own
// function scope, so it uses the intrinsics captured there (`jsonStringify`, `toStr`, `isArray`,
// `objectKeys`, `objectEntries`, `ErrorCtor`) and the host bridge `call`. It has no imports and
// reaches the host only through `call('matchSchema', ...)`, which Rust answers with the `jsonschema`
// crate.

class AssertionError extends ErrorCtor {
  constructor(message) {
    super(message);
    this.name = 'AssertionError';
  }
}

function deepEqual(a, b) {
  if (a === b) return true;
  if (a === null || b === null) return false;
  if (typeof a !== typeof b) return false;
  if (typeof a !== 'object') return false;
  const aKeys = objectKeys(a);
  if (aKeys.length !== objectKeys(b).length) return false;
  return aKeys.every((key) => deepEqual(a[key], b[key]));
}

function checkEmpty(val) {
  if (val === null || val === undefined) return true;
  if (typeof val === 'string') return val.length === 0;
  if (isArray(val)) return val.length === 0;
  if (typeof val === 'object') return objectKeys(val).length === 0;
  return false;
}

class TypeChecker {
  constructor(value, negated) {
    this.value = value;
    this.negated = negated;
  }

  check(typeName) {
    let pass;
    const v = this.value;
    switch (toStr(typeName).toLowerCase()) {
      case 'string': pass = typeof v === 'string'; break;
      case 'number': pass = typeof v === 'number'; break;
      case 'boolean': pass = typeof v === 'boolean'; break;
      case 'object': pass = typeof v === 'object' && v !== null && !isArray(v); break;
      case 'array': pass = isArray(v); break;
      case 'null': pass = v === null; break;
      case 'undefined': pass = v === undefined; break;
      case 'function': pass = typeof v === 'function'; break;
      default: pass = typeof v === typeName; break;
    }
    if (this.negated ? pass : !pass) {
      throw new AssertionError(
        this.negated
          ? `expected value to not be a ${typeName}`
          : `expected a ${typeName}, got ${typeof v}`,
      );
    }
  }
}

class Expectation {
  constructor(value) {
    this.value = value;
    this.negated = false;
  }

  get to() { return this; }
  get be() { return this; }
  get have() { return this; }
  get not() { this.negated = !this.negated; return this; }
  get a() {
    const checker = new TypeChecker(this.value, this.negated);
    return (typeName) => checker.check(typeName);
  }
  get an() { return this.a; }
  get true() { this.assert(this.value === true, `expected true, got ${this.value}`, 'expected value to not be true'); }
  get false() { this.assert(this.value === false, `expected false, got ${this.value}`, 'expected value to not be false'); }
  get null() { this.assert(this.value === null, `expected null, got ${this.value}`, 'expected value to not be null'); }
  get undefined() { this.assert(this.value === undefined, `expected undefined, got ${this.value}`, 'expected value to not be undefined'); }
  get empty() { this.assert(checkEmpty(this.value), 'expected value to be empty', 'expected value not to be empty'); }

  equal(expected) {
    this.assert(
      deepEqual(this.value, expected),
      `expected ${jsonStringify(this.value)} to equal ${jsonStringify(expected)}`,
      `expected ${jsonStringify(this.value)} to not equal ${jsonStringify(expected)}`,
    );
  }

  eql(expected) { this.equal(expected); }

  include(item) {
    const v = this.value;
    let pass = false;
    if (typeof v === 'string' && typeof item === 'string') {
      pass = v.includes(item);
    } else if (isArray(v)) {
      pass = v.includes(item);
    } else if (v && typeof v === 'object') {
      pass = item !== null && typeof item === 'object'
        ? objectEntries(item).every(([k, x]) => v[k] === x)
        : false;
    }
    this.assert(
      pass,
      `expected ${jsonStringify(v)} to include ${jsonStringify(item)}`,
      `expected ${jsonStringify(v)} to not include ${jsonStringify(item)}`,
    );
  }

  contain(item) { this.include(item); }

  property(name, val) {
    const obj = this.value;
    const has = obj != null && typeof obj === 'object' && name in obj;
    if (val !== undefined && has) {
      this.assert(
        obj[name] === val,
        `expected property "${name}" to equal ${jsonStringify(val)}, got ${jsonStringify(obj[name])}`,
        `expected property "${name}" to not equal ${jsonStringify(val)}`,
      );
    } else {
      this.assert(has, `expected object to have property "${name}"`, `expected object to not have property "${name}"`);
    }
  }

  length(expected) {
    const actual = this.value?.length;
    this.assert(actual === expected, `expected length ${actual} to equal ${expected}`, `expected length ${actual} to not equal ${expected}`);
  }

  above(n) {
    this.assert(typeof this.value === 'number' && this.value > n, `expected ${this.value} to be above ${n}`, `expected ${this.value} to not be above ${n}`);
  }

  below(n) {
    this.assert(typeof this.value === 'number' && this.value < n, `expected ${this.value} to be below ${n}`, `expected ${this.value} to not be below ${n}`);
  }

  least(n) {
    this.assert(typeof this.value === 'number' && this.value >= n, `expected ${this.value} to be at least ${n}`, `expected ${this.value} to not be at least ${n}`);
  }

  most(n) {
    this.assert(typeof this.value === 'number' && this.value <= n, `expected ${this.value} to be at most ${n}`, `expected ${this.value} to not be at most ${n}`);
  }

  match(regex) {
    this.assert(typeof this.value === 'string' && regex.test(this.value), `expected "${this.value}" to match ${regex}`, `expected "${this.value}" to not match ${regex}`);
  }

  oneOf(list) {
    this.assert(
      list.includes(this.value),
      `expected ${jsonStringify(this.value)} to be one of ${jsonStringify(list)}`,
      `expected ${jsonStringify(this.value)} to not be one of ${jsonStringify(list)}`,
    );
  }

  matchSchema(schema) {
    const r = call('matchSchema', jsonStringify({ v: this.value }), jsonStringify(schema));
    this.assert(r.pass, `schema validation failed: ${r.errors}`, 'expected value to not match schema');
  }

  ok() {
    this.assert(!!this.value, 'expected value to be truthy', 'expected value to be falsy');
  }

  assert(condition, failMessage, negatedFailMessage) {
    if (this.negated ? condition : !condition) {
      throw new AssertionError(this.negated ? negatedFailMessage : failMessage);
    }
  }
}

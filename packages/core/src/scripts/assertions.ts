import Ajv from 'ajv';
import addFormats from 'ajv-formats';

export interface TestResult {
  name: string;
  passed: boolean;
  error?: string;
  duration: number;
}

/** Error thrown when a runtime assertion fails. */
export class AssertionError extends Error {
  /**
   * Creates an assertion error with a fixed `name` of `AssertionError`.
   * @param message - Failure message shown to the caller.
   * @returns A new `AssertionError` instance.
   */
  constructor(message: string) {
    super(message);
    this.name = 'AssertionError';
  }
}

/** Runs named tests, records pass/fail results, and builds expectations with JSON-schema support. */
export class AssertionEngine {
  private results: TestResult[] = [];
  private ajv: Ajv;

  /** Creates an engine with AJV configured for schema validation used by expectations. */
  constructor() {
    this.ajv = new Ajv({ allErrors: true });
    addFormats(this.ajv);
  }

  /**
   * Runs a synchronous test, recording pass/fail and duration (errors are caught, not rethrown).
   * @param name - Label stored in the result list.
   * @param fn - Test body; thrown values become failure messages.
   * @returns void
   */
  test(name: string, fn: () => void): void {
    const start = performance.now();
    try {
      fn();
      this.results.push({
        name,
        passed: true,
        duration: performance.now() - start,
      });
    } catch (err) {
      this.results.push({
        name,
        passed: false,
        error: err instanceof Error ? err.message : String(err),
        duration: performance.now() - start,
      });
    }
  }

  /**
   * Starts a fluent assertion chain for the given subject value.
   * @param value - The value under test.
   * @returns An expectation bound to this engine’s AJV instance.
   */
  expect(value: unknown): Expectation {
    return new Expectation(value, this.ajv);
  }

  /**
   * Returns a snapshot of all recorded test outcomes.
   * @returns A shallow copy of the internal results array.
   */
  getResults(): TestResult[] {
    return [...this.results];
  }

  /**
   * Removes all recorded test results.
   * @returns void
   */
  clearResults(): void {
    this.results = [];
  }
}

/** Chai-style assertion builder: chainable matchers, optional negation via `not`, and JSON-schema checks. */
class Expectation {
  private _not = false;

  /**
   * @param value - Subject under assertion.
   * @param ajv - Validator used for `matchSchema`.
   */
  constructor(
    private value: unknown,
    private ajv: Ajv,
  ) {}

  /**
   * No-op chain segment for readable `expect(x).to...` style.
   * @returns This expectation for chaining.
   */
  get to(): this {
    return this;
  }

  /**
   * No-op chain segment for readable `...to.be...` style.
   * @returns This expectation for chaining.
   */
  get be(): this {
    return this;
  }

  /**
   * No-op chain segment for readable `...to.have...` style.
   * @returns This expectation for chaining.
   */
  get have(): this {
    return this;
  }

  /**
   * Toggles negation so the next assertion expects the opposite outcome.
   * @returns This expectation for chaining (negation applies to the next check).
   */
  get not(): this {
    this._not = !this._not;
    return this;
  }

  /**
   * Overridden at module load to return a `(typeName) => void` callable for `expect(x).to.a('string')`.
   * @returns In the live API, a function that delegates to {@link TypeChecker.check}; typings may still say `TypeChecker`.
   */
  get a(): TypeChecker {
    return new TypeChecker(this.value, this._not);
  }

  /**
   * Same runtime override as {@link Expectation.a} for grammar (`an array`, etc.).
   * @returns Same callable semantics as `a`; typings may still say `TypeChecker`.
   */
  get an(): TypeChecker {
    return new TypeChecker(this.value, this._not);
  }

  /**
   * Asserts the subject is empty (null/undefined, `""`, `[]`, or `{}` with no keys).
   * @returns void
   */
  get empty(): void {
    const isEmpty = this.checkEmpty(this.value);
    this.assert(isEmpty, `expected value to be empty`, `expected value not to be empty`);
    return;
  }

  /**
   * Asserts deep structural equality for plain objects (reference equality for primitives).
   * @param expected - Value the subject should match.
   * @returns void
   */
  equal(expected: unknown): void {
    const pass = this.deepEqual(this.value, expected);
    this.assert(
      pass,
      `expected ${JSON.stringify(this.value)} to equal ${JSON.stringify(expected)}`,
      `expected ${JSON.stringify(this.value)} to not equal ${JSON.stringify(expected)}`,
    );
  }

  /**
   * Alias for {@link Expectation.equal}.
   * @param expected - Value the subject should match.
   * @returns void
   */
  eql(expected: unknown): void {
    this.equal(expected);
  }

  /**
   * Asserts substring/array membership, or that an object contains matching key/value pairs from `item`.
   * @param item - Needle string, array element, or partial object for object subjects.
   * @returns void
   */
  include(item: unknown): void {
    let pass = false;
    if (typeof this.value === 'string' && typeof item === 'string') {
      pass = this.value.includes(item);
    } else if (Array.isArray(this.value)) {
      pass = this.value.includes(item);
    } else if (this.value && typeof this.value === 'object') {
      pass = item !== null && typeof item === 'object'
        ? Object.entries(item as Record<string, unknown>).every(
            ([k, v]) => (this.value as Record<string, unknown>)[k] === v,
          )
        : false;
    }
    this.assert(
      pass,
      `expected ${JSON.stringify(this.value)} to include ${JSON.stringify(item)}`,
      `expected ${JSON.stringify(this.value)} to not include ${JSON.stringify(item)}`,
    );
  }

  /**
   * Alias for {@link Expectation.include}.
   * @param item - Same semantics as `include`.
   * @returns void
   */
  contain(item: unknown): void {
    this.include(item);
  }

  /**
   * Asserts the subject has a property, optionally equal to `val` when provided.
   * @param name - Property key to require on the object subject.
   * @param val - When set, asserts the property’s value strictly equals this.
   * @returns void
   */
  property(name: string, val?: unknown): void {
    const obj = this.value as Record<string, unknown> | null | undefined;
    const has = obj != null && typeof obj === 'object' && name in obj;
    if (val !== undefined && has) {
      const match = obj![name] === val;
      this.assert(
        match,
        `expected property "${name}" to equal ${JSON.stringify(val)}, got ${JSON.stringify(obj![name])}`,
        `expected property "${name}" to not equal ${JSON.stringify(val)}`,
      );
    } else {
      this.assert(
        has,
        `expected object to have property "${name}"`,
        `expected object to not have property "${name}"`,
      );
    }
  }

  /**
   * Asserts the subject’s `.length` equals the given number.
   * @param expected - Expected length value.
   * @returns void
   */
  length(expected: number): void {
    const actual = (this.value as { length?: number })?.length;
    const pass = actual === expected;
    this.assert(
      pass,
      `expected length ${actual} to equal ${expected}`,
      `expected length ${actual} to not equal ${expected}`,
    );
  }

  /**
   * Asserts the subject is a number strictly greater than `n`.
   * @param n - Exclusive lower bound.
   * @returns void
   */
  above(n: number): void {
    const pass = typeof this.value === 'number' && this.value > n;
    this.assert(
      pass,
      `expected ${this.value} to be above ${n}`,
      `expected ${this.value} to not be above ${n}`,
    );
  }

  /**
   * Asserts the subject is a number strictly less than `n`.
   * @param n - Exclusive upper bound.
   * @returns void
   */
  below(n: number): void {
    const pass = typeof this.value === 'number' && this.value < n;
    this.assert(
      pass,
      `expected ${this.value} to be below ${n}`,
      `expected ${this.value} to not be below ${n}`,
    );
  }

  /**
   * Asserts the subject is a number greater than or equal to `n`.
   * @param n - Inclusive lower bound.
   * @returns void
   */
  least(n: number): void {
    const pass = typeof this.value === 'number' && this.value >= n;
    this.assert(
      pass,
      `expected ${this.value} to be at least ${n}`,
      `expected ${this.value} to not be at least ${n}`,
    );
  }

  /**
   * Asserts the subject is a number less than or equal to `n`.
   * @param n - Inclusive upper bound.
   * @returns void
   */
  most(n: number): void {
    const pass = typeof this.value === 'number' && this.value <= n;
    this.assert(
      pass,
      `expected ${this.value} to be at most ${n}`,
      `expected ${this.value} to not be at most ${n}`,
    );
  }

  /**
   * Asserts the subject is a string matching the regular expression.
   * @param regex - Pattern tested against the string subject.
   * @returns void
   */
  match(regex: RegExp): void {
    const pass = typeof this.value === 'string' && regex.test(this.value);
    this.assert(
      pass,
      `expected "${this.value}" to match ${regex}`,
      `expected "${this.value}" to not match ${regex}`,
    );
  }

  /**
   * Asserts the subject is strictly included in the given list (reference equality).
   * @param list - Allowed values.
   * @returns void
   */
  oneOf(list: unknown[]): void {
    const pass = list.includes(this.value);
    this.assert(
      pass,
      `expected ${JSON.stringify(this.value)} to be one of ${JSON.stringify(list)}`,
      `expected ${JSON.stringify(this.value)} to not be one of ${JSON.stringify(list)}`,
    );
  }

  /**
   * Asserts the subject validates against a JSON Schema object using AJV.
   * @param schema - JSON Schema definition passed to AJV’s `compile`.
   * @returns void
   */
  matchSchema(schema: Record<string, unknown>): void {
    const validate = this.ajv.compile(schema);
    const pass = validate(this.value) as boolean;
    const errors = validate.errors
      ? validate.errors.map((e) => `${e.instancePath} ${e.message}`).join('; ')
      : '';
    this.assert(
      pass,
      `schema validation failed: ${errors}`,
      `expected value to not match schema`,
    );
  }

  /**
   * Asserts the subject is truthy (after boolean coercion).
   * @returns void
   */
  ok(): void {
    const pass = !!this.value;
    this.assert(pass, `expected value to be truthy`, `expected value to be falsy`);
  }

  /**
   * Asserts the subject is strictly `true`.
   * @returns void
   */
  true(): void {
    this.assert(this.value === true, `expected true, got ${this.value}`, `expected value to not be true`);
  }

  /**
   * Asserts the subject is strictly `false`.
   * @returns void
   */
  false(): void {
    this.assert(this.value === false, `expected false, got ${this.value}`, `expected value to not be false`);
  }

  /**
   * Asserts the subject is `null`.
   * @returns void
   */
  null(): void {
    this.assert(this.value === null, `expected null, got ${this.value}`, `expected value to not be null`);
  }

  /**
   * Asserts the subject is `undefined`.
   * @returns void
   */
  undefined(): void {
    this.assert(this.value === undefined, `expected undefined, got ${this.value}`, `expected value to not be undefined`);
  }

  /**
   * Throws {@link AssertionError} unless the condition matches the current negation state.
   * @param condition - Outcome of the underlying check when not negated.
   * @param failMessage - Message when the check fails without negation.
   * @param negatedFailMessage - Message when the check fails with negation active.
   * @returns void
   */
  private assert(condition: boolean, failMessage: string, negatedFailMessage: string): void {
    const pass = this._not ? !condition : condition;
    if (!pass) {
      throw new AssertionError(this._not ? negatedFailMessage : failMessage);
    }
  }

  /**
   * Compares two values for deep equality of plain objects (recursive key/value); primitives by `===`.
   * @param a - First value.
   * @param b - Second value.
   * @returns Whether `a` and `b` are considered deeply equal.
   */
  private deepEqual(a: unknown, b: unknown): boolean {
    if (a === b) return true;
    if (a === null || b === null) return false;
    if (typeof a !== typeof b) return false;
    if (typeof a !== 'object') return false;
    const aObj = a as Record<string, unknown>;
    const bObj = b as Record<string, unknown>;
    const aKeys = Object.keys(aObj);
    const bKeys = Object.keys(bObj);
    if (aKeys.length !== bKeys.length) return false;
    return aKeys.every((key) => this.deepEqual(aObj[key], bObj[key]));
  }

  /**
   * Determines whether `val` counts as empty for the `empty` assertion.
   * @param val - Subject value.
   * @returns `true` if null/undefined, empty string, empty array, or plain object with no keys.
   */
  private checkEmpty(val: unknown): boolean {
    if (val === null || val === undefined) return true;
    if (typeof val === 'string') return val.length === 0;
    if (Array.isArray(val)) return val.length === 0;
    if (typeof val === 'object') return Object.keys(val).length === 0;
    return false;
  }
}

/** Performs `typeof`-style and special-case (`null`, `array`) checks with optional negation. */
class TypeChecker {
  /**
   * @param value - Subject whose type is being verified.
   * @param negated - When true, failure means the value matched the type (used with `not`).
   */
  constructor(
    private value: unknown,
    private negated: boolean,
  ) {}

  /**
   * Asserts the subject matches the named type (case-insensitive), or throws {@link AssertionError}.
   * @param typeName - One of `string`, `number`, `boolean`, `object`, `array`, `null`, `undefined`, `function`, or a `typeof` string.
   * @returns void
   */
  check(typeName: string): void {
    let pass = false;
    switch (typeName.toLowerCase()) {
      case 'string': pass = typeof this.value === 'string'; break;
      case 'number': pass = typeof this.value === 'number'; break;
      case 'boolean': pass = typeof this.value === 'boolean'; break;
      case 'object': pass = typeof this.value === 'object' && this.value !== null && !Array.isArray(this.value); break;
      case 'array': pass = Array.isArray(this.value); break;
      case 'null': pass = this.value === null; break;
      case 'undefined': pass = this.value === undefined; break;
      case 'function': pass = typeof this.value === 'function'; break;
      default: pass = typeof this.value === typeName; break;
    }
    const actual = this.negated ? !pass : pass;
    if (!actual) {
      throw new AssertionError(
        this.negated
          ? `expected value to not be a ${typeName}`
          : `expected a ${typeName}, got ${typeof this.value}`,
      );
    }
  }
}

// Make TypeChecker callable by overriding it with a function-based approach
// The nx.expect(val).to.be.a('string') pattern calls TypeChecker as a function
// We handle this by making the `a` getter return a callable
Object.defineProperty(Expectation.prototype, 'a', {
  get() {
    const checker = new TypeChecker(this.value, this._not);
    const fn = (typeName: string) => checker.check(typeName);
    return fn;
  },
});

Object.defineProperty(Expectation.prototype, 'an', {
  get() {
    const checker = new TypeChecker(this.value, this._not);
    const fn = (typeName: string) => checker.check(typeName);
    return fn;
  },
});

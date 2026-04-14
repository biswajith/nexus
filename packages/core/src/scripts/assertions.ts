import Ajv from 'ajv';
import addFormats from 'ajv-formats';

export interface TestResult {
  name: string;
  passed: boolean;
  error?: string;
  duration: number;
}

export class AssertionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'AssertionError';
  }
}

export class AssertionEngine {
  private results: TestResult[] = [];
  private ajv: Ajv;

  constructor() {
    this.ajv = new Ajv({ allErrors: true });
    addFormats(this.ajv);
  }

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

  expect(value: unknown): Expectation {
    return new Expectation(value, this.ajv);
  }

  getResults(): TestResult[] {
    return [...this.results];
  }

  clearResults(): void {
    this.results = [];
  }
}

class Expectation {
  private _not = false;

  constructor(
    private value: unknown,
    private ajv: Ajv,
  ) {}

  get to(): this {
    return this;
  }

  get be(): this {
    return this;
  }

  get have(): this {
    return this;
  }

  get not(): this {
    this._not = !this._not;
    return this;
  }

  get a(): TypeChecker {
    return new TypeChecker(this.value, this._not);
  }

  get an(): TypeChecker {
    return new TypeChecker(this.value, this._not);
  }

  get empty(): void {
    const isEmpty = this.checkEmpty(this.value);
    this.assert(isEmpty, `expected value to be empty`, `expected value not to be empty`);
    return;
  }

  equal(expected: unknown): void {
    const pass = this.deepEqual(this.value, expected);
    this.assert(
      pass,
      `expected ${JSON.stringify(this.value)} to equal ${JSON.stringify(expected)}`,
      `expected ${JSON.stringify(this.value)} to not equal ${JSON.stringify(expected)}`,
    );
  }

  eql(expected: unknown): void {
    this.equal(expected);
  }

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

  contain(item: unknown): void {
    this.include(item);
  }

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

  length(expected: number): void {
    const actual = (this.value as { length?: number })?.length;
    const pass = actual === expected;
    this.assert(
      pass,
      `expected length ${actual} to equal ${expected}`,
      `expected length ${actual} to not equal ${expected}`,
    );
  }

  above(n: number): void {
    const pass = typeof this.value === 'number' && this.value > n;
    this.assert(
      pass,
      `expected ${this.value} to be above ${n}`,
      `expected ${this.value} to not be above ${n}`,
    );
  }

  below(n: number): void {
    const pass = typeof this.value === 'number' && this.value < n;
    this.assert(
      pass,
      `expected ${this.value} to be below ${n}`,
      `expected ${this.value} to not be below ${n}`,
    );
  }

  least(n: number): void {
    const pass = typeof this.value === 'number' && this.value >= n;
    this.assert(
      pass,
      `expected ${this.value} to be at least ${n}`,
      `expected ${this.value} to not be at least ${n}`,
    );
  }

  most(n: number): void {
    const pass = typeof this.value === 'number' && this.value <= n;
    this.assert(
      pass,
      `expected ${this.value} to be at most ${n}`,
      `expected ${this.value} to not be at most ${n}`,
    );
  }

  match(regex: RegExp): void {
    const pass = typeof this.value === 'string' && regex.test(this.value);
    this.assert(
      pass,
      `expected "${this.value}" to match ${regex}`,
      `expected "${this.value}" to not match ${regex}`,
    );
  }

  oneOf(list: unknown[]): void {
    const pass = list.includes(this.value);
    this.assert(
      pass,
      `expected ${JSON.stringify(this.value)} to be one of ${JSON.stringify(list)}`,
      `expected ${JSON.stringify(this.value)} to not be one of ${JSON.stringify(list)}`,
    );
  }

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

  ok(): void {
    const pass = !!this.value;
    this.assert(pass, `expected value to be truthy`, `expected value to be falsy`);
  }

  true(): void {
    this.assert(this.value === true, `expected true, got ${this.value}`, `expected value to not be true`);
  }

  false(): void {
    this.assert(this.value === false, `expected false, got ${this.value}`, `expected value to not be false`);
  }

  null(): void {
    this.assert(this.value === null, `expected null, got ${this.value}`, `expected value to not be null`);
  }

  undefined(): void {
    this.assert(this.value === undefined, `expected undefined, got ${this.value}`, `expected value to not be undefined`);
  }

  private assert(condition: boolean, failMessage: string, negatedFailMessage: string): void {
    const pass = this._not ? !condition : condition;
    if (!pass) {
      throw new AssertionError(this._not ? negatedFailMessage : failMessage);
    }
  }

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

  private checkEmpty(val: unknown): boolean {
    if (val === null || val === undefined) return true;
    if (typeof val === 'string') return val.length === 0;
    if (Array.isArray(val)) return val.length === 0;
    if (typeof val === 'object') return Object.keys(val).length === 0;
    return false;
  }
}

class TypeChecker {
  constructor(
    private value: unknown,
    private negated: boolean,
  ) {}

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

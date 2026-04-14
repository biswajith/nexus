import { describe, it, expect } from 'vitest';
import { AssertionEngine, AssertionError } from './assertions.js';

describe('AssertionEngine — property-style assertions (issue #3)', () => {
  let engine: AssertionEngine;

  function expectation(value: unknown) {
    return engine.expect(value);
  }

  it('.to.be.true passes for true', () => {
    engine = new AssertionEngine();
    engine.test('should pass', () => {
      expectation(true).to.be.true;
    });
    const results = engine.getResults();
    expect(results).toHaveLength(1);
    expect(results[0]!.passed).toBe(true);
  });

  it('.to.be.true fails for false', () => {
    engine = new AssertionEngine();
    engine.test('should fail', () => {
      expectation(false).to.be.true;
    });
    const results = engine.getResults();
    expect(results).toHaveLength(1);
    expect(results[0]!.passed).toBe(false);
    expect(results[0]!.error).toContain('expected true');
  });

  it('.to.be.true fails for truthy non-boolean values', () => {
    engine = new AssertionEngine();
    engine.test('number 1', () => { expectation(1).to.be.true; });
    engine.test('non-empty string', () => { expectation('yes').to.be.true; });
    engine.test('object', () => { expectation({}).to.be.true; });
    const results = engine.getResults();
    expect(results.every((r) => !r.passed)).toBe(true);
  });

  it('.to.be.false passes for false', () => {
    engine = new AssertionEngine();
    engine.test('should pass', () => {
      expectation(false).to.be.false;
    });
    const results = engine.getResults();
    expect(results[0]!.passed).toBe(true);
  });

  it('.to.be.false fails for true', () => {
    engine = new AssertionEngine();
    engine.test('should fail', () => {
      expectation(true).to.be.false;
    });
    const results = engine.getResults();
    expect(results[0]!.passed).toBe(false);
    expect(results[0]!.error).toContain('expected false');
  });

  it('.to.be.null passes for null', () => {
    engine = new AssertionEngine();
    engine.test('should pass', () => {
      expectation(null).to.be.null;
    });
    const results = engine.getResults();
    expect(results[0]!.passed).toBe(true);
  });

  it('.to.be.null fails for non-null', () => {
    engine = new AssertionEngine();
    engine.test('should fail', () => {
      expectation(0).to.be.null;
    });
    const results = engine.getResults();
    expect(results[0]!.passed).toBe(false);
  });

  it('.to.be.undefined passes for undefined', () => {
    engine = new AssertionEngine();
    engine.test('should pass', () => {
      expectation(undefined).to.be.undefined;
    });
    const results = engine.getResults();
    expect(results[0]!.passed).toBe(true);
  });

  it('.to.be.undefined fails for defined values', () => {
    engine = new AssertionEngine();
    engine.test('should fail', () => {
      expectation(null).to.be.undefined;
    });
    const results = engine.getResults();
    expect(results[0]!.passed).toBe(false);
  });

  it('.not.to.be.true passes for false', () => {
    engine = new AssertionEngine();
    engine.test('should pass', () => {
      expectation(false).not.to.be.true;
    });
    const results = engine.getResults();
    expect(results[0]!.passed).toBe(true);
  });

  it('.not.to.be.false passes for true', () => {
    engine = new AssertionEngine();
    engine.test('should pass', () => {
      expectation(true).not.to.be.false;
    });
    const results = engine.getResults();
    expect(results[0]!.passed).toBe(true);
  });

  it('reproduces the exact scenario from issue #3', () => {
    engine = new AssertionEngine();

    const jsonData = { success: false };
    engine.test('Validate success true', () => {
      expectation(jsonData.success).to.be.true;
    });
    const results = engine.getResults();
    expect(results).toHaveLength(1);
    expect(results[0]!.passed).toBe(false);
    expect(results[0]!.error).toContain('expected true, got false');
  });

  it('direct property access throws AssertionError (not silently passes)', () => {
    engine = new AssertionEngine();
    expect(() => {
      expectation(false).to.be.true;
    }).toThrow(AssertionError);
  });
});

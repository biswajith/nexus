import { describe, it, expect } from 'vitest';
import {
  transpilePostmanToNexus,
  transpileNexusToPostman,
  detectScriptPlatform,
} from './script-transpiler.js';

describe('transpilePostmanToNexus', () => {
  describe('variable scope replacements', () => {
    it('converts pm.environment.get to nx.environment.get', () => {
      const { script } = transpilePostmanToNexus('pm.environment.get("key")');
      expect(script).toBe('nx.environment.get("key")');
    });

    it('converts pm.environment.set to nx.environment.set', () => {
      const { script } = transpilePostmanToNexus('pm.environment.set("key", "val")');
      expect(script).toBe('nx.environment.set("key", "val")');
    });

    it('converts pm.environment.unset', () => {
      const { script } = transpilePostmanToNexus('pm.environment.unset("key")');
      expect(script).toBe('nx.environment.unset("key")');
    });

    it('converts pm.environment.toObject', () => {
      const { script } = transpilePostmanToNexus('pm.environment.toObject()');
      expect(script).toBe('nx.environment.toObject()');
    });

    it('converts pm.collectionVariables.get', () => {
      const { script } = transpilePostmanToNexus('pm.collectionVariables.get("k")');
      expect(script).toBe('nx.collectionVariables.get("k")');
    });

    it('converts pm.collectionVariables.set', () => {
      const { script } = transpilePostmanToNexus('pm.collectionVariables.set("k", "v")');
      expect(script).toBe('nx.collectionVariables.set("k", "v")');
    });

    it('converts pm.collectionVariables.unset', () => {
      const { script } = transpilePostmanToNexus('pm.collectionVariables.unset("k")');
      expect(script).toBe('nx.collectionVariables.unset("k")');
    });

    it('converts pm.collectionVariables.toObject', () => {
      const { script } = transpilePostmanToNexus('pm.collectionVariables.toObject()');
      expect(script).toBe('nx.collectionVariables.toObject()');
    });

    it('converts pm.globals.get', () => {
      const { script } = transpilePostmanToNexus('pm.globals.get("k")');
      expect(script).toBe('nx.globals.get("k")');
    });

    it('converts pm.globals.set', () => {
      const { script } = transpilePostmanToNexus('pm.globals.set("k", "v")');
      expect(script).toBe('nx.globals.set("k", "v")');
    });

    it('converts pm.globals.unset', () => {
      const { script } = transpilePostmanToNexus('pm.globals.unset("k")');
      expect(script).toBe('nx.globals.unset("k")');
    });

    it('converts pm.globals.toObject', () => {
      const { script } = transpilePostmanToNexus('pm.globals.toObject()');
      expect(script).toBe('nx.globals.toObject()');
    });

    it('converts pm.variables.get', () => {
      const { script } = transpilePostmanToNexus('pm.variables.get("k")');
      expect(script).toBe('nx.variables.get("k")');
    });

    it('converts pm.variables.set', () => {
      const { script } = transpilePostmanToNexus('pm.variables.set("k", "v")');
      expect(script).toBe('nx.variables.set("k", "v")');
    });

    it('converts pm.variables.replaceIn', () => {
      const { script } = transpilePostmanToNexus('pm.variables.replaceIn("{{url}}")');
      expect(script).toBe('nx.variables.replaceIn("{{url}}")');
    });
  });

  describe('test and assertion replacements', () => {
    it('converts pm.test to nx.test', () => {
      const { script } = transpilePostmanToNexus('pm.test("Status is 200", function() { });');
      expect(script).toBe('nx.test("Status is 200", function() { });');
    });

    it('converts pm.expect to nx.expect', () => {
      const { script } = transpilePostmanToNexus('pm.expect(true).to.be.true;');
      expect(script).toBe('nx.expect(true).to.be.true;');
    });
  });

  describe('response property replacements', () => {
    it('converts pm.response.code', () => {
      const { script } = transpilePostmanToNexus('pm.response.code');
      expect(script).toBe('nx.response.code');
    });

    it('converts pm.response.status', () => {
      const { script } = transpilePostmanToNexus('pm.response.status');
      expect(script).toBe('nx.response.status');
    });

    it('converts pm.response.headers', () => {
      const { script } = transpilePostmanToNexus('pm.response.headers');
      expect(script).toBe('nx.response.headers');
    });

    it('converts pm.response.json', () => {
      const { script } = transpilePostmanToNexus('pm.response.json()');
      expect(script).toBe('nx.response.json()');
    });

    it('converts pm.response.text', () => {
      const { script } = transpilePostmanToNexus('pm.response.text()');
      expect(script).toBe('nx.response.text()');
    });

    it('converts pm.response.responseTime', () => {
      const { script } = transpilePostmanToNexus('pm.response.responseTime');
      expect(script).toBe('nx.response.responseTime');
    });

    it('converts pm.response.responseSize to nx.response.size', () => {
      const { script } = transpilePostmanToNexus('pm.response.responseSize');
      expect(script).toBe('nx.response.size');
    });
  });

  describe('request property replacements', () => {
    it('converts pm.request.url', () => {
      const { script } = transpilePostmanToNexus('pm.request.url');
      expect(script).toBe('nx.request.url');
    });

    it('converts pm.request.headers', () => {
      const { script } = transpilePostmanToNexus('pm.request.headers');
      expect(script).toBe('nx.request.headers');
    });

    it('converts pm.request.method', () => {
      const { script } = transpilePostmanToNexus('pm.request.method');
      expect(script).toBe('nx.request.method');
    });

    it('converts pm.request.body', () => {
      const { script } = transpilePostmanToNexus('pm.request.body');
      expect(script).toBe('nx.request.body');
    });
  });

  describe('execution control', () => {
    it('converts pm.execution.setNextRequest', () => {
      const { script } = transpilePostmanToNexus('pm.execution.setNextRequest("Next")');
      expect(script).toBe('nx.execution.setNextRequest("Next")');
    });

    it('converts postman.setNextRequest', () => {
      const { script } = transpilePostmanToNexus('postman.setNextRequest("Next")');
      expect(script).toBe('nx.execution.setNextRequest("Next")');
    });
  });

  describe('legacy Postman object conversions', () => {
    it('converts postman.setEnvironmentVariable', () => {
      const { script } = transpilePostmanToNexus('postman.setEnvironmentVariable("k", "v")');
      expect(script).toBe('nx.environment.set("k", "v")');
    });

    it('converts postman.getEnvironmentVariable', () => {
      const { script } = transpilePostmanToNexus('postman.getEnvironmentVariable("k")');
      expect(script).toBe('nx.environment.get("k")');
    });

    it('converts postman.clearEnvironmentVariable', () => {
      const { script } = transpilePostmanToNexus('postman.clearEnvironmentVariable("k")');
      expect(script).toBe('nx.environment.unset("k")');
    });

    it('converts postman.setGlobalVariable', () => {
      const { script } = transpilePostmanToNexus('postman.setGlobalVariable("k", "v")');
      expect(script).toBe('nx.globals.set("k", "v")');
    });

    it('converts postman.getGlobalVariable', () => {
      const { script } = transpilePostmanToNexus('postman.getGlobalVariable("k")');
      expect(script).toBe('nx.globals.get("k")');
    });

    it('converts postman.clearGlobalVariable', () => {
      const { script } = transpilePostmanToNexus('postman.clearGlobalVariable("k")');
      expect(script).toBe('nx.globals.unset("k")');
    });
  });

  describe('legacy tests[] pattern', () => {
    it('converts tests["name"] = expr to nx.test()', () => {
      const { script, warnings } = transpilePostmanToNexus('tests["Status is 200"] = responseCode.code === 200;');
      expect(script).toContain('nx.test("Status is 200"');
      expect(script).toContain('nx.expect(');
      expect(script).toContain('.to.be.true()');
      expect(warnings.some((w) => w.includes('legacy tests['))).toBe(true);
    });
  });

  describe('pm.response.to.have/be patterns', () => {
    it('converts pm.response.to.have.status(200)', () => {
      const { script } = transpilePostmanToNexus('pm.response.to.have.status(200)');
      expect(script).toBe('nx.expect(nx.response.code).to.equal(200)');
    });

    it('converts pm.response.to.be.ok', () => {
      const { script } = transpilePostmanToNexus('pm.response.to.be.ok');
      expect(script).toBe('nx.expect(nx.response.code).to.be.below(300)');
    });

    it('converts pm.response.to.be.success', () => {
      const { script } = transpilePostmanToNexus('pm.response.to.be.success');
      expect(script).toBe('nx.expect(nx.response.code).to.be.below(300)');
    });

    it('converts pm.response.to.be.clientError', () => {
      const { script } = transpilePostmanToNexus('pm.response.to.be.clientError');
      expect(script).toContain('above(399)');
      expect(script).toContain('below(500)');
    });

    it('converts pm.response.to.be.serverError', () => {
      const { script } = transpilePostmanToNexus('pm.response.to.be.serverError');
      expect(script).toContain('above(499)');
    });

    it('converts pm.response.to.be.rateLimited', () => {
      const { script } = transpilePostmanToNexus('pm.response.to.be.rateLimited');
      expect(script).toContain('equal(429)');
    });

    it('converts pm.response.to.be.badRequest', () => {
      const { script } = transpilePostmanToNexus('pm.response.to.be.badRequest');
      expect(script).toContain('equal(400)');
    });

    it('converts pm.response.to.be.unauthorised', () => {
      const { script } = transpilePostmanToNexus('pm.response.to.be.unauthorised');
      expect(script).toContain('equal(401)');
    });

    it('converts pm.response.to.be.unauthorized', () => {
      const { script } = transpilePostmanToNexus('pm.response.to.be.unauthorized');
      expect(script).toContain('equal(401)');
    });

    it('converts pm.response.to.be.forbidden', () => {
      const { script } = transpilePostmanToNexus('pm.response.to.be.forbidden');
      expect(script).toContain('equal(403)');
    });

    it('converts pm.response.to.be.notFound', () => {
      const { script } = transpilePostmanToNexus('pm.response.to.be.notFound');
      expect(script).toContain('equal(404)');
    });

    it('converts pm.response.to.have.header("name")', () => {
      const { script } = transpilePostmanToNexus('pm.response.to.have.header("Content-Type")');
      expect(script).toContain('nx.response.headers');
      expect(script).toContain('not.be.undefined');
    });

    it('converts pm.response.to.have.jsonBody', () => {
      const { script } = transpilePostmanToNexus('pm.response.to.have.jsonBody');
      expect(script).toContain('nx.response.json()');
      expect(script).toContain('not.throw');
    });
  });

  describe('.has() conversions', () => {
    it('converts pm.environment.has(key)', () => {
      const { script } = transpilePostmanToNexus('pm.environment.has("key")');
      expect(script).toBe('(nx.environment.get("key") !== undefined)');
    });

    it('converts pm.globals.has(key)', () => {
      const { script } = transpilePostmanToNexus('pm.globals.has("key")');
      expect(script).toBe('(nx.globals.get("key") !== undefined)');
    });

    it('converts pm.variables.has(key)', () => {
      const { script } = transpilePostmanToNexus('pm.variables.has("key")');
      expect(script).toBe('(nx.variables.get("key") !== undefined)');
    });
  });

  describe('legacy global variables', () => {
    it('converts responseBody to nx.response.text()', () => {
      const { script, warnings } = transpilePostmanToNexus('var body = responseBody;');
      expect(script).toContain('nx.response.text()');
      expect(warnings.some((w) => w.includes('responseBody'))).toBe(true);
    });

    it('converts responseCode.code to nx.response.code', () => {
      const { script, warnings } = transpilePostmanToNexus('var code = responseCode.code;');
      expect(script).toContain('nx.response.code');
      expect(warnings.some((w) => w.includes('responseCode'))).toBe(true);
    });
  });

  describe('unsupported API warnings', () => {
    it('warns about pm.sendRequest', () => {
      const { script, warnings } = transpilePostmanToNexus('pm.sendRequest("https://example.com", function(err, res) {});');
      expect(warnings.some((w) => w.includes('pm.sendRequest'))).toBe(true);
      expect(script).toContain('nx.sendRequest');
      expect(script).toContain('TODO');
    });

    it('warns about pm.info', () => {
      const { warnings } = transpilePostmanToNexus('console.log(pm.info.requestName);');
      expect(warnings.some((w) => w.includes('pm.info'))).toBe(true);
    });

    it('warns about pm.iterationData', () => {
      const { script, warnings } = transpilePostmanToNexus('pm.iterationData.get("key")');
      expect(warnings.some((w) => w.includes('pm.iterationData'))).toBe(true);
      expect(script).toContain('nx.variables.get');
    });

    it('warns about pm.cookies', () => {
      const { warnings } = transpilePostmanToNexus('pm.cookies.get("session")');
      expect(warnings.some((w) => w.includes('pm.cookies'))).toBe(true);
    });

    it('warns about pm.vault', () => {
      const { warnings } = transpilePostmanToNexus('pm.vault.get("secret")');
      expect(warnings.some((w) => w.includes('pm.vault'))).toBe(true);
    });

    it('warns about pm.visualizer', () => {
      const { script, warnings } = transpilePostmanToNexus('pm.visualizer.set(template, data)');
      expect(warnings.some((w) => w.includes('pm.visualizer'))).toBe(true);
      expect(script).toContain('TODO');
    });

    it('warns about require() calls', () => {
      const { warnings } = transpilePostmanToNexus('const _ = require("lodash");');
      expect(warnings.some((w) => w.includes('require()'))).toBe(true);
    });

    it('warns about xml2Json', () => {
      const { warnings } = transpilePostmanToNexus('var obj = xml2Json(responseBody);');
      expect(warnings.some((w) => w.includes('xml2Json'))).toBe(true);
    });
  });

  describe('multi-line scripts', () => {
    it('transpiles a full script with multiple patterns', () => {
      const input = [
        'pm.test("Status is 200", function() {',
        '  pm.response.to.have.status(200);',
        '  var json = pm.response.json();',
        '  pm.expect(json.name).to.equal("Alice");',
        '});',
        'pm.environment.set("userId", json.id);',
      ].join('\n');

      const { script } = transpilePostmanToNexus(input);
      expect(script).toContain('nx.test');
      expect(script).toContain('nx.expect(nx.response.code).to.equal(200)');
      expect(script).toContain('nx.response.json()');
      expect(script).toContain('nx.expect(json.name)');
      expect(script).toContain('nx.environment.set');
    });
  });

  describe('empty and no-op inputs', () => {
    it('returns empty string for empty script', () => {
      const { script, warnings } = transpilePostmanToNexus('');
      expect(script).toBe('');
      expect(warnings).toHaveLength(0);
    });

    it('returns script unchanged when no pm patterns', () => {
      const input = 'console.log("hello");';
      const { script } = transpilePostmanToNexus(input);
      expect(script).toBe(input);
    });
  });
});

describe('transpileNexusToPostman', () => {
  describe('simple replacements', () => {
    it('converts nx.environment.get to pm.environment.get', () => {
      const { script } = transpileNexusToPostman('nx.environment.get("key")');
      expect(script).toBe('pm.environment.get("key")');
    });

    it('converts nx.environment.set to pm.environment.set', () => {
      const { script } = transpileNexusToPostman('nx.environment.set("k", "v")');
      expect(script).toBe('pm.environment.set("k", "v")');
    });

    it('converts nx.environment.unset', () => {
      const { script } = transpileNexusToPostman('nx.environment.unset("k")');
      expect(script).toBe('pm.environment.unset("k")');
    });

    it('converts nx.environment.toObject', () => {
      const { script } = transpileNexusToPostman('nx.environment.toObject()');
      expect(script).toBe('pm.environment.toObject()');
    });

    it('converts nx.collectionVariables.get', () => {
      const { script } = transpileNexusToPostman('nx.collectionVariables.get("k")');
      expect(script).toBe('pm.collectionVariables.get("k")');
    });

    it('converts nx.globals.get', () => {
      const { script } = transpileNexusToPostman('nx.globals.get("k")');
      expect(script).toBe('pm.globals.get("k")');
    });

    it('converts nx.globals.set', () => {
      const { script } = transpileNexusToPostman('nx.globals.set("k", "v")');
      expect(script).toBe('pm.globals.set("k", "v")');
    });

    it('converts nx.variables.get', () => {
      const { script } = transpileNexusToPostman('nx.variables.get("k")');
      expect(script).toBe('pm.variables.get("k")');
    });

    it('converts nx.variables.set', () => {
      const { script } = transpileNexusToPostman('nx.variables.set("k", "v")');
      expect(script).toBe('pm.variables.set("k", "v")');
    });

    it('converts nx.test to pm.test', () => {
      const { script } = transpileNexusToPostman('nx.test("ok", () => {});');
      expect(script).toBe('pm.test("ok", () => {});');
    });

    it('converts nx.expect to pm.expect', () => {
      const { script } = transpileNexusToPostman('nx.expect(val).to.equal(1);');
      expect(script).toBe('pm.expect(val).to.equal(1);');
    });

    it('converts nx.response.code', () => {
      const { script } = transpileNexusToPostman('nx.response.code');
      expect(script).toBe('pm.response.code');
    });

    it('converts nx.response.status', () => {
      const { script } = transpileNexusToPostman('nx.response.status');
      expect(script).toBe('pm.response.status');
    });

    it('converts nx.response.json', () => {
      const { script } = transpileNexusToPostman('nx.response.json()');
      expect(script).toBe('pm.response.json()');
    });

    it('converts nx.response.text', () => {
      const { script } = transpileNexusToPostman('nx.response.text()');
      expect(script).toBe('pm.response.text()');
    });

    it('converts nx.response.responseTime', () => {
      const { script } = transpileNexusToPostman('nx.response.responseTime');
      expect(script).toBe('pm.response.responseTime');
    });

    it('converts nx.response.size to pm.response.responseSize', () => {
      const { script } = transpileNexusToPostman('nx.response.size');
      expect(script).toBe('pm.response.responseSize');
    });

    it('converts nx.request.url', () => {
      const { script } = transpileNexusToPostman('nx.request.url');
      expect(script).toBe('pm.request.url');
    });

    it('converts nx.request.method', () => {
      const { script } = transpileNexusToPostman('nx.request.method');
      expect(script).toBe('pm.request.method');
    });

    it('converts nx.execution.setNextRequest', () => {
      const { script } = transpileNexusToPostman('nx.execution.setNextRequest("Next")');
      expect(script).toBe('pm.execution.setNextRequest("Next")');
    });
  });

  describe('status check pattern folding', () => {
    it('folds nx.expect(nx.response.code).to.equal(200) to pm.response.to.have.status(200)', () => {
      const { script } = transpileNexusToPostman('nx.expect(nx.response.code).to.equal(200)');
      expect(script).toBe('pm.response.to.have.status(200)');
    });

    it('folds for other status codes', () => {
      const { script } = transpileNexusToPostman('nx.expect(nx.response.code).to.equal(404)');
      expect(script).toBe('pm.response.to.have.status(404)');
    });
  });

  describe('nx.execution.skipRequest', () => {
    it('comments out skipRequest and warns', () => {
      const { script, warnings } = transpileNexusToPostman('nx.execution.skipRequest()');
      expect(script).toContain('NEXUS');
      expect(script).toContain('no Postman equivalent');
      expect(warnings.some((w) => w.includes('skipRequest'))).toBe(true);
    });
  });

  describe('edge cases', () => {
    it('returns empty string for empty input', () => {
      const { script, warnings } = transpileNexusToPostman('');
      expect(script).toBe('');
      expect(warnings).toHaveLength(0);
    });

    it('returns unchanged script when no nx patterns', () => {
      const { script } = transpileNexusToPostman('console.log("hello");');
      expect(script).toBe('console.log("hello");');
    });
  });
});

describe('detectScriptPlatform', () => {
  it('detects postman scripts by pm. usage', () => {
    expect(detectScriptPlatform('pm.test("ok", function() { pm.response.code; });')).toBe('postman');
  });

  it('detects nexus scripts by nx. usage', () => {
    expect(detectScriptPlatform('nx.test("ok", () => { nx.response.code; });')).toBe('nexus');
  });

  it('detects postman when pm count exceeds nx count', () => {
    expect(detectScriptPlatform('pm.test(); pm.expect(); nx.response.code;')).toBe('postman');
  });

  it('detects nexus when nx count exceeds pm count', () => {
    expect(detectScriptPlatform('nx.test(); nx.expect(); pm.response.code;')).toBe('nexus');
  });

  it('detects postman from legacy postman.setEnvironmentVariable', () => {
    expect(detectScriptPlatform('postman.setEnvironmentVariable("k", "v");')).toBe('postman');
  });

  it('detects postman from legacy postman.getGlobalVariable', () => {
    expect(detectScriptPlatform('postman.getGlobalVariable("k");')).toBe('postman');
  });

  it('detects postman from legacy postman.clearEnvironmentVariable', () => {
    expect(detectScriptPlatform('postman.clearEnvironmentVariable("k");')).toBe('postman');
  });

  it('detects postman from legacy tests[] pattern', () => {
    expect(detectScriptPlatform('tests["status"] = true;')).toBe('postman');
  });

  it('prefers legacy postman detection even with nx patterns', () => {
    expect(detectScriptPlatform('tests["ok"] = true; nx.response.code;')).toBe('postman');
  });

  it('defaults to postman when pm and nx counts are equal but > 0', () => {
    expect(detectScriptPlatform('pm.test(); nx.test();')).toBe('postman');
  });

  it('returns unknown when no patterns match', () => {
    expect(detectScriptPlatform('console.log("hello");')).toBe('unknown');
  });

  it('returns unknown for empty string', () => {
    expect(detectScriptPlatform('')).toBe('unknown');
  });

  it('returns postman when only pm. appears (equal to 0 nx)', () => {
    expect(detectScriptPlatform('pm.test();')).toBe('postman');
  });
});

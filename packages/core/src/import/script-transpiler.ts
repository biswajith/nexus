export interface TranspileResult {
  script: string;
  warnings: string[];
}

const PM_TO_NX_SIMPLE: [RegExp, string][] = [
  // Variable scopes
  [/\bpm\.environment\.get\b/g, 'nx.environment.get'],
  [/\bpm\.environment\.set\b/g, 'nx.environment.set'],
  [/\bpm\.environment\.unset\b/g, 'nx.environment.unset'],
  [/\bpm\.environment\.toObject\b/g, 'nx.environment.toObject'],
  [/\bpm\.collectionVariables\.get\b/g, 'nx.collectionVariables.get'],
  [/\bpm\.collectionVariables\.set\b/g, 'nx.collectionVariables.set'],
  [/\bpm\.collectionVariables\.unset\b/g, 'nx.collectionVariables.unset'],
  [/\bpm\.collectionVariables\.toObject\b/g, 'nx.collectionVariables.toObject'],
  [/\bpm\.globals\.get\b/g, 'nx.globals.get'],
  [/\bpm\.globals\.set\b/g, 'nx.globals.set'],
  [/\bpm\.globals\.unset\b/g, 'nx.globals.unset'],
  [/\bpm\.globals\.toObject\b/g, 'nx.globals.toObject'],
  [/\bpm\.variables\.get\b/g, 'nx.variables.get'],
  [/\bpm\.variables\.set\b/g, 'nx.variables.set'],
  [/\bpm\.variables\.replaceIn\b/g, 'nx.variables.replaceIn'],

  // Test and assertions
  [/\bpm\.test\b/g, 'nx.test'],
  [/\bpm\.expect\b/g, 'nx.expect'],

  // Response
  [/\bpm\.response\.code\b/g, 'nx.response.code'],
  [/\bpm\.response\.status\b/g, 'nx.response.status'],
  [/\bpm\.response\.headers\b/g, 'nx.response.headers'],
  [/\bpm\.response\.json\b/g, 'nx.response.json'],
  [/\bpm\.response\.text\b/g, 'nx.response.text'],
  [/\bpm\.response\.responseTime\b/g, 'nx.response.responseTime'],
  [/\bpm\.response\.responseSize\b/g, 'nx.response.size'],

  // Request
  [/\bpm\.request\.url\b/g, 'nx.request.url'],
  [/\bpm\.request\.headers\b/g, 'nx.request.headers'],
  [/\bpm\.request\.method\b/g, 'nx.request.method'],
  [/\bpm\.request\.body\b/g, 'nx.request.body'],

  // Execution control
  [/\bpm\.execution\.setNextRequest\b/g, 'nx.execution.setNextRequest'],
  [/\bpostman\.setNextRequest\b/g, 'nx.execution.setNextRequest'],

  // Legacy Postman object (very old scripts)
  [/\bpostman\.setEnvironmentVariable\b/g, 'nx.environment.set'],
  [/\bpostman\.getEnvironmentVariable\b/g, 'nx.environment.get'],
  [/\bpostman\.clearEnvironmentVariable\b/g, 'nx.environment.unset'],
  [/\bpostman\.setGlobalVariable\b/g, 'nx.globals.set'],
  [/\bpostman\.getGlobalVariable\b/g, 'nx.globals.get'],
  [/\bpostman\.clearGlobalVariable\b/g, 'nx.globals.unset'],
];

const NX_TO_PM_SIMPLE: [RegExp, string][] = [
  [/\bnx\.environment\.get\b/g, 'pm.environment.get'],
  [/\bnx\.environment\.set\b/g, 'pm.environment.set'],
  [/\bnx\.environment\.unset\b/g, 'pm.environment.unset'],
  [/\bnx\.environment\.toObject\b/g, 'pm.environment.toObject'],
  [/\bnx\.collectionVariables\.get\b/g, 'pm.collectionVariables.get'],
  [/\bnx\.collectionVariables\.set\b/g, 'pm.collectionVariables.set'],
  [/\bnx\.collectionVariables\.unset\b/g, 'pm.collectionVariables.unset'],
  [/\bnx\.collectionVariables\.toObject\b/g, 'pm.collectionVariables.toObject'],
  [/\bnx\.globals\.get\b/g, 'pm.globals.get'],
  [/\bnx\.globals\.set\b/g, 'pm.globals.set'],
  [/\bnx\.globals\.unset\b/g, 'pm.globals.unset'],
  [/\bnx\.globals\.toObject\b/g, 'pm.globals.toObject'],
  [/\bnx\.variables\.get\b/g, 'pm.variables.get'],
  [/\bnx\.variables\.set\b/g, 'pm.variables.set'],
  [/\bnx\.test\b/g, 'pm.test'],
  [/\bnx\.expect\b/g, 'pm.expect'],
  [/\bnx\.response\.code\b/g, 'pm.response.code'],
  [/\bnx\.response\.status\b/g, 'pm.response.status'],
  [/\bnx\.response\.headers\b/g, 'pm.response.headers'],
  [/\bnx\.response\.json\b/g, 'pm.response.json'],
  [/\bnx\.response\.text\b/g, 'pm.response.text'],
  [/\bnx\.response\.responseTime\b/g, 'pm.response.responseTime'],
  [/\bnx\.response\.size\b/g, 'pm.response.responseSize'],
  [/\bnx\.request\.url\b/g, 'pm.request.url'],
  [/\bnx\.request\.headers\b/g, 'pm.request.headers'],
  [/\bnx\.request\.method\b/g, 'pm.request.method'],
  [/\bnx\.request\.body\b/g, 'pm.request.body'],
  [/\bnx\.execution\.setNextRequest\b/g, 'pm.execution.setNextRequest'],
  [/\bnx\.execution\.skipRequest\b/g, '/* NEXUS: nx.execution.skipRequest() has no Postman equivalent */'],
];

/**
 * Rewrites a Postman script (`pm.*`, legacy globals) into Nexus (`nx.*`) form, including
 * pattern-specific conversions and warnings for APIs that need manual follow-up.
 *
 * @param script - Source script text using Postman APIs or legacy syntax.
 * @returns The transpiled script plus any non-fatal conversion warnings.
 */
export function transpilePostmanToNexus(script: string): TranspileResult {
  const warnings: string[] = [];
  let result = script;

  // Handle legacy tests["name"] = boolean pattern (very old Postman syntax)
  result = result.replace(
    /tests\["([^"]+)"\]\s*=\s*(.+);/g,
    (_match, name: string, expr: string) => {
      warnings.push(`Converted legacy tests["${name}"] syntax to nx.test()`);
      return `nx.test("${name}", () => { nx.expect(${expr.trim()}).to.be.true(); });`;
    },
  );

  // Handle pm.response.to.have.status(code) → nx.expect(nx.response.code).to.equal(code)
  result = result.replace(
    /pm\.response\.to\.have\.status\((\d+)\)/g,
    'nx.expect(nx.response.code).to.equal($1)',
  );

  // Handle pm.response.to.be.ok → status < 300
  result = result.replace(
    /pm\.response\.to\.be\.ok\b/g,
    'nx.expect(nx.response.code).to.be.below(300)',
  );

  // Handle pm.response.to.be.success → same as ok
  result = result.replace(
    /pm\.response\.to\.be\.success\b/g,
    'nx.expect(nx.response.code).to.be.below(300)',
  );

  // Handle pm.response.to.be.clientError
  result = result.replace(
    /pm\.response\.to\.be\.clientError\b/g,
    'nx.expect(nx.response.code).to.be.above(399).and.to.be.below(500)',
  );

  // Handle pm.response.to.be.serverError
  result = result.replace(
    /pm\.response\.to\.be\.serverError\b/g,
    'nx.expect(nx.response.code).to.be.above(499)',
  );

  // Handle pm.response.to.be.rateLimited
  result = result.replace(
    /pm\.response\.to\.be\.rateLimited\b/g,
    'nx.expect(nx.response.code).to.equal(429)',
  );

  // Handle pm.response.to.be.badRequest
  result = result.replace(
    /pm\.response\.to\.be\.badRequest\b/g,
    'nx.expect(nx.response.code).to.equal(400)',
  );

  // Handle pm.response.to.be.unauthorised / unauthorized
  result = result.replace(
    /pm\.response\.to\.be\.unauthori[sz]ed\b/g,
    'nx.expect(nx.response.code).to.equal(401)',
  );

  // Handle pm.response.to.be.forbidden
  result = result.replace(
    /pm\.response\.to\.be\.forbidden\b/g,
    'nx.expect(nx.response.code).to.equal(403)',
  );

  // Handle pm.response.to.be.notFound
  result = result.replace(
    /pm\.response\.to\.be\.notFound\b/g,
    'nx.expect(nx.response.code).to.equal(404)',
  );

  // Handle pm.response.to.have.header("name") → check header existence
  result = result.replace(
    /pm\.response\.to\.have\.header\("([^"]+)"\)/g,
    'nx.expect(nx.response.headers["$1".toLowerCase()]).to.not.be.undefined()',
  );

  // Handle pm.response.to.have.jsonBody → parse check
  result = result.replace(
    /pm\.response\.to\.have\.jsonBody\b/g,
    'nx.expect(() => nx.response.json()).to.not.throw',
  );

  // Handle pm.environment.has(key) → nx.environment.get(key) !== undefined
  result = result.replace(
    /pm\.environment\.has\(([^)]+)\)/g,
    '(nx.environment.get($1) !== undefined)',
  );

  // Handle pm.globals.has(key)
  result = result.replace(
    /pm\.globals\.has\(([^)]+)\)/g,
    '(nx.globals.get($1) !== undefined)',
  );

  // Handle pm.variables.has(key)
  result = result.replace(
    /pm\.variables\.has\(([^)]+)\)/g,
    '(nx.variables.get($1) !== undefined)',
  );

  // Apply simple replacements
  for (const [pattern, replacement] of PM_TO_NX_SIMPLE) {
    result = result.replace(pattern, replacement);
  }

  // Warn about unsupported APIs that couldn't be converted
  if (/\bpm\.sendRequest\b/.test(result)) {
    result = result.replace(
      /\bpm\.sendRequest\b/g,
      '/* TODO: pm.sendRequest is not yet supported in Nexus */ nx.sendRequest',
    );
    warnings.push('pm.sendRequest() found — nx.sendRequest() is not yet implemented. These calls will need manual review.');
  }

  if (/\bpm\.info\b/.test(result)) {
    warnings.push('pm.info (requestName, requestId, etc.) is not available in Nexus. References will be left as-is and will fail at runtime.');
  }

  if (/\bpm\.iterationData\b/.test(result)) {
    result = result.replace(
      /\bpm\.iterationData\.get\(([^)]+)\)/g,
      '/* iterationData: use data file variables */ nx.variables.get($1)',
    );
    warnings.push('pm.iterationData converted to nx.variables.get(). Data file variables are accessed through the standard variable scope in Nexus.');
  }

  if (/\bpm\.cookies\b/.test(result)) {
    warnings.push('pm.cookies is not available in Nexus scripts. Cookie access from scripts is not yet supported.');
  }

  if (/\bpm\.vault\b/.test(result)) {
    warnings.push('pm.vault (Postman Vault) is not available in Nexus. Use secret-type environment variables instead.');
  }

  if (/\bpm\.visualizer\b/.test(result)) {
    warnings.push('pm.visualizer is not yet supported in Nexus. Visualizer calls will be left as comments.');
    result = result.replace(/\bpm\.visualizer\.set\b/g, '/* TODO: visualizer not supported */ // pm.visualizer.set');
  }

  if (/\brequire\s*\(/.test(result)) {
    warnings.push('require() calls found. Nexus sandbox does not support require(). Libraries like lodash, moment, crypto-js are not available.');
  }

  // Handle xml2Json (Postman built-in)
  if (/\bxml2Json\b/.test(result)) {
    warnings.push('xml2Json() is a Postman built-in and is not available in Nexus. You will need to handle XML parsing differently.');
  }

  // Handle responseBody (legacy Postman global)
  result = result.replace(/\bresponseBody\b/g, 'nx.response.text()');
  if (script.includes('responseBody')) {
    warnings.push('Legacy "responseBody" global converted to nx.response.text().');
  }

  // Handle responseCode (legacy Postman global)
  result = result.replace(/\bresponseCode\.code\b/g, 'nx.response.code');
  if (script.includes('responseCode')) {
    warnings.push('Legacy "responseCode" global converted to nx.response.code.');
  }

  // Handle responseHeaders (legacy Postman global)
  if (script !== result || /\bresponseHeaders\b/.test(script)) {
    result = result.replace(/\bresponseHeaders\b/g, 'nx.response.headers');
  }

  // Handle responseTime (legacy Postman global)
  result = result.replace(/(?<!\.)(\b)responseTime\b(?![\w.])/g, '$1nx.response.responseTime');

  return { script: result, warnings };
}

/**
 * Rewrites a Nexus script (`nx.*`) into Postman-style `pm.*` calls using inverse symbol mapping
 * and a few structural substitutions where applicable.
 *
 * @param script - Source script text using Nexus APIs.
 * @returns The Postman-oriented script plus warnings (e.g. unsupported inverse mappings).
 */
export function transpileNexusToPostman(script: string): TranspileResult {
  const warnings: string[] = [];
  let result = script;

  for (const [pattern, replacement] of NX_TO_PM_SIMPLE) {
    result = result.replace(pattern, replacement);
  }

  // nx.expect(nx.response.code).to.equal(X) → pm.response.to.have.status(X) where simple
  result = result.replace(
    /pm\.expect\(pm\.response\.code\)\.to\.equal\((\d+)\)/g,
    'pm.response.to.have.status($1)',
  );

  if (script.includes('nx.execution.skipRequest')) {
    warnings.push('nx.execution.skipRequest() has no direct Postman equivalent. It has been commented out.');
  }

  return { script: result, warnings };
}

/**
 * Heuristically classifies a script as Postman, Nexus, or unknown from `pm.` / `nx.` usage
 * counts and legacy Postman patterns (`postman.*Variable`, `tests[...]`).
 *
 * @param script - Script text to inspect.
 * @returns `'postman'`, `'nexus'`, or `'unknown'` based on detected conventions.
 */
export function detectScriptPlatform(script: string): 'postman' | 'nexus' | 'unknown' {
  const pmMatches = (script.match(/\bpm\./g) || []).length;
  const nxMatches = (script.match(/\bnx\./g) || []).length;
  const legacyPostman = /\bpostman\.(set|get|clear)(Environment|Global)Variable\b/.test(script);
  const legacyTests = /\btests\[/.test(script);

  if (legacyPostman || legacyTests) return 'postman';
  if (pmMatches > nxMatches) return 'postman';
  if (nxMatches > pmMatches) return 'nexus';
  if (pmMatches > 0) return 'postman';
  return 'unknown';
}

//! `pm.*` <-> `nx.*` script rewriting (port of `script-transpiler.ts`). Purely regex based, like the TS.

use regex::{NoExpand, Regex};
use std::sync::LazyLock;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranspileResult {
    pub script: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptPlatform {
    Postman,
    Nexus,
    Unknown,
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static regex")
}

fn word_table(pairs: &[(&str, &str)]) -> Vec<(Regex, String)> {
    pairs
        .iter()
        .map(|(from, to)| {
            (
                re(&format!(r"\b{}\b", regex::escape(from))),
                (*to).to_string(),
            )
        })
        .collect()
}

static PM_TO_NX_SIMPLE: LazyLock<Vec<(Regex, String)>> = LazyLock::new(|| {
    word_table(&[
        ("pm.environment.get", "nx.environment.get"),
        ("pm.environment.set", "nx.environment.set"),
        ("pm.environment.unset", "nx.environment.unset"),
        ("pm.environment.toObject", "nx.environment.toObject"),
        ("pm.collectionVariables.get", "nx.collectionVariables.get"),
        ("pm.collectionVariables.set", "nx.collectionVariables.set"),
        (
            "pm.collectionVariables.unset",
            "nx.collectionVariables.unset",
        ),
        (
            "pm.collectionVariables.toObject",
            "nx.collectionVariables.toObject",
        ),
        ("pm.globals.get", "nx.globals.get"),
        ("pm.globals.set", "nx.globals.set"),
        ("pm.globals.unset", "nx.globals.unset"),
        ("pm.globals.toObject", "nx.globals.toObject"),
        ("pm.variables.get", "nx.variables.get"),
        ("pm.variables.set", "nx.variables.set"),
        ("pm.variables.replaceIn", "nx.variables.replaceIn"),
        ("pm.test", "nx.test"),
        ("pm.expect", "nx.expect"),
        ("pm.response.code", "nx.response.code"),
        ("pm.response.status", "nx.response.status"),
        ("pm.response.headers", "nx.response.headers"),
        ("pm.response.json", "nx.response.json"),
        ("pm.response.text", "nx.response.text"),
        ("pm.response.responseTime", "nx.response.responseTime"),
        ("pm.response.responseSize", "nx.response.size"),
        ("pm.request.url", "nx.request.url"),
        ("pm.request.headers", "nx.request.headers"),
        ("pm.request.method", "nx.request.method"),
        ("pm.request.body", "nx.request.body"),
        ("pm.execution.setNextRequest", "nx.execution.setNextRequest"),
        ("postman.setNextRequest", "nx.execution.setNextRequest"),
        ("postman.setEnvironmentVariable", "nx.environment.set"),
        ("postman.getEnvironmentVariable", "nx.environment.get"),
        ("postman.clearEnvironmentVariable", "nx.environment.unset"),
        ("postman.setGlobalVariable", "nx.globals.set"),
        ("postman.getGlobalVariable", "nx.globals.get"),
        ("postman.clearGlobalVariable", "nx.globals.unset"),
    ])
});

static NX_TO_PM_SIMPLE: LazyLock<Vec<(Regex, String)>> = LazyLock::new(|| {
    word_table(&[
        ("nx.environment.get", "pm.environment.get"),
        ("nx.environment.set", "pm.environment.set"),
        ("nx.environment.unset", "pm.environment.unset"),
        ("nx.environment.toObject", "pm.environment.toObject"),
        ("nx.collectionVariables.get", "pm.collectionVariables.get"),
        ("nx.collectionVariables.set", "pm.collectionVariables.set"),
        (
            "nx.collectionVariables.unset",
            "pm.collectionVariables.unset",
        ),
        (
            "nx.collectionVariables.toObject",
            "pm.collectionVariables.toObject",
        ),
        ("nx.globals.get", "pm.globals.get"),
        ("nx.globals.set", "pm.globals.set"),
        ("nx.globals.unset", "pm.globals.unset"),
        ("nx.globals.toObject", "pm.globals.toObject"),
        ("nx.variables.get", "pm.variables.get"),
        ("nx.variables.set", "pm.variables.set"),
        ("nx.test", "pm.test"),
        ("nx.expect", "pm.expect"),
        ("nx.response.code", "pm.response.code"),
        ("nx.response.status", "pm.response.status"),
        ("nx.response.headers", "pm.response.headers"),
        ("nx.response.json", "pm.response.json"),
        ("nx.response.text", "pm.response.text"),
        ("nx.response.responseTime", "pm.response.responseTime"),
        ("nx.response.size", "pm.response.responseSize"),
        ("nx.request.url", "pm.request.url"),
        ("nx.request.headers", "pm.request.headers"),
        ("nx.request.method", "pm.request.method"),
        ("nx.request.body", "pm.request.body"),
        ("nx.execution.setNextRequest", "pm.execution.setNextRequest"),
        (
            "nx.execution.skipRequest",
            "/* NEXUS: nx.execution.skipRequest() has no Postman equivalent */",
        ),
    ])
});

/// Pattern-specific rewrites applied before the simple table. Replacement strings use `${n}` groups.
static PM_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    let t = |p: &str, r: &'static str| (re(p), r);
    vec![
        t(
            r"pm\.response\.to\.have\.status\((\d+)\)",
            "nx.expect(nx.response.code).to.equal(${1})",
        ),
        t(
            r"pm\.response\.to\.be\.ok\b",
            "nx.expect(nx.response.code).to.be.below(300)",
        ),
        t(
            r"pm\.response\.to\.be\.success\b",
            "nx.expect(nx.response.code).to.be.below(300)",
        ),
        t(
            r"pm\.response\.to\.be\.clientError\b",
            "nx.expect(nx.response.code).to.be.above(399).and.to.be.below(500)",
        ),
        t(
            r"pm\.response\.to\.be\.serverError\b",
            "nx.expect(nx.response.code).to.be.above(499)",
        ),
        t(
            r"pm\.response\.to\.be\.rateLimited\b",
            "nx.expect(nx.response.code).to.equal(429)",
        ),
        t(
            r"pm\.response\.to\.be\.badRequest\b",
            "nx.expect(nx.response.code).to.equal(400)",
        ),
        t(
            r"pm\.response\.to\.be\.unauthori[sz]ed\b",
            "nx.expect(nx.response.code).to.equal(401)",
        ),
        t(
            r"pm\.response\.to\.be\.forbidden\b",
            "nx.expect(nx.response.code).to.equal(403)",
        ),
        t(
            r"pm\.response\.to\.be\.notFound\b",
            "nx.expect(nx.response.code).to.equal(404)",
        ),
        t(
            r#"pm\.response\.to\.have\.header\("([^"]+)"\)"#,
            r#"nx.expect(nx.response.headers["${1}".toLowerCase()]).to.not.be.undefined()"#,
        ),
        t(
            r"pm\.response\.to\.have\.jsonBody\b",
            "nx.expect(() => nx.response.json()).to.not.throw",
        ),
        t(
            r"pm\.environment\.has\(([^)]+)\)",
            "(nx.environment.get(${1}) !== undefined)",
        ),
        t(
            r"pm\.globals\.has\(([^)]+)\)",
            "(nx.globals.get(${1}) !== undefined)",
        ),
        t(
            r"pm\.variables\.has\(([^)]+)\)",
            "(nx.variables.get(${1}) !== undefined)",
        ),
    ]
});

static LEGACY_TESTS: LazyLock<Regex> = LazyLock::new(|| re(r#"tests\["([^"]+)"\]\s*=\s*(.+);"#));
static PM_SEND_REQUEST: LazyLock<Regex> = LazyLock::new(|| re(r"\bpm\.sendRequest\b"));
static PM_INFO: LazyLock<Regex> = LazyLock::new(|| re(r"\bpm\.info\b"));
static PM_ITERATION_DATA: LazyLock<Regex> = LazyLock::new(|| re(r"\bpm\.iterationData\b"));
static PM_ITERATION_GET: LazyLock<Regex> =
    LazyLock::new(|| re(r"\bpm\.iterationData\.get\(([^)]+)\)"));
static PM_COOKIES: LazyLock<Regex> = LazyLock::new(|| re(r"\bpm\.cookies\b"));
static PM_VAULT: LazyLock<Regex> = LazyLock::new(|| re(r"\bpm\.vault\b"));
static PM_VISUALIZER: LazyLock<Regex> = LazyLock::new(|| re(r"\bpm\.visualizer\b"));
static PM_VISUALIZER_SET: LazyLock<Regex> = LazyLock::new(|| re(r"\bpm\.visualizer\.set\b"));
static REQUIRE_CALL: LazyLock<Regex> = LazyLock::new(|| re(r"\brequire\s*\("));
static XML2JSON: LazyLock<Regex> = LazyLock::new(|| re(r"\bxml2Json\b"));
static RESPONSE_BODY: LazyLock<Regex> = LazyLock::new(|| re(r"\bresponseBody\b"));
static RESPONSE_CODE: LazyLock<Regex> = LazyLock::new(|| re(r"\bresponseCode\.code\b"));
static RESPONSE_HEADERS: LazyLock<Regex> = LazyLock::new(|| re(r"\bresponseHeaders\b"));
static RESPONSE_TIME: LazyLock<Regex> = LazyLock::new(|| re(r"\bresponseTime\b"));
static PM_DOT: LazyLock<Regex> = LazyLock::new(|| re(r"\bpm\."));
static NX_DOT: LazyLock<Regex> = LazyLock::new(|| re(r"\bnx\."));
static LEGACY_POSTMAN: LazyLock<Regex> =
    LazyLock::new(|| re(r"\bpostman\.(set|get|clear)(Environment|Global)Variable\b"));
static LEGACY_TESTS_INDEX: LazyLock<Regex> = LazyLock::new(|| re(r"\btests\["));
static NX_STATUS_FOLD: LazyLock<Regex> =
    LazyLock::new(|| re(r"pm\.expect\(pm\.response\.code\)\.to\.equal\((\d+)\)"));

fn replace_literal(text: &str, re: &Regex, with: &str) -> String {
    re.replace_all(text, NoExpand(with)).into_owned()
}

/// Rewrites a Postman script into Nexus form; APIs without an equivalent produce warnings.
pub fn transpile_postman_to_nexus(script: &str) -> TranspileResult {
    let mut warnings: Vec<String> = vec![];
    let mut result = script.to_string();

    // Legacy `tests["name"] = expr;`
    result = LEGACY_TESTS
        .replace_all(&result, |caps: &regex::Captures| {
            let name = &caps[1];
            warnings.push(format!(
                "Converted legacy tests[\"{name}\"] syntax to nx.test()"
            ));
            format!(
                "nx.test(\"{name}\", () => {{ nx.expect({}).to.be.true(); }});",
                caps[2].trim()
            )
        })
        .into_owned();

    for (pattern, replacement) in PM_PATTERNS.iter() {
        result = pattern.replace_all(&result, *replacement).into_owned();
    }
    for (pattern, replacement) in PM_TO_NX_SIMPLE.iter() {
        result = replace_literal(&result, pattern, replacement);
    }

    if PM_SEND_REQUEST.is_match(&result) {
        result = replace_literal(
            &result,
            &PM_SEND_REQUEST,
            "/* TODO: pm.sendRequest is not yet supported in Nexus */ nx.sendRequest",
        );
        warnings.push("pm.sendRequest() found — nx.sendRequest() is not yet implemented. These calls will need manual review.".into());
    }
    if PM_INFO.is_match(&result) {
        warnings.push("pm.info (requestName, requestId, etc.) is not available in Nexus. References will be left as-is and will fail at runtime.".into());
    }
    if PM_ITERATION_DATA.is_match(&result) {
        result = PM_ITERATION_GET
            .replace_all(
                &result,
                "/* iterationData: use data file variables */ nx.variables.get(${1})",
            )
            .into_owned();
        warnings.push("pm.iterationData converted to nx.variables.get(). Data file variables are accessed through the standard variable scope in Nexus.".into());
    }
    if PM_COOKIES.is_match(&result) {
        warnings.push("pm.cookies is not available in Nexus scripts. Cookie access from scripts is not yet supported.".into());
    }
    if PM_VAULT.is_match(&result) {
        warnings.push("pm.vault (Postman Vault) is not available in Nexus. Use secret-type environment variables instead.".into());
    }
    if PM_VISUALIZER.is_match(&result) {
        warnings.push("pm.visualizer is not yet supported in Nexus. Visualizer calls will be left as comments.".into());
        result = replace_literal(
            &result,
            &PM_VISUALIZER_SET,
            "/* TODO: visualizer not supported */ // pm.visualizer.set",
        );
    }
    if REQUIRE_CALL.is_match(&result) {
        warnings.push("require() calls found. Nexus sandbox does not support require(). Libraries like lodash, moment, crypto-js are not available.".into());
    }
    if XML2JSON.is_match(&result) {
        warnings.push("xml2Json() is a Postman built-in and is not available in Nexus. You will need to handle XML parsing differently.".into());
    }

    result = replace_literal(&result, &RESPONSE_BODY, "nx.response.text()");
    if script.contains("responseBody") {
        warnings.push("Legacy \"responseBody\" global converted to nx.response.text().".into());
    }

    result = replace_literal(&result, &RESPONSE_CODE, "nx.response.code");
    if script.contains("responseCode") {
        warnings.push("Legacy \"responseCode\" global converted to nx.response.code.".into());
    }

    if script != result || RESPONSE_HEADERS.is_match(script) {
        result = replace_literal(&result, &RESPONSE_HEADERS, "nx.response.headers");
    }

    result = replace_bare_response_time(&result);

    TranspileResult {
        script: result,
        warnings,
    }
}

/// JS `/(?<!\.)\bresponseTime\b(?![\w.])/g` -> `nx.response.responseTime` (no look-around in `regex`).
fn replace_bare_response_time(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in RESPONSE_TIME.find_iter(text) {
        let before = text[..m.start()].chars().next_back();
        let after = text[m.end()..].chars().next();
        let blocked = before == Some('.')
            || after.is_some_and(|c| c == '.' || c == '_' || c.is_ascii_alphanumeric());
        if !blocked {
            out.push_str(&text[last..m.start()]);
            out.push_str("nx.response.responseTime");
            last = m.end();
        }
    }
    out.push_str(&text[last..]);
    out
}

/// Rewrites a Nexus script into Postman `pm.*` form.
pub fn transpile_nexus_to_postman(script: &str) -> TranspileResult {
    let mut warnings = vec![];
    let mut result = script.to_string();

    for (pattern, replacement) in NX_TO_PM_SIMPLE.iter() {
        result = replace_literal(&result, pattern, replacement);
    }
    result = NX_STATUS_FOLD
        .replace_all(&result, "pm.response.to.have.status(${1})")
        .into_owned();

    if script.contains("nx.execution.skipRequest") {
        warnings.push("nx.execution.skipRequest() has no direct Postman equivalent. It has been commented out.".into());
    }
    TranspileResult {
        script: result,
        warnings,
    }
}

/// Heuristic: is this script written against `pm.*`, `nx.*`, or neither?
pub fn detect_script_platform(script: &str) -> ScriptPlatform {
    let pm = PM_DOT.find_iter(script).count();
    let nx = NX_DOT.find_iter(script).count();
    if LEGACY_POSTMAN.is_match(script) || LEGACY_TESTS_INDEX.is_match(script) {
        return ScriptPlatform::Postman;
    }
    match (pm, nx) {
        _ if pm > nx => ScriptPlatform::Postman,
        _ if nx > pm => ScriptPlatform::Nexus,
        _ if pm > 0 => ScriptPlatform::Postman,
        _ => ScriptPlatform::Unknown,
    }
}

#[cfg(test)]
mod tests;

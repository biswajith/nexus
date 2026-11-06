use super::*;

fn pm(s: &str) -> TranspileResult {
    transpile_postman_to_nexus(s)
}

fn nx(s: &str) -> TranspileResult {
    transpile_nexus_to_postman(s)
}

fn has_warning(r: &TranspileResult, needle: &str) -> bool {
    r.warnings.iter().any(|w| w.contains(needle))
}
#[test]
fn pm_to_nx_exact_rewrites() {
    let cases: &[(&str, &str)] = &[
        (
            r#"pm.environment.get("key")"#,
            r#"nx.environment.get("key")"#,
        ),
        (
            r#"pm.environment.set("key", "val")"#,
            r#"nx.environment.set("key", "val")"#,
        ),
        (
            r#"pm.environment.unset("key")"#,
            r#"nx.environment.unset("key")"#,
        ),
        (
            r#"pm.environment.toObject()"#,
            r#"nx.environment.toObject()"#,
        ),
        (
            r#"pm.collectionVariables.get("k")"#,
            r#"nx.collectionVariables.get("k")"#,
        ),
        (
            r#"pm.collectionVariables.set("k", "v")"#,
            r#"nx.collectionVariables.set("k", "v")"#,
        ),
        (
            r#"pm.collectionVariables.unset("k")"#,
            r#"nx.collectionVariables.unset("k")"#,
        ),
        (
            r#"pm.collectionVariables.toObject()"#,
            r#"nx.collectionVariables.toObject()"#,
        ),
        (r#"pm.globals.get("k")"#, r#"nx.globals.get("k")"#),
        (r#"pm.globals.set("k", "v")"#, r#"nx.globals.set("k", "v")"#),
        (r#"pm.globals.unset("k")"#, r#"nx.globals.unset("k")"#),
        (r#"pm.globals.toObject()"#, r#"nx.globals.toObject()"#),
        (r#"pm.variables.get("k")"#, r#"nx.variables.get("k")"#),
        (
            r#"pm.variables.set("k", "v")"#,
            r#"nx.variables.set("k", "v")"#,
        ),
        (
            r#"pm.variables.replaceIn("{{url}}")"#,
            r#"nx.variables.replaceIn("{{url}}")"#,
        ),
        (
            r#"pm.test("Status is 200", function() { });"#,
            r#"nx.test("Status is 200", function() { });"#,
        ),
        (
            r#"pm.expect(true).to.be.true;"#,
            r#"nx.expect(true).to.be.true;"#,
        ),
        (r#"pm.response.code"#, r#"nx.response.code"#),
        (r#"pm.response.status"#, r#"nx.response.status"#),
        (r#"pm.response.headers"#, r#"nx.response.headers"#),
        (r#"pm.response.json()"#, r#"nx.response.json()"#),
        (r#"pm.response.text()"#, r#"nx.response.text()"#),
        (r#"pm.response.responseTime"#, r#"nx.response.responseTime"#),
        (r#"pm.response.responseSize"#, r#"nx.response.size"#),
        (r#"pm.request.url"#, r#"nx.request.url"#),
        (r#"pm.request.headers"#, r#"nx.request.headers"#),
        (r#"pm.request.method"#, r#"nx.request.method"#),
        (r#"pm.request.body"#, r#"nx.request.body"#),
        (
            r#"pm.execution.setNextRequest("Next")"#,
            r#"nx.execution.setNextRequest("Next")"#,
        ),
        (
            r#"postman.setNextRequest("Next")"#,
            r#"nx.execution.setNextRequest("Next")"#,
        ),
        (
            r#"postman.setEnvironmentVariable("k", "v")"#,
            r#"nx.environment.set("k", "v")"#,
        ),
        (
            r#"postman.getEnvironmentVariable("k")"#,
            r#"nx.environment.get("k")"#,
        ),
        (
            r#"postman.clearEnvironmentVariable("k")"#,
            r#"nx.environment.unset("k")"#,
        ),
        (
            r#"postman.setGlobalVariable("k", "v")"#,
            r#"nx.globals.set("k", "v")"#,
        ),
        (
            r#"postman.getGlobalVariable("k")"#,
            r#"nx.globals.get("k")"#,
        ),
        (
            r#"postman.clearGlobalVariable("k")"#,
            r#"nx.globals.unset("k")"#,
        ),
        (
            r#"pm.response.to.have.status(200)"#,
            r#"nx.expect(nx.response.code).to.equal(200)"#,
        ),
        (
            r#"pm.response.to.be.ok"#,
            r#"nx.expect(nx.response.code).to.be.below(300)"#,
        ),
        (
            r#"pm.response.to.be.success"#,
            r#"nx.expect(nx.response.code).to.be.below(300)"#,
        ),
        (
            r#"pm.environment.has("key")"#,
            r#"(nx.environment.get("key") !== undefined)"#,
        ),
        (
            r#"pm.globals.has("key")"#,
            r#"(nx.globals.get("key") !== undefined)"#,
        ),
        (
            r#"pm.variables.has("key")"#,
            r#"(nx.variables.get("key") !== undefined)"#,
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(pm(input).script, *expected, "input: {input}");
    }
}

#[test]
fn nx_to_pm_exact_rewrites() {
    let cases: &[(&str, &str)] = &[
        (
            r#"nx.environment.get("key")"#,
            r#"pm.environment.get("key")"#,
        ),
        (
            r#"nx.environment.set("k", "v")"#,
            r#"pm.environment.set("k", "v")"#,
        ),
        (
            r#"nx.environment.unset("k")"#,
            r#"pm.environment.unset("k")"#,
        ),
        (
            r#"nx.environment.toObject()"#,
            r#"pm.environment.toObject()"#,
        ),
        (
            r#"nx.collectionVariables.get("k")"#,
            r#"pm.collectionVariables.get("k")"#,
        ),
        (r#"nx.globals.get("k")"#, r#"pm.globals.get("k")"#),
        (r#"nx.globals.set("k", "v")"#, r#"pm.globals.set("k", "v")"#),
        (r#"nx.variables.get("k")"#, r#"pm.variables.get("k")"#),
        (
            r#"nx.variables.set("k", "v")"#,
            r#"pm.variables.set("k", "v")"#,
        ),
        (r#"nx.test("ok", () => {});"#, r#"pm.test("ok", () => {});"#),
        (
            r#"nx.expect(val).to.equal(1);"#,
            r#"pm.expect(val).to.equal(1);"#,
        ),
        (r#"nx.response.code"#, r#"pm.response.code"#),
        (r#"nx.response.status"#, r#"pm.response.status"#),
        (r#"nx.response.json()"#, r#"pm.response.json()"#),
        (r#"nx.response.text()"#, r#"pm.response.text()"#),
        (r#"nx.response.responseTime"#, r#"pm.response.responseTime"#),
        (r#"nx.response.size"#, r#"pm.response.responseSize"#),
        (r#"nx.request.url"#, r#"pm.request.url"#),
        (r#"nx.request.method"#, r#"pm.request.method"#),
        (
            r#"nx.execution.setNextRequest("Next")"#,
            r#"pm.execution.setNextRequest("Next")"#,
        ),
        (
            r#"nx.expect(nx.response.code).to.equal(200)"#,
            r#"pm.response.to.have.status(200)"#,
        ),
        (
            r#"nx.expect(nx.response.code).to.equal(404)"#,
            r#"pm.response.to.have.status(404)"#,
        ),
        (r#"console.log("hello");"#, r#"console.log("hello");"#),
    ];
    for (input, expected) in cases {
        assert_eq!(nx(input).script, *expected, "input: {input}");
    }
}

#[test]
fn legacy_tests_array_pattern() {
    let r = pm(r#"tests["Status is 200"] = responseCode.code === 200;"#);
    assert!(r.script.contains(r#"nx.test("Status is 200""#));
    assert!(r.script.contains("nx.expect("));
    assert!(r.script.contains(".to.be.true()"));
    assert!(has_warning(&r, "legacy tests["));
}

#[test]
fn response_status_patterns() {
    assert_eq!(
        pm("pm.response.to.have.status(200)").script,
        "nx.expect(nx.response.code).to.equal(200)"
    );
    assert_eq!(
        pm("pm.response.to.be.ok").script,
        "nx.expect(nx.response.code).to.be.below(300)"
    );
    assert_eq!(
        pm("pm.response.to.be.success").script,
        "nx.expect(nx.response.code).to.be.below(300)"
    );
    let s = pm("pm.response.to.be.clientError").script;
    assert!(s.contains("above(399)") && s.contains("below(500)"));
    assert!(pm("pm.response.to.be.serverError")
        .script
        .contains("above(499)"));
    assert!(pm("pm.response.to.be.rateLimited")
        .script
        .contains("equal(429)"));
    assert!(pm("pm.response.to.be.badRequest")
        .script
        .contains("equal(400)"));
    assert!(pm("pm.response.to.be.unauthorised")
        .script
        .contains("equal(401)"));
    assert!(pm("pm.response.to.be.unauthorized")
        .script
        .contains("equal(401)"));
    assert!(pm("pm.response.to.be.forbidden")
        .script
        .contains("equal(403)"));
    assert!(pm("pm.response.to.be.notFound")
        .script
        .contains("equal(404)"));
    let s = pm(r#"pm.response.to.have.header("Content-Type")"#).script;
    assert!(s.contains("nx.response.headers") && s.contains("not.be.undefined"));
    let s = pm("pm.response.to.have.jsonBody").script;
    assert!(s.contains("nx.response.json()") && s.contains("not.throw"));
}

#[test]
fn has_conversions() {
    assert_eq!(
        pm(r#"pm.environment.has("key")"#).script,
        r#"(nx.environment.get("key") !== undefined)"#
    );
    assert_eq!(
        pm(r#"pm.globals.has("key")"#).script,
        r#"(nx.globals.get("key") !== undefined)"#
    );
    assert_eq!(
        pm(r#"pm.variables.has("key")"#).script,
        r#"(nx.variables.get("key") !== undefined)"#
    );
}

#[test]
fn legacy_globals() {
    let r = pm("var body = responseBody;");
    assert!(r.script.contains("nx.response.text()"));
    assert!(has_warning(&r, "responseBody"));
    let r = pm("var code = responseCode.code;");
    assert!(r.script.contains("nx.response.code"));
    assert!(has_warning(&r, "responseCode"));
}

#[test]
fn bare_response_time_is_rewritten_but_not_member_access() {
    assert_eq!(
        pm("var t = responseTime;").script,
        "var t = nx.response.responseTime;"
    );
    assert_eq!(pm("foo.responseTime").script, "foo.responseTime");
    assert_eq!(pm("responseTime.x").script, "responseTime.x");
}

#[test]
fn unsupported_api_warnings() {
    let r = pm(r#"pm.sendRequest("https://example.com", function(err, res) {});"#);
    assert!(has_warning(&r, "pm.sendRequest"));
    assert!(r.script.contains("nx.sendRequest") && r.script.contains("TODO"));

    assert!(has_warning(
        &pm("console.log(pm.info.requestName);"),
        "pm.info"
    ));

    let r = pm(r#"pm.iterationData.get("key")"#);
    assert!(has_warning(&r, "pm.iterationData"));
    assert!(r.script.contains("nx.variables.get"));

    assert!(has_warning(
        &pm(r#"pm.cookies.get("session")"#),
        "pm.cookies"
    ));
    assert!(has_warning(&pm(r#"pm.vault.get("secret")"#), "pm.vault"));

    let r = pm("pm.visualizer.set(template, data)");
    assert!(has_warning(&r, "pm.visualizer"));
    assert!(r.script.contains("TODO"));

    assert!(has_warning(
        &pm(r#"const _ = require("lodash");"#),
        "require()"
    ));
    assert!(has_warning(
        &pm("var obj = xml2Json(responseBody);"),
        "xml2Json"
    ));
}

#[test]
fn multi_line_script() {
    let input = [
        r#"pm.test("Status is 200", function() {"#,
        "  pm.response.to.have.status(200);",
        "  var json = pm.response.json();",
        r#"  pm.expect(json.name).to.equal("Alice");"#,
        "});",
        r#"pm.environment.set("userId", json.id);"#,
    ]
    .join("\n");
    let s = pm(&input).script;
    assert!(s.contains("nx.test"));
    assert!(s.contains("nx.expect(nx.response.code).to.equal(200)"));
    assert!(s.contains("nx.response.json()"));
    assert!(s.contains("nx.expect(json.name)"));
    assert!(s.contains("nx.environment.set"));
}

#[test]
fn empty_and_noop_inputs() {
    let r = pm("");
    assert_eq!(r.script, "");
    assert!(r.warnings.is_empty());
    assert_eq!(
        pm(r#"console.log("hello");"#).script,
        r#"console.log("hello");"#
    );
}

#[test]
fn nx_status_check_folding() {
    assert_eq!(
        nx("nx.expect(nx.response.code).to.equal(200)").script,
        "pm.response.to.have.status(200)"
    );
    assert_eq!(
        nx("nx.expect(nx.response.code).to.equal(404)").script,
        "pm.response.to.have.status(404)"
    );
}

#[test]
fn nx_skip_request_is_commented_out() {
    let r = nx("nx.execution.skipRequest()");
    assert!(r.script.contains("NEXUS"));
    assert!(r.script.contains("no Postman equivalent"));
    assert!(has_warning(&r, "skipRequest"));
}

#[test]
fn nx_edge_cases() {
    let r = nx("");
    assert_eq!(r.script, "");
    assert!(r.warnings.is_empty());
    assert_eq!(
        nx(r#"console.log("hello");"#).script,
        r#"console.log("hello");"#
    );
}

#[test]
fn platform_detection() {
    use ScriptPlatform::*;
    let d = detect_script_platform;
    assert_eq!(
        d(r#"pm.test("ok", function() { pm.response.code; });"#),
        Postman
    );
    assert_eq!(d(r#"nx.test("ok", () => { nx.response.code; });"#), Nexus);
    assert_eq!(d("pm.test(); pm.expect(); nx.response.code;"), Postman);
    assert_eq!(d("nx.test(); nx.expect(); pm.response.code;"), Nexus);
    assert_eq!(d(r#"postman.setEnvironmentVariable("k", "v");"#), Postman);
    assert_eq!(d(r#"postman.getGlobalVariable("k");"#), Postman);
    assert_eq!(d(r#"postman.clearEnvironmentVariable("k");"#), Postman);
    assert_eq!(d(r#"tests["status"] = true;"#), Postman);
    assert_eq!(d(r#"tests["ok"] = true; nx.response.code;"#), Postman);
    assert_eq!(d("pm.test(); nx.test();"), Postman);
    assert_eq!(d(r#"console.log("hello");"#), Unknown);
    assert_eq!(d(""), Unknown);
    assert_eq!(d("pm.test();"), Postman);
}

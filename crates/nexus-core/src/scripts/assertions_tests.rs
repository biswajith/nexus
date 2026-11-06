//! Port of `assertions.test.ts` plus coverage of the rest of the `nx.expect` chain. The engine is
//! `assertions.js`, so every case runs through a real sandbox.

use indexmap::IndexMap;

use super::*;

fn ctx() -> ScriptContext {
    let sb = ScriptSandbox::default();
    ScriptContext::new(
        sb.create_tracked_scope("environment", &[]),
        sb.create_tracked_scope("collection", &[]),
        sb.create_tracked_scope("global", &[]),
        MutableRequest {
            url: "https://x".into(),
            method: "GET".into(),
            headers: IndexMap::new(),
            body: None,
        },
    )
}

fn run(script: &str) -> ScriptResult {
    ScriptSandbox::default().execute(script, &mut ctx())
}

/// Runs a single `nx.test` whose body is `body` and returns its result.
fn one(body: &str) -> TestResult {
    let r = run(&format!("nx.test('t', () => {{ {body} }});"));
    assert_eq!(r.error, None);
    assert_eq!(r.test_results.len(), 1);
    r.test_results[0].clone()
}

fn passes(body: &str) {
    let t = one(body);
    assert!(t.passed, "expected `{body}` to pass, got {:?}", t.error);
}

fn fails(body: &str) -> String {
    let t = one(body);
    assert!(!t.passed, "expected `{body}` to fail");
    t.error.expect("failure carries a message")
}

// ---- assertions.test.ts: property-style assertions (issue #3) ----

#[test]
fn true_passes_for_true() {
    passes("nx.expect(true).to.be.true;");
}

#[test]
fn true_fails_for_false() {
    assert!(fails("nx.expect(false).to.be.true;").contains("expected true"));
}

#[test]
fn true_fails_for_truthy_non_boolean_values() {
    let r = run("nx.test('number 1', () => { nx.expect(1).to.be.true; });
         nx.test('non-empty string', () => { nx.expect('yes').to.be.true; });
         nx.test('object', () => { nx.expect({}).to.be.true; });");
    assert_eq!(r.test_results.len(), 3);
    assert!(r.test_results.iter().all(|t| !t.passed));
}

#[test]
fn false_passes_for_false() {
    passes("nx.expect(false).to.be.false;");
}

#[test]
fn false_fails_for_true() {
    assert!(fails("nx.expect(true).to.be.false;").contains("expected false"));
}

#[test]
fn null_passes_for_null() {
    passes("nx.expect(null).to.be.null;");
}

#[test]
fn null_fails_for_non_null() {
    fails("nx.expect(0).to.be.null;");
}

#[test]
fn undefined_passes_for_undefined() {
    passes("nx.expect(undefined).to.be.undefined;");
}

#[test]
fn undefined_fails_for_defined_values() {
    fails("nx.expect(null).to.be.undefined;");
}

#[test]
fn not_true_passes_for_false() {
    passes("nx.expect(false).not.to.be.true;");
}

#[test]
fn not_false_passes_for_true() {
    passes("nx.expect(true).not.to.be.false;");
}

#[test]
fn reproduces_the_exact_scenario_from_issue_3() {
    let t = one("const jsonData = { success: false }; nx.expect(jsonData.success).to.be.true;");
    assert!(!t.passed);
    assert!(t.error.unwrap().contains("expected true, got false"));
}

#[test]
fn direct_property_access_throws_assertion_error() {
    let r = run("let name = null;
         try { nx.expect(false).to.be.true; } catch (e) { name = e.name; }
         console.log(name);");
    assert_eq!(r.logs[0].args, vec![serde_json::json!("AssertionError")]);
}

// ---- the rest of the chain (the TS suite only covered the above) ----

#[test]
fn equal_is_deep_and_negatable() {
    passes("nx.expect({a: [1, {b: 2}]}).to.equal({a: [1, {b: 2}]});");
    passes("nx.expect({a: 1}).eql({a: 1});");
    assert_eq!(
        fails("nx.expect(200).to.equal(404);"),
        "expected 200 to equal 404"
    );
    assert_eq!(
        fails("nx.expect(1).not.to.equal(1);"),
        "expected 1 to not equal 1"
    );
    passes("nx.expect({a: 1}).not.to.equal({a: 2});");
}

#[test]
fn include_works_on_strings_arrays_and_objects() {
    passes("nx.expect('hello').to.include('ell');");
    passes("nx.expect([1, 2]).to.contain(2);");
    passes("nx.expect({a: 1, b: 2}).to.include({a: 1});");
    assert_eq!(
        fails("nx.expect([1]).to.include(2);"),
        "expected [1] to include 2"
    );
    fails("nx.expect({a: 1}).to.include({a: 2});");
    fails("nx.expect(5).to.include(5);");
}

#[test]
fn property_checks_presence_and_value() {
    passes("nx.expect({a: 1}).to.have.property('a');");
    passes("nx.expect({a: 1}).to.have.property('a', 1);");
    assert_eq!(
        fails("nx.expect({a: 1}).to.have.property('a', 2);"),
        "expected property \"a\" to equal 2, got 1"
    );
    assert_eq!(
        fails("nx.expect({}).to.have.property('x');"),
        "expected object to have property \"x\""
    );
    fails("nx.expect(null).to.have.property('x');");
    passes("nx.expect({}).not.to.have.property('x');");
}

#[test]
fn length_and_empty() {
    passes("nx.expect([1, 2, 3]).to.have.length(3);");
    passes("nx.expect('ab').to.have.length(2);");
    assert_eq!(
        fails("nx.expect([1]).to.have.length(2);"),
        "expected length 1 to equal 2"
    );
    passes("nx.expect([]).to.be.empty;");
    passes("nx.expect('').to.be.empty;");
    passes("nx.expect({}).to.be.empty;");
    passes("nx.expect(null).to.be.empty;");
    fails("nx.expect([0]).to.be.empty;");
    fails("nx.expect(0).to.be.empty;");
    passes("nx.expect([1]).not.to.be.empty;");
}

#[test]
fn numeric_comparisons() {
    passes("nx.expect(5).to.be.above(4);");
    passes("nx.expect(5).to.be.below(6);");
    passes("nx.expect(5).to.have.least(5);");
    passes("nx.expect(5).to.have.most(5);");
    assert_eq!(
        fails("nx.expect(5).to.be.above(5);"),
        "expected 5 to be above 5"
    );
    assert_eq!(
        fails("nx.expect('5').to.be.below(9);"),
        "expected 5 to be below 9"
    );
    assert_eq!(
        fails("nx.expect(1).to.have.least(2);"),
        "expected 1 to be at least 2"
    );
    assert_eq!(
        fails("nx.expect(3).to.have.most(2);"),
        "expected 3 to be at most 2"
    );
    passes("nx.expect(5).not.to.be.above(5);");
}

#[test]
fn match_one_of_and_ok() {
    passes("nx.expect('abc123').to.match(/\\d+/);");
    fails("nx.expect(12).to.match(/\\d+/);");
    passes("nx.expect(2).to.be.oneOf([1, 2, 3]);");
    assert_eq!(
        fails("nx.expect(9).to.be.oneOf([1, 2]);"),
        "expected 9 to be one of [1,2]"
    );
    passes("nx.expect(1).to.be.ok();");
    fails("nx.expect(0).to.be.ok();");
    passes("nx.expect(0).not.to.be.ok();");
}

#[test]
fn type_checks() {
    passes("nx.expect('s').to.be.a('string');");
    passes("nx.expect(1).to.be.a('number');");
    passes("nx.expect(true).to.be.a('boolean');");
    passes("nx.expect({}).to.be.an('object');");
    passes("nx.expect([]).to.be.an('array');");
    passes("nx.expect(null).to.be.a('null');");
    passes("nx.expect(undefined).to.be.an('undefined');");
    passes("nx.expect(() => 1).to.be.a('function');");
    passes("nx.expect('s').to.be.a('STRING');");
    assert_eq!(
        fails("nx.expect([]).to.be.an('object');"),
        "expected a object, got object"
    );
    assert_eq!(
        fails("nx.expect(1).to.be.a('string');"),
        "expected a string, got number"
    );
    assert_eq!(
        fails("nx.expect('s').not.to.be.a('string');"),
        "expected value to not be a string"
    );
    passes("nx.expect(1n).to.be.a('bigint');");
}

#[test]
fn match_schema_validates_with_json_schema() {
    passes("nx.expect({id: 1, name: 'x'}).to.matchSchema({type: 'object', required: ['id'], properties: {id: {type: 'number'}}});");
    let msg = fails("nx.expect({id: 'a'}).to.matchSchema({type: 'object', properties: {id: {type: 'number'}}});");
    assert!(msg.starts_with("schema validation failed: /id "), "{msg}");
    assert_eq!(
        fails("nx.expect({}).not.to.matchSchema({type: 'object'});"),
        "expected value to not match schema"
    );
}

#[test]
fn match_schema_collects_all_errors_and_checks_formats() {
    let msg = fails(
        "nx.expect({a: 1, b: 2}).to.matchSchema({type: 'object', properties: {a: {type: 'string'}, b: {type: 'string'}}});",
    );
    assert!(
        msg.contains("/a ") && msg.contains("/b ") && msg.contains("; "),
        "{msg}"
    );
    passes("nx.expect('a@b.co').to.matchSchema({type: 'string', format: 'email'});");
    fails("nx.expect('nope').to.matchSchema({type: 'string', format: 'email'});");
    passes("nx.expect('2024-01-31').to.matchSchema({type: 'string', format: 'date'});");
    fails("nx.expect('not a date').to.matchSchema({type: 'string', format: 'date'});");
}

#[test]
fn invalid_schema_is_a_failed_assertion_not_a_crash() {
    let msg = fails("nx.expect({}).to.matchSchema({type: 'nope'});");
    assert!(msg.starts_with("schema validation failed: "), "{msg}");
}

#[test]
fn match_schema_never_resolves_remote_refs() {
    // No network access: a remote $ref must fail to compile (quickly) rather than being fetched.
    let msg = fails("nx.expect({}).to.matchSchema({$ref: 'http://127.0.0.1:9/schema.json'});");
    assert!(msg.starts_with("schema validation failed: "), "{msg}");
}

use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

use super::dynamic;
use crate::types::{ScopeLevel, Variable, VariableType};

/// Lookup order, highest precedence first.
const SCOPE_ORDER: [ScopeLevel; 4] = [
    ScopeLevel::Local,
    ScopeLevel::Environment,
    ScopeLevel::Collection,
    ScopeLevel::Global,
];

/// `.` in the JS regex does not match line terminators.
static TOKEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{([^\n\r\u{2028}\u{2029}]+?)\}\}").expect("valid regex"));

/// In-memory scope backed by an ordered map of enabled variables.
#[derive(Debug, Clone, Default)]
pub struct VariableScope {
    vars: IndexMap<String, Variable>,
}

impl VariableScope {
    /// Builds a scope from enabled variables only (disabled ones are omitted; a later duplicate wins).
    pub fn new(variables: &[Variable]) -> Self {
        let vars = variables
            .iter()
            .filter(|v| v.enabled)
            .map(|v| (v.key.clone(), v.clone()))
            .collect();
        Self { vars }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(|v| v.value.as_str())
    }

    /// Sets or updates a variable, creating an enabled string entry when missing.
    pub fn set(&mut self, key: &str, value: &str) {
        match self.vars.get_mut(key) {
            Some(existing) => existing.value = value.to_string(),
            None => {
                self.vars.insert(
                    key.to_string(),
                    Variable {
                        key: key.to_string(),
                        value: value.to_string(),
                        var_type: VariableType::String,
                        enabled: true,
                        description: None,
                    },
                );
            }
        }
    }

    pub fn unset(&mut self, key: &str) {
        self.vars.shift_remove(key);
    }

    pub fn get_all(&self) -> Vec<Variable> {
        self.vars.values().cloned().collect()
    }

    pub fn to_record(&self) -> IndexMap<String, String> {
        self.vars
            .iter()
            .map(|(k, v)| (k.clone(), v.value.clone()))
            .collect()
    }
}

/// The `{ global?, collection?, environment? }` payload of `variables:resolve`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VariableScopes {
    #[serde(default)]
    pub global: Vec<Variable>,
    #[serde(default)]
    pub collection: Vec<Variable>,
    #[serde(default)]
    pub environment: Vec<Variable>,
}

/// Layered variable scopes with `{{name}}` resolution and dynamic `{{$name}}` placeholders.
#[derive(Debug, Clone, Default)]
pub struct VariableStore {
    global: VariableScope,
    collection: VariableScope,
    environment: VariableScope,
    local: VariableScope,
}

impl VariableStore {
    /// Seeds global, collection and environment scopes; local starts empty.
    pub fn new(scopes: &VariableScopes) -> Self {
        Self {
            global: VariableScope::new(&scopes.global),
            collection: VariableScope::new(&scopes.collection),
            environment: VariableScope::new(&scopes.environment),
            local: VariableScope::default(),
        }
    }

    pub fn scope(&self, level: ScopeLevel) -> &VariableScope {
        match level {
            ScopeLevel::Local => &self.local,
            ScopeLevel::Environment => &self.environment,
            ScopeLevel::Collection => &self.collection,
            ScopeLevel::Global => &self.global,
        }
    }

    pub fn scope_mut(&mut self, level: ScopeLevel) -> &mut VariableScope {
        match level {
            ScopeLevel::Local => &mut self.local,
            ScopeLevel::Environment => &mut self.environment,
            ScopeLevel::Collection => &mut self.collection,
            ScopeLevel::Global => &mut self.global,
        }
    }

    /// Replaces `{{key}}` segments using scope precedence (local > environment > collection > global)
    /// and dynamic generators. Unknown keys are left as the original token.
    pub fn resolve(&self, template: &str) -> String {
        TOKEN_RE
            .replace_all(template, |caps: &Captures| {
                let trimmed = caps[1].trim();
                if trimmed.starts_with('$') {
                    return Self::resolve_dynamic(trimmed);
                }
                for level in SCOPE_ORDER {
                    if let Some(val) = self.scope(level).get(trimmed) {
                        return val.to_string();
                    }
                }
                caps[0].to_string()
            })
            .into_owned()
    }

    /// Resolves both header names and values.
    pub fn resolve_headers(&self, headers: &IndexMap<String, String>) -> IndexMap<String, String> {
        headers
            .iter()
            .map(|(k, v)| (self.resolve(k), self.resolve(v)))
            .collect()
    }

    /// Writes key/value pairs into the local scope (overwrites existing keys).
    pub fn set_local_scope<'a>(&mut self, data: impl IntoIterator<Item = (&'a str, &'a str)>) {
        for (k, v) in data {
            self.local.set(k, v);
        }
    }

    pub fn clear_local_scope(&mut self) {
        self.local = VariableScope::default();
    }

    fn resolve_dynamic(key: &str) -> String {
        dynamic::generate(key).unwrap_or_else(|| format!("{{{{{key}}}}}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environments::DYNAMIC_KEYS;

    fn var(key: &str, value: &str) -> Variable {
        Variable {
            key: key.into(),
            value: value.into(),
            var_type: VariableType::String,
            enabled: true,
            description: None,
        }
    }

    fn disabled(key: &str, value: &str) -> Variable {
        Variable {
            enabled: false,
            ..var(key, value)
        }
    }

    fn env(vars: Vec<Variable>) -> VariableStore {
        VariableStore::new(&VariableScopes {
            environment: vars,
            ..Default::default()
        })
    }

    #[test]
    fn resolves_environment_variables() {
        let store = env(vec![var("host", "api.example.com")]);
        assert_eq!(
            store.resolve("https://{{host}}/users"),
            "https://api.example.com/users"
        );
    }

    #[test]
    fn resolves_collection_variables() {
        let store = VariableStore::new(&VariableScopes {
            collection: vec![var("version", "v2")],
            ..Default::default()
        });
        assert_eq!(store.resolve("/api/{{version}}/items"), "/api/v2/items");
    }

    #[test]
    fn resolves_global_variables() {
        let store = VariableStore::new(&VariableScopes {
            global: vec![var("apiKey", "abc123")],
            ..Default::default()
        });
        assert_eq!(store.resolve("?key={{apiKey}}"), "?key=abc123");
    }

    #[test]
    fn respects_scope_precedence() {
        let mut store = VariableStore::new(&VariableScopes {
            global: vec![var("x", "global")],
            collection: vec![var("x", "collection")],
            environment: vec![var("x", "environment")],
        });
        assert_eq!(store.resolve("{{x}}"), "environment");
        store.set_local_scope([("x", "local")]);
        assert_eq!(store.resolve("{{x}}"), "local");
    }

    #[test]
    fn collection_beats_global() {
        let store = VariableStore::new(&VariableScopes {
            global: vec![var("x", "global")],
            collection: vec![var("x", "collection")],
            ..Default::default()
        });
        assert_eq!(store.resolve("{{x}}"), "collection");
    }

    #[test]
    fn resolves_multiple_variables_in_one_url() {
        let store = VariableStore::new(&VariableScopes {
            environment: vec![
                var("protocol", "https"),
                var("host", "api.example.com"),
                var("port", "8443"),
            ],
            collection: vec![var("basePath", "api/v1")],
            ..Default::default()
        });
        assert_eq!(
            store.resolve("{{protocol}}://{{host}}:{{port}}/{{basePath}}/users"),
            "https://api.example.com:8443/api/v1/users"
        );
    }

    #[test]
    fn leaves_unresolved_tokens() {
        let store = VariableStore::default();
        assert_eq!(
            store.resolve("https://{{unknown}}/path"),
            "https://{{unknown}}/path"
        );
    }

    #[test]
    fn ignores_disabled_variables() {
        let store = env(vec![disabled("host", "disabled.com")]);
        assert_eq!(
            store.resolve("https://{{host}}/path"),
            "https://{{host}}/path"
        );
    }

    #[test]
    fn resolves_headers() {
        let store = env(vec![var("token", "bearer-xyz")]);
        let headers: IndexMap<String, String> = [
            ("Authorization".to_string(), "{{token}}".to_string()),
            ("Content-Type".to_string(), "application/json".to_string()),
        ]
        .into_iter()
        .collect();
        let resolved = store.resolve_headers(&headers);
        assert_eq!(resolved["Authorization"], "bearer-xyz");
        assert_eq!(resolved["Content-Type"], "application/json");
    }

    #[test]
    fn resolves_header_names() {
        let store = env(vec![var("h", "X-Custom")]);
        let headers: IndexMap<String, String> = [("{{h}}".to_string(), "v".to_string())]
            .into_iter()
            .collect();
        assert_eq!(store.resolve_headers(&headers)["X-Custom"], "v");
    }

    #[test]
    fn resolves_guid_and_timestamp() {
        let store = VariableStore::default();
        let guid = store.resolve("{{$guid}}");
        let re =
            Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").unwrap();
        assert!(re.is_match(&guid), "{guid}");
        assert!(store.resolve("{{$timestamp}}").parse::<u64>().unwrap() > 0);
    }

    #[test]
    fn whitespace_in_token_names() {
        let store = env(vec![var("host", "example.com")]);
        assert_eq!(store.resolve("{{ host }}"), "example.com");
        assert_eq!(store.resolve("{{  host  }}"), "example.com");
    }

    #[test]
    fn empty_and_tokenless_input() {
        let store = env(vec![var("host", "example.com")]);
        assert_eq!(store.resolve(""), "");
        assert_eq!(
            store.resolve("https://plain-url.com/path"),
            "https://plain-url.com/path"
        );
    }

    #[test]
    fn clears_local_scope() {
        let mut store = env(vec![var("x", "env-val")]);
        store.set_local_scope([("x", "local-val")]);
        assert_eq!(store.resolve("{{x}}"), "local-val");
        store.clear_local_scope();
        assert_eq!(store.resolve("{{x}}"), "env-val");
    }

    #[test]
    fn unknown_dynamic_variable_is_left_as_token() {
        assert_eq!(VariableStore::default().resolve("{{$nope}}"), "{{$nope}}");
    }

    #[test]
    fn scope_set_unset_and_records() {
        let mut store = VariableStore::default();
        let scope = store.scope_mut(ScopeLevel::Global);
        scope.set("a", "1");
        scope.set("b", "2");
        scope.set("a", "3");
        assert_eq!(scope.get("a"), Some("3"));
        assert_eq!(scope.get_all().len(), 2);
        assert_eq!(scope.to_record().keys().collect::<Vec<_>>(), ["a", "b"]);
        scope.unset("a");
        assert_eq!(scope.get("a"), None);
        assert_eq!(
            store.scope(ScopeLevel::Global).get_all()[0].var_type,
            VariableType::String
        );
    }

    #[test]
    fn every_dynamic_variable_resolves_to_non_empty_value() {
        assert_eq!(DYNAMIC_KEYS.len(), 118);
        let store = VariableStore::default();
        for key in DYNAMIC_KEYS {
            let token = format!("{{{{{key}}}}}");
            let out = store.resolve(&token);
            assert_ne!(out, token, "{key} did not resolve");
            assert!(!out.is_empty(), "{key} produced an empty value");
        }
    }

    fn dynamic(key: &str) -> String {
        VariableStore::default().resolve(&format!("{{{{{key}}}}}"))
    }

    #[test]
    fn dynamic_value_shapes() {
        let check = |key: &str, pattern: &str| {
            for _ in 0..20 {
                let v = dynamic(key);
                assert!(
                    Regex::new(pattern).unwrap().is_match(&v),
                    "{key}: {v:?} !~ {pattern}"
                );
            }
        };
        check(
            "$randomUUID",
            r"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[0-9a-f]{4}-[0-9a-f]{12}$",
        );
        check(
            "$isoTimestamp",
            r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$",
        );
        check("$randomAlphaNumeric", r"^[A-Za-z0-9]$");
        check("$randomBoolean", r"^(true|false)$");
        check("$randomInt", r"^\d{1,4}$");
        check("$randomHexColor", r"^#[0-9a-f]{6}$");
        check("$randomIP", r"^\d{1,3}(\.\d{1,3}){3}$");
        check("$randomIPV6", r"^[0-9a-fA-F:]+$");
        check("$randomMACAddress", r"^([0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}$");
        check("$randomPassword", r"^[A-Za-z0-9]{15}$");
        check("$randomSemver", r"^\d+\.\d+\.\d+$");
        check("$randomPhoneNumberExt", r" x\d{4}$");
        check("$randomLatitude", r"^-?\d{1,2}\.\d+$");
        check("$randomLongitude", r"^-?\d{1,3}\.\d+$");
        check("$randomImageUrl", r"^https://loremflickr\.com/640/480$");
        check(
            "$randomCatsImage",
            r"^https://loremflickr\.com/640/480/cats$",
        );
        check("$randomImageDataUri", r"^data:image/svg\+xml");
        check("$randomBankAccount", r"^\d{8}$");
        check("$randomCreditCardMask", r"^\d{4}$");
        check("$randomBankAccountIban", r"^GB\d{2}[A-Z]{4}\d{14}$");
        check("$randomBitcoin", r"^[13][1-9A-HJ-NP-Za-km-z]{25,33}$");
        check("$randomCurrencyCode", r"^[A-Z]{3}$");
        check(
            "$randomDateFuture",
            r"^[A-Z][a-z]{2} [A-Z][a-z]{2} \d{2} \d{4} \d{2}:\d{2}:\d{2} GMT[+-]\d{4}$",
        );
        check(
            "$randomDatePast",
            r"^[A-Z][a-z]{2} [A-Z][a-z]{2} \d{2} \d{4} ",
        );
        check("$randomEmail", r"^[^@\s]+@[^@\s]+\.[^@\s]+$");
        check(
            "$randomExampleEmail",
            r"^[a-z]+\.[a-z]+@example\.(org|com|net)$",
        );
        check("$randomUrl", r"^https?://[^/\s]+\.[a-z]+$");
        check("$randomDomainName", r"^[a-z0-9]+\.[a-z.]+$");
        check("$randomFileName", r"\.\w+$");
        check("$randomPrice", r"^\d{1,4}\.\d{2}$");
        check("$randomLoremSlug", r"^[a-z]+(-[a-z]+)*$");
        check("$randomWords", r"^\S+( \S+){4}$");
        check("$randomLoremSentence", r"\.$");
    }

    #[test]
    fn generated_iban_has_valid_checksum() {
        for _ in 0..50 {
            let iban = dynamic("$randomBankAccountIban");
            let rearranged = format!("{}{}", &iban[4..], &iban[..4]);
            let rem = rearranged.chars().fold(0u32, |acc, c| {
                let d = if c.is_ascii_digit() {
                    c.to_string()
                } else {
                    (c as u8 - b'A' + 10).to_string()
                };
                d.bytes()
                    .fold(acc, |a, b| (a * 10 + u32::from(b - b'0')) % 97)
            });
            assert_eq!(rem, 1, "{iban}");
        }
    }
}

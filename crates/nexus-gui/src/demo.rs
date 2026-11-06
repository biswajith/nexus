//! Demo data for screenshots and tests (`--seed-demo`): a few collections, environments and history.

use nexus_core::service::App;
use nexus_core::types::{
    create_default_collection, create_default_environment, create_default_request, BodyMode,
    CollectionItem, FormDataEntry, FormDataType, GraphqlBody, HistoryEntry, HistoryRequest,
    HistoryResponse, HttpMethod, KeyValuePair, NexusFolder, NexusRequest, Variable, VariableType,
};
use std::path::Path;

fn req(name: &str, method: HttpMethod, url: &str) -> NexusRequest {
    let mut r = create_default_request();
    r.name = name.into();
    r.method = method;
    r.url = url.into();
    r
}

/// Writes the demo data under `home` (a data directory, normally a temp dir).
pub fn seed(home: &Path) -> std::io::Result<()> {
    let app = App::new(home)?;
    let fail = |e: nexus_core::Error| std::io::Error::other(e.to_string());

    let mut pets = create_default_collection("Petstore API");
    let mut create = req("Create pet", HttpMethod::Post, "{{baseUrl}}/pets");
    create.body.mode = BodyMode::Json;
    create.body.raw = Some("{\n  \"name\": \"Rex\",\n  \"tag\": \"dog\"\n}".into());
    let mut list = req("List pets", HttpMethod::Get, "{{baseUrl}}/pets");
    list.params.push(KeyValuePair {
        key: "limit".into(),
        value: "10".into(),
        enabled: true,
        description: None,
    });
    pets.variables.push(Variable {
        key: "baseUrl".into(),
        value: "https://petstore.example.com/v1".into(),
        var_type: VariableType::String,
        enabled: true,
        description: None,
    });
    pets.items = vec![
        CollectionItem::Folder(NexusFolder {
            id: "folder-pets".into(),
            name: "Pets".into(),
            description: None,
            items: vec![
                CollectionItem::Request(list),
                CollectionItem::Request(create),
                CollectionItem::Request(req(
                    "Delete pet",
                    HttpMethod::Delete,
                    "{{baseUrl}}/pets/1",
                )),
            ],
            auth: None,
            pre_request_script: None,
            post_response_script: None,
            item_order: None,
        }),
        CollectionItem::Request(req("Health check", HttpMethod::Get, "{{baseUrl}}/health")),
    ];
    app.collection_create(&pets).map_err(fail)?;

    let kv = |k: &str, v: &str, d: Option<&str>| KeyValuePair {
        key: k.into(),
        value: v.into(),
        enabled: true,
        description: d.map(Into::into),
    };
    let mut login = req("Login", HttpMethod::Post, "https://auth.example.com/login");
    login.headers = vec![
        kv("Accept", "application/json", Some("Ask for JSON")),
        kv("X-Request-Id", "{{$guid}}", None),
    ];
    login.params = vec![kv("client", "web", None)];
    login.url = "https://auth.example.com/login?client=web".into();
    login.body.mode = BodyMode::XWwwFormUrlencoded;
    login.body.urlencoded = Some(vec![
        kv("username", "ada", None),
        kv("password", "lovelace", Some("never log this")),
    ]);
    login.auth = nexus_core::types::AuthConfig::new(nexus_core::types::AuthType::Basic);
    login
        .auth
        .extra
        .insert("username".into(), "service-account".into());
    login.auth.extra.insert("password".into(), "s3cret".into());

    let mut upload = req(
        "Upload avatar",
        HttpMethod::Post,
        "{{baseUrl}}/users/1/avatar",
    );
    upload.body.mode = BodyMode::FormData;
    upload.body.form_data = Some(vec![
        FormDataEntry {
            key: "caption".into(),
            value: "Me at the beach".into(),
            entry_type: FormDataType::Text,
            enabled: true,
            description: None,
        },
        FormDataEntry {
            key: "avatar".into(),
            value: "/home/ada/Pictures/beach.png".into(),
            entry_type: FormDataType::File,
            enabled: true,
            description: Some("PNG, max 2 MB".into()),
        },
    ]);
    upload.auth = nexus_core::types::AuthConfig::new(nexus_core::types::AuthType::Bearer);
    upload
        .auth
        .extra
        .insert("token".into(), "{{auth_token}}".into());

    let mut search = req("Search pets", HttpMethod::Post, "{{baseUrl}}/graphql");
    search.body.mode = BodyMode::Graphql;
    search.body.graphql = Some(GraphqlBody {
        query: "query Search($name: String!) {\n  pets(name: $name) {\n    id\n    name\n  }\n}"
            .into(),
        variables: Some("{\n  \"name\": \"Rex\"\n}".into()),
    });

    let mut refresh = req(
        "Refresh token",
        HttpMethod::Put,
        "https://auth.example.com/refresh",
    );
    refresh.auth = nexus_core::types::AuthConfig::new(nexus_core::types::AuthType::Oauth2);
    refresh
        .auth
        .extra
        .insert("accessToken".into(), "ya29.a0".into());
    refresh.pre_request_script = Some(
        "// stamp the request\nnx.request.headers[\"X-Sent-At\"] = new Date().toISOString();"
            .into(),
    );
    refresh.post_response_script = Some(
        "nx.test(\"Status is 200\", () => {\n  nx.expect(nx.response.code).to.equal(200);\n});"
            .into(),
    );

    let mut auth = create_default_collection("Auth Service");
    auth.items = vec![
        CollectionItem::Request(login),
        CollectionItem::Request(refresh),
        CollectionItem::Request(upload),
        CollectionItem::Request(search),
    ];
    app.collection_create(&auth).map_err(fail)?;

    for name in ["Development", "Staging"] {
        let mut e = create_default_environment();
        e.name = name.into();
        e.variables.push(Variable {
            key: "baseUrl".into(),
            value: format!("https://{}.example.com", name.to_lowercase()),
            var_type: VariableType::String,
            enabled: true,
            description: None,
        });
        app.environment_save(&e).map_err(fail)?;
    }

    let now = chrono::Utc::now().timestamp_millis();
    for (i, (m, u, s, ms)) in [
        ("GET", "https://api.example.com/users", 200u16, 142.0),
        ("POST", "https://api.example.com/users", 201, 233.0),
        ("GET", "https://api.example.com/users/99", 404, 87.0),
        ("DELETE", "https://api.example.com/sessions/3", 500, 310.0),
    ]
    .into_iter()
    .enumerate()
    {
        app.history_log(&HistoryEntry {
            id: format!("h{i}"),
            timestamp: now - (i as i64) * 60_000,
            request: HistoryRequest {
                method: m.into(),
                url: u.into(),
                headers: Default::default(),
                body: None,
            },
            response: HistoryResponse {
                status: s,
                status_text: String::new(),
                response_time: ms,
                size: 512,
            },
            collection_id: None,
            request_id: None,
        })
        .map_err(fail)?;
    }
    Ok(())
}

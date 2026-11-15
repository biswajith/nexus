# Chapter 17: Saving data: JSON and serde

> **Brain Power**
> Nexus keeps every collection of requests as plain files in a folder. Inside your program, a request is a struct with a method, a path and a list of headers. How do you turn that bundle into **text** that can be saved in a file (and read back later into the same struct)? Think about how many lines it would take you to do it by hand for a struct with thirty fields.

Probably *a lot*. And you would have to write the reverse, the parser, too. This chapter shows how Rust programmers get both in **one line**, with everything you learned about traits and derive.

## What is JSON?

**JSON** is a plain-text format for data, used by almost every web service. It is built from just a few things:

```text
{
  "key": "Accept",
  "value": "*/*",
  "enabled": true,
  "tags": ["a", "b"],
  "description": null
}
```

* `{ ... }` is an **object**: names (in quotes) with values.
* `[ ... ]` is a **list**.
* Values are text in double quotes, numbers, `true` / `false`, or `null` ("nothing").

It lines up beautifully with Rust:

| Rust | JSON |
|---|---|
| `struct` (fields) | object |
| `Vec<T>` | list |
| `String`, `&str` | text |
| numbers, `bool` | numbers, `true`/`false` |
| `Option<T>` (`None`) | the value, or `null` |
| enum variant without data | text, like `"GET"` |

> **New word: serialize / deserialize**
> To **serialize** is to turn a value in memory (a struct) into text (like JSON). To **deserialize** is the reverse: read text and build the value. (Both words are about "serial" = one thing after another, like characters in a file.)

![#[derive(Serialize, Deserialize)]: a two-way translator](img/ch17-serde.svg)

## serde: the translator

The Rust community's answer is a pair of crates (Chapter 15):

* **`serde`** defines two traits (Chapter 10), **`Serialize`** and **`Deserialize`**, and *derive macros* that write them for you (Chapter 10's `derive`). It knows nothing about JSON in particular.
* **`serde_json`** knows the JSON format, and works with anything that is `Serialize` / `Deserialize`.

To follow along, make a playground project and add both:

```bash
cargo add serde --features derive
cargo add serde_json
```

Here is the whole idea in twelve lines. (The first line, `// needs-deps`, is just a reminder that this example needs those two crates.)

```rust
// needs-deps
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Header {
    key: String,
    value: String,
    enabled: bool,
}

fn main() {
    let h = Header {
        key: String::from("Accept"),
        value: String::from("*/*"),
        enabled: true,
    };

    let json = serde_json::to_string(&h).unwrap();
    println!("{json}");

    let back: Header = serde_json::from_str(&json).unwrap();
    println!("{back:?}");
}
```

```output
{"key":"Accept","value":"*/*","enabled":true}
Header { key: "Accept", value: "*/*", enabled: true }
```

Take it apart:

* **`#[derive(Debug, Serialize, Deserialize)]`**: ask the compiler to write the two serde traits, the way `Debug` is written (Chapter 6). Every field must itself be serializable: `String` and `bool` are.
* **`serde_json::to_string(&h)`**: turn the header into JSON text. It borrows `h` (`&`, Chapter 5). It returns a **`Result`** (Chapter 8): serializing can fail in odd cases, so we `.unwrap()` it for this experiment.
* **`serde_json::from_str(&json)`**: the reverse. We tell it what to build with the type annotation: `let back: Header`. (Same trick as `collect` in Chapter 13: the target type decides.)
* The output of the first `println!` is compact. For humans, use **`to_string_pretty`**:

```rust
// needs-deps
use serde::Serialize;

#[derive(Serialize)]
struct Header {
    key: String,
    value: String,
    enabled: bool,
}

fn main() {
    let h = Header {
        key: String::from("Accept"),
        value: String::from("*/*"),
        enabled: true,
    };
    println!("{}", serde_json::to_string_pretty(&h).unwrap());
}
```

```output
{
  "key": "Accept",
  "value": "*/*",
  "enabled": true
}
```

(You can derive just `Serialize` if you only need one direction.)

### When the text is wrong

Reading JSON from outside is where `Result` earns its keep. Deserializing can fail, and the error is a friendly, specific value (Chapter 8). Predict what each of these says:

```rust
// needs-deps
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Header {
    key: String,
    value: String,
    enabled: bool,
}

fn main() {
    let missing: Result<Header, _> = serde_json::from_str("{\"key\": \"A\"}");
    println!("{}", missing.unwrap_err());

    let wrong: Result<Header, _> =
        serde_json::from_str("{\"key\": \"A\", \"value\": 5, \"enabled\": true}");
    println!("{}", wrong.unwrap_err());

    let nonsense: Result<Header, _> = serde_json::from_str("not json");
    println!("{}", nonsense.unwrap_err());
}
```

<details><summary>Answer</summary>

```output
missing field `value` at line 1 column 12
invalid type: integer `5`, expected a string at line 1 column 23
expected ident at line 1 column 2
```

A missing field, a value of the wrong *type* (`5` where text was expected), and broken JSON each give a precise message with a position. Note that a program can't be tricked into building a half-filled `Header`: it either gets a complete, correctly-typed value, or an `Err`. `.unwrap_err()` is the opposite of `.unwrap()`: it extracts the error (and panics on `Ok`), handy for demos.

</details>

## Making the JSON look the way you want

Rust and JSON have different taste. Rust names use `snake_case`, many JSON files use `camelCase`; Rust enum variants are `Get`, while JSON wants `"GET"`. You steer serde with **attributes** (Chapter 6) written `#[serde(...)]`:

```rust
// needs-deps
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
enum Method {
    Get,
    Post,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    status_code: u16,
    method: Method,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

fn main() {
    let r = Response { status_code: 200, method: Method::Get, note: None };
    println!("{}", serde_json::to_string(&r).unwrap());

    let r = Response { status_code: 404, method: Method::Post, note: Some(String::from("gone")) };
    println!("{}", serde_json::to_string(&r).unwrap());

    let text = "{\"statusCode\": 500, \"method\": \"POST\"}";
    let parsed: Response = serde_json::from_str(text).unwrap();
    println!("{parsed:?}");
}
```

```output
{"statusCode":200,"method":"GET"}
{"statusCode":404,"method":"POST","note":"gone"}
Response { status_code: 500, method: Post, note: None }
```

The attributes, one at a time:

* **`#[serde(rename_all = "UPPERCASE")]`** on the enum: `Get` becomes `"GET"` in JSON. On the struct, **`"camelCase"`** turns the Rust field `status_code` into `statusCode`. (There is also `rename = "type"` for a single field, handy when the JSON name is a Rust keyword.)
* **`note: Option<String>`** is `Some("gone")` or `None`. With `skip_serializing_if = "Option::is_none"`, a `None` note is simply *left out* of the JSON instead of written as `null`. (`Option::is_none` is "the function `is_none` of `Option`": a path, Chapter 14.)
* **`default`** means: if the field is *missing* when reading, use the type's `Default` (Chapter 10). `None` is the default for an `Option`, so the third example parses fine without a `note`.

Rule: *derive gives you the behaviour; attributes tune the spelling.*

> **There are no Dumb Questions**
>
> **Q: What if the JSON has extra fields my struct doesn't have?**
> A: By default they are ignored. serde only picks what it needs. (You can ask it to reject extras with `#[serde(deny_unknown_fields)]`.)
>
> **Q: Does it work for nested structs, lists and maps?**
> A: Yes. Anything made of serializable parts is serializable: `Vec<Header>`, `HashMap<String, String>`, a struct containing a struct... all with the same one derive line.
>
> **Q: Is it fast?**
> A: Very. The derive writes plain code at compile time. There is no runtime guessing.
>
> **Q: Is JSON the only format?**
> A: No. The `Serialize` / `Deserialize` traits are format-independent; other crates handle YAML, TOML and more. Nexus uses `serde_yaml` to read OpenAPI files, with the same derives.

> **Sharpen your pencil**
> Given this struct and `#[serde(rename_all = "camelCase")]`, what JSON key will each field get?
> ```text
> struct Settings {
>     timeout_ms: u64,
>     follow_redirects: bool,
>     proxy_url: Option<String>,
> }
> ```
>
> <details><summary>Answer</summary>
>
> `timeoutMs`, `followRedirects` and `proxyUrl`. Each underscore-plus-letter becomes an uppercase letter, and the first word stays lowercase.
> </details>

## Now look at real Nexus

Now you understand the second of the two attribute lines that stood above every Nexus type since Chapter 6. Here is `KeyValuePair` *with its attributes*, from [`types/mod.rs`](../../crates/nexus-core/src/types/mod.rs):

```rust,nexus
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValuePair {
    pub key: String,
    pub value: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
```

Decode each line with this chapter and Chapter 10:

* `derive(Debug, Clone, PartialEq, Serialize, Deserialize)`: the abilities from Chapter 10 plus the two from this chapter.
* `rename_all = "camelCase"`: so a field like `form_data` is `formData` in the file. This keeps the file format compatible with the earlier version of Nexus, which was written in TypeScript and used camelCase everywhere.
* `description: Option<String>` with `default` and `skip_serializing_if`: exactly your `note` field. A header without a description is stored without the key at all; and reading an old file without it still works.

The HTTP method enum uses the `UPPERCASE` trick you just learned:

```rust,nexus
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    // ...
}
```

So `HttpMethod::Post` is stored as `"POST"` in your `~/.nexus` files. Not every enum fits one rule: `BodyMode` renames each variant by hand (a variant can have its own `#[serde(rename = "form-data")]`).

And the sidebar item `CollectionItem` (Chapter 7's "a folder *or* a request") has a different attribute, which shows serde's power:

```rust,nexus
#[serde(untagged)]
pub enum CollectionItem {
    Folder(NexusFolder),
    Request(NexusRequest),
}
```

`untagged` tells serde: "there's no label saying which variant this is. Try each variant in order and use the first one whose fields fit." A JSON object with an `items` list is a folder; one with a `method` is a request. The saved files stay exactly like they were before.

Here is the picture to keep: **one derive line, and `serde_json` turns a whole collection into a folder of plain JSON files and back.** That is how Nexus saves data without any database.

## Checkpoint project: request builder, version 16

The builder can now **save** a request to a file and **load** it back. Start from Chapter 16's workspace and make these changes. First, add `serde` and `serde_json` to the shared dependency list and to `rb-core`:

```toml,proj=rb17,file=Cargo.toml
[workspace]
resolver = "2"
members = ["crates/rb-core", "crates/rb-cli"]

[workspace.package]
version = "0.1.0"
edition = "2021"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
uuid = { version = "1", features = ["v4"] }
rb-core = { path = "crates/rb-core" }
```

```toml,proj=rb17,file=crates/rb-core/Cargo.toml
[package]
name = "rb-core"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
uuid.workspace = true
```

<!-- proj=rb17 copy=15_crates_and_workspaces:rb15:crates/rb-cli/Cargo.toml>crates/rb-cli/Cargo.toml -->
<!-- proj=rb17 copy=15_crates_and_workspaces:rb15:crates/rb-core/src/variables.rs>crates/rb-core/src/variables.rs -->

The error type of Chapter 16 gains the two variants that Nexus's own `error.rs` has: one for file problems, one for JSON problems. (We drop `PartialEq` from the derive list, since `std::io::Error` cannot be compared.)

```rust,proj=rb17,file=crates/rb-core/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unknown method '{0}'")]
    UnknownMethod(String),

    #[error("header line '{0}' has no ':'")]
    BadHeader(String),

    #[error("path '{path}' is invalid: {reason}")]
    InvalidPath { path: String, reason: String },

    #[error("invalid port: {0}")]
    BadPort(#[from] std::num::ParseIntError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
```

`method.rs` gets the serde derives and the `UPPERCASE` rename (`parse` and `Display` are unchanged):

```rust,proj=rb17,file=crates/rb-core/src/method.rs
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    #[default]
    Get,
    Post,
}

impl Method {
    pub fn parse(text: &str) -> Result<Method> {
        match text {
            "GET" => Ok(Method::Get),
            "POST" => Ok(Method::Post),
            other => Err(Error::UnknownMethod(other.to_string())),
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let text = match self {
            Method::Get => "GET",
            Method::Post => "POST",
        };
        write!(f, "{text}")
    }
}
```

`header.rs` only gains the two derives on `Header`:

```rust,proj=rb17,file=crates/rb-core/src/header.rs
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Header {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

impl Header {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), enabled: true }
    }

    pub fn parse(line: &str) -> Result<Header> {
        match line.split_once(':') {
            Some((key, value)) => Ok(Header::new(key.trim(), value.trim())),
            None => Err(Error::BadHeader(line.to_string())),
        }
    }
}

impl From<(&str, &str)> for Header {
    fn from(pair: (&str, &str)) -> Self {
        Header::new(pair.0, pair.1)
    }
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.value)
    }
}
```

`request.rs` gets the derives and three new methods: `to_json`, `save` and `load`. (`&Path` is a *borrowed file location*, the `&str` of file paths. A `PathBuf` is the owned version, like `String`.)

```rust,proj=rb17,file=crates/rb-core/src/request.rs
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::header::Header;
use crate::method::Method;
use crate::variables::resolve;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub method: Method,
    pub path: String,
    pub headers: Vec<Header>,
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} HTTP/1.1", self.method, self.path)?;
        for header in self.headers.iter().filter(|h| h.enabled) {
            write!(f, "\n{header}")?;
        }
        Ok(())
    }
}

impl Request {
    pub fn new(method: Method, path: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            method,
            path: path.to_string(),
            headers: Vec::new(),
        }
    }

    pub fn add_header(&mut self, header: Header) {
        self.headers.push(header);
    }

    pub fn to_text(&self, variables: &HashMap<String, String>) -> String {
        resolve(&self.to_string(), variables)
    }

    pub fn validate(&self) -> Result<()> {
        if !self.path.starts_with('/') {
            return Err(Error::InvalidPath {
                path: self.path.clone(),
                reason: String::from("must start with '/'"),
            });
        }
        Ok(())
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.to_json()?)?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Request> {
        let text = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }
}

pub fn parse_port(text: &str) -> Result<u16> {
    Ok(text.trim().parse::<u16>()?)
}

pub fn build(method: &str, path: &str, header_line: &str) -> Result<Request> {
    let method = Method::parse(method)?;
    let header = Header::parse(header_line)?;
    let mut request = Request::new(method, path);
    request.add_header(header);
    request.validate()?;
    Ok(request)
}
```

Look at `save` and `load`: they are the Nexus `read_json` function again. `std::fs::write` and `read_to_string` can fail with an `io::Error`, serde with a `serde_json::Error`. Both `?`s convert automatically into our `Error`, because of the `#[from]` variants.

`lib.rs` is unchanged from Chapter 16:

<!-- proj=rb17 copy=16_your_own_errors:rb16:crates/rb-core/src/lib.rs>crates/rb-core/src/lib.rs -->

And `main.rs`, with a `run` function that can use `?` (Chapter 8: `?` only works in a function that returns a `Result`):

```rust,proj=rb17,file=crates/rb-cli/src/main.rs
use rb_core::{Error, Header, Method, Request, Result};

fn run() -> Result<()> {
    let mut request = Request::new(Method::Post, "/v2/users");
    request.id = String::from("demo-1");
    request.add_header(Header::new("Host", "api.example.test"));
    request.add_header(Header::new("Content-Type", "application/json"));

    println!("{}", request.to_json()?);

    let file = std::env::temp_dir().join("rb-request.json");
    request.save(&file)?;
    let loaded = Request::load(&file)?;
    println!("round trip equal: {}", loaded == request);

    let missing = std::env::temp_dir().join("rb-does-not-exist.json");
    match Request::load(&missing) {
        Ok(_) => println!("unexpected"),
        Err(e) => println!("missing file is an io error: {}", matches!(e, Error::Io(_))),
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        println!("ERROR: {e}");
    }
}
```

```output,proj=rb17,run=-p+rb-cli
{
  "id": "demo-1",
  "method": "POST",
  "path": "/v2/users",
  "headers": [
    {
      "key": "Host",
      "value": "api.example.test",
      "enabled": true
    },
    {
      "key": "Content-Type",
      "value": "application/json",
      "enabled": true
    }
  ]
}
round trip equal: true
missing file is an io error: true
```

The JSON you see is a structure Nexus would feel at home with. A file you could open in any text editor, share, or put in version control. Open `/tmp/rb-request.json` (or look in your temp folder) and you will find exactly that text.

> **Sharpen your pencil**
> Add a field `body: Option<String>` to `Request`, and make it vanish from the JSON when it is `None`. Which attribute do you need on the field, and which attribute makes it OK to load an *old* file that has no `body` key at all?
>
> <details><summary>Answer</summary>
>
> `#[serde(default, skip_serializing_if = "Option::is_none")] pub body: Option<String>`: the exact line Nexus uses for `description`. `skip_serializing_if` removes the key when it's `None`, and `default` supplies `None` when the key is absent in an old file. (You must also set `body: None` in `Request::new`.)
> </details>

## Bullet Points

* **JSON** is plain text built from objects `{}`, lists `[]`, text, numbers, `true`/`false`, `null`. It maps naturally onto structs, `Vec`s, `String`s and `Option`s.
* **Serialize** = value to text; **deserialize** = text to value.
* The **`serde`** crate defines the `Serialize`/`Deserialize` traits and their derives; **`serde_json`** does JSON: `to_string`, `to_string_pretty`, `from_str`.
* Add `#[derive(Serialize, Deserialize)]` and every field must be serializable too.
* Both directions return a `Result`: bad JSON is an `Err`, never a half-built value.
* Tune the spelling with attributes: `rename_all = "camelCase" | "UPPERCASE"`, `rename = ".."`, `default`, `skip_serializing_if = "Option::is_none"`, and `untagged` for enums.
* Wrap `io::Error` and `serde_json::Error` in your error enum with `#[from]` so `?` works across file and JSON steps.

## Cheat sheet

```text
cargo add serde --features derive      cargo add serde_json
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]     status_code <-> statusCode
#[serde(rename = "type")]              one field
#[serde(default, skip_serializing_if = "Option::is_none")]
let text = serde_json::to_string(&value)?;     to_string_pretty for humans
let value: MyType = serde_json::from_str(&text)?;
std::fs::write(path, text)?;  std::fs::read_to_string(path)?;
```

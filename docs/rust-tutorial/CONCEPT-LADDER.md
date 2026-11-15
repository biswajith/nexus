# The concept ladder

The one rule of this book: **a term, a symbol, or an idea may not be used before the chapter that introduces it**, unless it is flagged on the spot as "ignore this for now, Chapter N explains it". This page is the contract. For each chapter it lists what is **new** and what is **assumed**.

A chapter's checkpoint program (the *request builder*) uses only the concepts on this ladder up to that chapter.

## Chapter by chapter

### Part 1: First steps

**Ch 1: Hello, Cargo!**
New: program, source code, **compiler** (`rustc`), terminal, `fn main() { }`, `println!("..")` and the `!` (**macro**), `;`, double-quoted text, reading a compiler error (headline, `-->` location, `^` pointer, `help:`), **Cargo** (`new`, `run`, `check`, `build`, `--release`), `Cargo.toml` (`[package]`, `[dependencies]`, empty), `src/main.rs`, `target/`, debug vs release, **crate** (light: "a Cargo project's worth of code"), what an HTTP request looks like (request line + header lines), a picture-only tour of Nexus (engine and window, press Send).
Assumes: nothing.

**Ch 2: Values, variables and types**
New: placeholders `{}`, `{name}`, `{:?}`; `let`, variable, value; comments `//`; immutable by default, `mut`; **type**, inference, `: type`; `i32 u8 u16 u32 u64 usize f64 bool char`; arithmetic (`+ - * / %`, integer division); `as`; mismatched-types error; **shadowing**; **method** (preview: `.len()`); tuples (`.0`, `let (a, b) =`); `const` and `UPPER_SNAKE_CASE`; `snake_case`; `&str` (flagged: "ignore the `&` until Chapter 5"); `///` doc comment.
Assumes: Ch 1.

**Ch 3: Functions and decisions**
New: `fn name(param: Type) -> Type`, call, parameter vs argument, last expression without `;` is the result, **expression vs statement**, `()` (unit), `return`; `if / else if / else` (condition must be `bool`), `==  !=  <  >  <=  >=  &&  ||  !`, `if` as an expression; `for n in 1..=3`, `..` vs `..=`, `while`, `loop`, `break`, `+=`.
Assumes: Ch 2.

### Part 2: Owning things

**Ch 4: Ownership**
New: **memory**, **stack** ("desk") and **heap** ("warehouse"), `String`, `String::from`, `::` ("inside"), `.to_string()`, `push_str`, `push`, `\n`, **owner**, **scope**, **drop**, the three ownership rules, **move** (variables and function arguments), **clone**, **Copy** (numbers, `bool`, `char`), `mut` parameters, `format!`, `{:.1}` precision, returning a value to give ownership back.
Flagged forward references: the words "borrow" and "trait" inside quoted compiler errors; `pub` in a Nexus excerpt.
Assumes: Ch 3.

**Ch 5: Borrowing**
New: **reference** `&x`, **borrow**, `&T` vs `&mut T`, `&mut` at the call and in the parameter, the two golden rules (many readers or one writer), borrow lasts until last use, "does not live long enough" (preview of lifetimes), **`&str` explained**, **slice** `&text[0..3]`, `&String` to `&str`, parameters should be `&str`, method chaining, `*r` (reach through a reference), `.replace`.
Assumes: Ch 4.

### Part 3: Modelling data

**Ch 6: Structs and methods**
New: **struct**, **field**, `Struct { field: value }`, `value.field`, a struct owns its fields, `#[derive(Debug)]`, `{:?}` and `{:#?}`, **attribute** (`#[...]`), **warning**, **`impl`** block, **method** (`&self`, `&mut self`, `self`), **associated function** (`new`) and `Type::function()`, `Self`, field init shorthand.
Flagged forward references: the word "trait" in the `Debug` error; `pub` in a Nexus excerpt; the second attribute line of Nexus types.
Assumes: Ch 5.

**Ch 7: Enums and match**
New: **enum**, **variant**, `Enum::Variant`, `match` (arms `=>`, exhaustive), `_`, `|`, ranges as patterns, variants that carry data (`Text(String)`, `Basic { user, .. }`), `..` in patterns, `if let`, `matches!`, `Body::Empty => {}`; struct = AND, enum = OR.
Assumes: Ch 6.

**Ch 8: Option, Result and `?`**
New: **`Option<T>`** (`Some`, `None`), `<...>` read as "of", `match` on text, `.unwrap_or`, `.is_some`, `.is_none`, `unwrap` / `expect`, **panic**, **`Result<T, E>`** (`Ok`, `Err`), `parse`, `_` in a type, the **`?` operator** (on `Result` and `Option`), `return Err(..)`, `Result<(), String>`, `.ok()` and `.split_once` in passing, `.trim`.
Flagged forward references: a literal array `[ .. ]` with a `for` loop in the checkpoint (Ch 9); "libraries" and `url::Url::parse` (Ch 15); error conversion with `?` (Ch 16).
Assumes: Ch 7.

**Ch 9: Collections**
New: **array** `[T; N]` and indexing, index out of bounds panic, **`Vec<T>`** (`new`, `push`, `pop`, `vec!`, `get`, `len`, indexing), `for x in &v / &mut v / v`, a `Vec` owns its items, you cannot hold a reference into a `Vec` while pushing, **`&[T]`** slices of lists, **`HashMap<K, V>`** (`insert`, `get`, `contains_key`), `use std::collections::HashMap;` (preview of `use`).
Assumes: Ch 8.

### Part 4: Reusable code

**Ch 10: Traits**
New: **trait**, `trait X { fn f(&self); }`, `impl Trait for Type`, **`Display`** (`fmt::Formatter`, `write!`, `.to_string()`), **derive explained**, **`Clone`**, **`PartialEq`** (`==`), **`Copy`** (as a trait), **`Default`** (`#[default]`, `..Default::default()`), **`From` / `.into()`**, **`Drop`**, `use std::fmt;`.
Assumes: Ch 9.

**Ch 11: Generics**
New: **generic** functions `fn f<T>`, several blanks `<A, B>`, generic structs and enums, `impl<T> S<T>`, `Option` and `Vec` demystified, **trait bounds** (`T: PartialOrd`, `+`, `where`), **`impl Trait`** in parameters, `impl Into<String>`, zero-cost (monomorphization).
Assumes: Ch 10.

**Ch 12: Lifetimes**
New: **lifetime**, "missing lifetime specifier", `'a`, `fn f<'a>(..)`, what a label means, **elision** rules, labels on only the relevant input, structs with references (`struct S<'a>`, `impl<'a>`), **`'static`**, `'_`, `500..` open range.
Assumes: Ch 11.

**Ch 13: Closures and iterators**
New: **closure** `|x| ..`, **capture** ("backpack"), `move`, `impl Fn(..) -> ..`, `Fn / FnMut / FnOnce`, **iterator**, `.iter()`, `.next()`, **adapters** (`filter`, `map`, `copied`, `cloned`, `rev`, `enumerate` in passing) and **consumers** (`collect`, `count`, `sum`, `any`, `find`), laziness, `.sort()`, `.join()`, `.keys()`, turbofish `::<>`, `Box<dyn Fn>` (peek).
Assumes: Ch 12.

### Part 5: Real projects

**Ch 14: Modules**
New: **module** (`mod name { }`), paths, `pub` (on items and fields), private by default, `use` (and `{ }`, `self`, `super`, `crate`), `pub use`, `mod name;` loads a file, `mod` vs `use`, crate root, `pub(crate)`, folder modules (`mod.rs`).
Assumes: Ch 13.

**Ch 15: Crates, dependencies and workspaces**
New: **binary crate** vs **library crate** (`src/lib.rs`), crates.io, `[dependencies]`, `cargo add`, versions and features, `Cargo.lock`, `[workspace]`, `members`, `[workspace.package]`, `[workspace.dependencies]`, `x.workspace = true`, path dependency, `-p`, `[[bin]]` / `[lib]`, `[profile.release]`, `[dev-dependencies]` (named).
Flagged forward references: `serde` and `thiserror` in a Nexus excerpt (Ch 16, 17).
Assumes: Ch 14.

**Ch 16: Your own error type**
New: error enum, `impl std::error::Error`, `?` converts errors through `From`, "`?` couldn't convert the error", **`thiserror`** (`#[derive(Error)]`, `#[error("..")]`, `#[from]`), `type Result<T> = ..` (type alias), enums inside libraries and strings at the edge.
Assumes: Ch 15.

**Ch 17: Saving data: JSON and serde**
New: **JSON**, **serialize / deserialize**, `serde`, `serde_json`, `Serialize` / `Deserialize`, `to_string`, `to_string_pretty`, `from_str`, `#[serde(rename_all, rename, default, skip_serializing_if, untagged)]`, `std::fs::write` / `read_to_string`, `Path`, `PathBuf`, `.unwrap_err()`.
Assumes: Ch 16.

### Part 6: Many things at once

**Ch 18: Threads**
New: **thread**, `thread::spawn`, `join`, `thread::sleep`, `Duration`, `move` closures for threads, **`Arc`** (`Arc::new`, `Arc::clone`), **`Mutex`** (`lock`, guard, poisoning), `Arc<Mutex<T>>`, **channels** (`mpsc::channel`, `tx`, `rx`, `send`, `recv`, `try_recv`, `try_iter`, `iter`, `tx.clone()`, `drop(tx)`), messages as enums, **`Send` / `Sync`**, data races versus deadlocks.
Assumes: Ch 17.

**Ch 19: Async and tokio**
New: **future**, `async fn`, `.await`, **runtime**, `tokio`, `#[tokio::main]`, lazy futures, `tokio::join!`, `tokio::spawn` and `async move`, tasks, `spawn_blocking`, `tokio::time::sleep` / `timeout`.
Assumes: Ch 18.

**Ch 20: Putting it together**
New: **immediate-mode** GUI and **frame**, the `Msg` / `Runtime` / `spawn` / `drain` bridge (all built from Ch 7, 12, 13, 18, 19), `let ... else`, building a runtime by hand.
Assumes: Ch 19.

### Part 7: Quality

**Ch 21: Testing**
New: `#[test]`, `cargo test`, `assert!`, `assert_eq!`, `assert_ne!`, Arrange-Act-Assert, `#[cfg(test)] mod tests`, `use super::*`, integration tests in `tests/`, `#[should_panic]`, tests returning `Result`, `#[tokio::test]`, `[dev-dependencies]` used, `cargo test -- --nocapture`.
Assumes: Ch 20.

**Ch 22: Where to go next**
New: nothing new in Rust. Glossary, reading order, a critical read of Nexus. A few bonus words are named for further study (`Rc`, `RefCell`, `macro_rules!`, `unsafe`, `Pin`).
Assumes: everything.

## Machine-checked term index

This table is the exact list the "nothing before its chapter" check searches for. A match in an earlier chapter must be one of the flagged forward references named above. The *Pattern* is a regular expression.

| Term | Pattern | Ch |
|---|---|---|
| variable | `\bvariables?\b` | 2 |
| mutable | `\bmut\b` | 2 |
| shadowing | `\bshadow` | 2 |
| tuple | `\btuples?\b` | 2 |
| `&str` | `&str\b` | 2 |
| String | `\bString\b` | 4 |
| heap | `\bheap\b` | 4 |
| stack | `\bstack\b` | 4 |
| ownership | `\bownership\b` | 4 |
| owner | `\bowners?\b` | 4 |
| move (value) | `\bmoved?\b` | 4 |
| clone | `\bclone\b` | 4 |
| borrow | `\bborrow` | 5 |
| reference | `\breferences?\b` | 5 |
| `&mut` | `&mut\b` | 5 |
| slice | `\bslices?\b` | 5 |
| struct | `\bstructs?\b` | 6 |
| impl | `\bimpl\b` | 6 |
| derive | `\bderive\b` | 6 |
| attribute | `#\[` | 6 |
| Self | `\bSelf\b` | 6 |
| enum | `\benums?\b` | 7 |
| match | `\bmatch\b` | 7 |
| variant | `\bvariants?\b` | 7 |
| Option | `\bOption\b` | 8 |
| Result | `\bResult\b` | 8 |
| unwrap | `\bunwrap` | 8 |
| panic | `\bpanic` | 8 |
| `?` operator | `\)\?[;.)]` | 8 |
| Vec | `\bVec\b` | 9 |
| array | `\barrays?\b` | 9 |
| HashMap | `\bHashMap\b` | 9 |
| trait | `\btraits?\b` | 10 |
| Display | `\bDisplay\b` | 10 |
| PartialEq | `\bPartialEq\b` | 10 |
| Default | `\bDefault\b` | 10 |
| From | `\bFrom[<:(]` | 10 |
| Drop | `\bDrop\b` | 10 |
| generic | `\bgenerics?\b` | 11 |
| trait bound | `\btrait bound` | 11 |
| lifetime | `\blifetimes?\b` | 12 |
| lifetime label | `'a\b` | 12 |
| `'static` | `'static` | 12 |
| closure | `\bclosures?\b` | 13 |
| iterator | `\biterators?\b` | 13 |
| `.iter()` | `\.iter\(\)` | 13 |
| `.collect` | `\.collect\b` | 13 |
| `.map(` | `\.map\(` | 13 |
| `.filter(` | `\.filter\(` | 13 |
| module | `\bmodules?\b` | 14 |
| mod | `\bmod\b(?!\.rs)` | 14 |
| pub | `\bpub\b` | 14 |
| `crate::` | `\bcrate::` | 14 |
| library crate | `library crate\|lib\.rs\|\[lib\]` | 15 |
| workspace | `\bworkspaces?\b` | 15 |
| crates.io | `crates\.io` | 15 |
| thiserror | `\bthiserror\b` | 16 |
| serde | `\bserde\b` | 17 |
| Serialize | `\bSerialize\b` | 17 |
| JSON | `\bJSON\b` | 17 |
| thread | `\bthreads?\b(?! '\w+')` | 18 |
| Arc | `\bArc\b` | 18 |
| Mutex | `\bMutex\b` | 18 |
| channel | `\bchannels?\b` | 18 |
| mpsc | `\bmpsc\b` | 18 |
| async | `\basync\b` | 19 |
| await | `\bawait\b` | 19 |
| tokio | `\btokio\b` | 19 |
| future | `\bfutures?\b` | 19 |
| immediate mode | `[Ii]mmediate[- ]mode` | 20 |
| `#[test]` | `#\[test\]` | 21 |
| assert | `\bassert(_eq)?!` | 21 |

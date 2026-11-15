# Chapter 22: Where to go next

> **Brain Power**
> Without looking back: explain to an imaginary friend, in one sentence each, (1) what *ownership* is, (2) why `?` exists, and (3) what happens between pressing **Send** in Nexus and seeing the response. If you got stuck on one, that's your next chapter to re-read.

You made it. Twenty-one chapters ago `fn main() { println!("Hello"); }` was a new idea. Now you can open a 48,000-line Rust project and *read* most of it. Let's make that concrete.

## What you can do now

Look at how the one small program you've been growing, the request builder, mirrors Nexus:

| Request builder | Nexus | Chapter |
|---|---|---|
| `Request`, `Header` structs | `NexusRequest`, `KeyValuePair` | 6 |
| `Method` enum with `match` | `HttpMethod`, `Msg` | 7 |
| `Option`, `Result`, `?` | everywhere | 8, 16 |
| `Vec<Header>`, `HashMap` of variables | `headers`, `loaded` collections | 9 |
| `Display`, `Default`, `From`, derives | the same derive lines | 10 |
| `{{host}}` resolution | `VariableStore::resolve` | 9, 13 |
| `rb-core` + `rb-cli` workspace | `nexus-core` + `nexus-gui` | 15 |
| `Error` enum with `thiserror` | `error.rs` | 16 |
| `serde` save and load | `~/.nexus` JSON files | 17 |
| `send_all` with threads and `Arc<Mutex>` | `WebSocketClient`, `Shared` | 18 |
| `send_all_async` with timeouts | `send_http_request` | 19 |
| `bridge.rs` | `runtime.rs` | 20 |
| `cargo test --workspace` | ~830 tests | 21 |

## Read Nexus in this order

![Read Nexus in this order](img/ch22-reading-order.svg)

Start with small, pure code; end with the big concurrent parts. For each file, first read the doc comments, then the function signatures, and only then the bodies.

| # | File | What you'll practice | Book chapters |
|---|---|---|---|
| 1 | [`nexus-core/src/error.rs`](../../crates/nexus-core/src/error.rs) | a short error file with `thiserror` | 16 |
| 2 | [`nexus-core/src/types/mod.rs`](../../crates/nexus-core/src/types/mod.rs) | structs, enums, derive, serde attributes | 6, 7, 10, 17 |
| 3 | [`nexus-core/src/trust.rs`](../../crates/nexus-core/src/trust.rs) | pure functions, iterators, `Option`, lifetimes; then read `trust/tests.rs` | 8, 12, 13, 21 |
| 4 | [`nexus-core/src/fs/mod.rs`](../../crates/nexus-core/src/fs/mod.rs) | `Result`, `?`, `match` on modes | 8, 16 |
| 5 | [`nexus-core/src/environments/variable_store.rs`](../../crates/nexus-core/src/environments/variable_store.rs) | `impl` blocks, closures, `for` loops | 6, 9, 13 |
| 6 | [`nexus-core/src/collections/mod.rs`](../../crates/nexus-core/src/collections/mod.rs) | file I/O, `match` guards, `let ... else`, generic helpers | 7, 11, 16, 20 |
| 7 | [`nexus-core/src/http/request_sender.rs`](../../crates/nexus-core/src/http/request_sender.rs) | async/await, `timeout`, `while let` | 19 |
| 8 | [`nexus-gui/src/runtime.rs`](../../crates/nexus-gui/src/runtime.rs) then [`app.rs`](../../crates/nexus-gui/src/app.rs) (`send_active`, `poll`) | threads, channels, `Arc`, `move` | 18, 19, 20 |
| 9 | [`nexus-core/src/websocket/mod.rs`](../../crates/nexus-core/src/websocket/mod.rs) | `Arc<Mutex>`, channels, `select!`, `Drop` | 10, 18, 19 |

(`while let` and `select!` are small steps beyond this book. `while let Some(x) = it.next() { ... }` is `if let` in a loop, and `select!` waits for *whichever of several futures finishes first*.)

Bonus rounds: [`editor/model.rs`](../../crates/nexus-gui/src/editor/model.rs) (a text editor's brain as pure functions plus unit tests, no UI at all) and the tests in [`crates/nexus-gui/tests/`](../../crates/nexus-gui/tests).

## Try-it ideas (small, safe)

* Run `cargo test -p nexus-core trust`, then deliberately break `origin_of` and watch which tests fail. Revert.
* Add a new unit test next to `origin_of` for a URL with a username (`https://user@host/`). Predict the result first.
* Add a `println!` (or its debugging cousin `dbg!`, which prints a value together with its file and line) to `Runtime::drain` in a debug build and watch messages arrive while you click around.
* Find three places using `?` and trace which `From` conversion makes each compile.
* Choose any `match` over `HttpMethod` and add a pretend `Trace` variant. Count the compile errors. (Revert afterwards!)

> Do these on a scratch git branch so you can throw changes away.

## Reading real-world code: what a reviewer noticed

A book that only shows good code isn't honest. Real programs have seams. A read-through of Nexus found strengths *and* things worth discussing. This section shows how to think critically about Rust code. The rule I used: **I only list what I could confirm by opening the files.** (The snippets use a few library methods this book didn't teach, such as `and_then` and `ok`. You can follow the ideas without them.)

![Four things a reviewer noticed](img/ch22-reviewer.svg)

First the good news, because it matters: Nexus keeps a clean layering (a window-free engine behind a single `App` facade), has an unusually strong test suite, uses no `unsafe`, and is careful about security (owner-only file modes, path-traversal guards, a "trust" model for imported collections). The four observations below are about maintainability and robustness, not "this is a bad project".

### 1. Duplicated logic: two `resolve_request` functions

Variables like `{{token}}` must be substituted into a request before it's sent. You wrote the same job yourself in Chapter 9. In Nexus, two *different* functions with the same name do it:

* [`nexus-gui/src/payload.rs`](../../crates/nexus-gui/src/payload.rs) (`pub fn resolve_request`), used by the single-request path: it resolves the URL, header and param **keys and values**, the raw body, urlencoded and form-data rows, the binary path, and GraphQL fields.
* [`nexus-core/src/runner/mod.rs`](../../crates/nexus-core/src/runner/mod.rs) (a private `fn resolve_request`), used by the collection runner: it resolves only enabled header **values**, the body, enabled urlencoded rows and GraphQL, and it skips form-data and binary paths. The runner sends the body it gets back from its script chain, and that body is built by `extract_body` in `scripts/script_runner.rs`, which ends with `_ => None` for the modes it doesn't list (form-data and binary), so those bodies appear to be dropped in runs.

Two near-copies that have *drifted apart* mean the same request can behave differently when sent from the tab versus from the runner. That's the classic cost of duplication: fix a bug in one, forget the other.

**What to ask:** "Is there one pipeline both callers should share?" A Rust-flavoured fix: move one `resolve_request` into `nexus-core` and call it from both, plus tests that run the *same* request through both paths.

### 2. Non-atomic file writes

All persistence goes through [`fs/mod.rs`](../../crates/nexus-core/src/fs/mod.rs), which is where the owner-only permissions are enforced. Good! But look at what "replace a file" does:

```rust,nexus
match mode {
    WriteMode::Replace => opts.create(true).truncate(true),
    WriteMode::CreateNew => opts.create_new(true),
    WriteMode::Append => opts.create(true).append(true),
};
let mut handle = opts.open(file)?;
handle.set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))?;
handle.write_all(data)
```

A `match` on a mode enum (Chapter 7) and three `?`-style steps (Chapter 8). `truncate(true)` empties the existing file *first*, then writes the new bytes. If the process is killed or the machine loses power between those two steps, you're left with an empty or half-written file. And look at how loaders treat unreadable files, e.g. [`ui_state.rs`](../../crates/nexus-gui/src/state/ui_state.rs):

```rust,nexus
let mut s: Self = std::fs::read_to_string(Self::path(home))
    .ok()
    .and_then(|t| serde_json::from_str(&t).ok())
    .unwrap_or_default();
```

`.ok()` swallows the error (Chapter 8) and `unwrap_or_default()` quietly substitutes defaults (Chapter 10). For UI layout that's a fair choice. For a collection request file it can mean **silent data loss**: a damaged file just vanishes from the list.

**What to ask:** "What happens if this is interrupted halfway?" The standard remedy is *write to a temporary file in the same folder, then `rename` over the original*: renames are atomic on common filesystems, so readers see either the old file or the new one, never half of each. Another question this raises: is swallowing an error with `.ok()` always right, or should at least a warning be logged?

### 3. Blocking work on the UI thread

The project's own [conventions](../../.cursor/rules/project-conventions.mdc) say: "Never block the UI thread... File reads and heavy parsing go to `spawn_blocking`." And mostly they do, as you saw in Chapters 19 and 20. But follow the first lines of `send_active` in [`app.rs`](../../crates/nexus-gui/src/app.rs):

```rust,nexus
pub fn send_active(&mut self) {
    if let Some(dir) = self
        .tabs
        .active()
        .and_then(|t| t.origin.as_ref().map(|o| o.dir_name.clone()))
    {
        self.collections.recheck_trust(&self.app, &dir);
    }
    // ...
```

`recheck_trust` calls `app.collection_trust_status(dir)`, which calls `CollectionManager::trust_status`, which calls `content_hash`, which in [`collections/mod.rs`](../../crates/nexus-core/src/collections/mod.rs) does:

```rust,nexus
let c = self.load_collection(dir_name)?;
Ok(hex::encode(Sha256::digest(serde_json::to_vec(&c)?)))
```

That's: **read the whole collection from disk, parse it, re-serialize it and SHA-256 hash it**, all synchronously inside the click handler, *on the UI thread*, before the request is even spawned. For a small collection it's imperceptible. For a *trusted* collection with thousands of requests, the window can stutter on every Send. (`trust_status` returns `Untrusted` early when the collection isn't in the trusted map, so only trusted ones pay for the hash.)

A clearer, unconditional example is a few hundred lines away in the same file: `save_response_to` (`app.rs`) calls `std::fs::write` directly with the response body, which can be up to 100 MiB (the constant you decoded in Chapter 2), *on the UI thread*. The window freezes until the disk finishes, and the write also bypasses `nexus_core::fs`, the module the conventions say all file writes should use.

It's a good example of a rule that's followed 95% of the time. The fix would be to run the check inside the async task (it already has a `spawn_blocking` for building the payload), or to cache the hash and invalidate it when files change.

**What to ask:** "Where does I/O happen relative to the thread that must stay responsive?" Reading code with that question in mind finds issues no compiler can.

### 4. Stale comments from the previous implementation

Nexus is a port of an earlier TypeScript/Electron app, and many comments still speak to that history. For example, the first lines of [`types/mod.rs`](../../crates/nexus-core/src/types/mod.rs):

```rust,nexus
//! Serde port of `packages/core/src/types/index.ts`. JSON produced/consumed here must match the
//! TypeScript shapes exactly (camelCase keys, omitted optionals, same enum strings).
```

and `fs/mod.rs` (`port of packages/core/src/fs/private-fs.ts`, plus "the `'w' | 'wx' | 'a'` open flags of the TS"), `websocket/mod.rs` ("What the TS client emitted as..."), `runner/mod.rs` ("like the TS `run()`"), `scripts/mod.rs` (`Port of packages/core/src/scripts`). A `grep` for "TS" and "port of" in the source finds a few dozen such lines.

Some of them are valuable (the JSON format *really must* stay compatible with the old app's files: Chapter 17's `camelCase`). But a reader who doesn't have `packages/core/` can't follow them, and references like "as in the TS" explain nothing on their own. Comments rot silently because the compiler never checks them.

**What to ask:** "Does this comment still tell the truth, and can a newcomer act on it?" Keep the *reason* (e.g. "keys must be camelCase so old collection files load") and drop the pointer to a repository the reader doesn't have.

### How to read code critically (the takeaway)

1. **Start from the promises** (docs, conventions) and check whether the code keeps them.
2. **Look for twins.** Same name or same job in two places? Compare them.
3. **Ask "what if it stops here?"** for every write, every network call, every lock.
4. **Follow the thread.** Which thread runs this? What does it wait on?
5. **Weigh the cost.** Each observation above is *fine* for a small collection and a healthy disk. A reviewer's job is to know where the edges are, and then decide with eyes open.
6. **Praise specifically.** Notice what's done well (a never-panic runner, hash-bound trust, a clock passed as a parameter) and copy it.

> **Sharpen your pencil**
> Pick **one** of the four findings and write 3 sentences: (a) the scenario in which it hurts a user, (b) the smallest change that would fix it, (c) the test you'd write first.
>
> <details><summary>Sample answer (finding 2, non-atomic writes)</summary>
>
> (a) The user saves a request just as the laptop battery dies; on the next launch `collection.json` is empty, the loader returns nothing, and the collection appears to have vanished. (b) Add a new `WriteMode` variant (say `AtomicReplace`) that writes to `<name>.tmp` in the same directory and `rename`s it over the target. (c) A test that calls the new mode, checks that no `.tmp` file is left behind, and that after a simulated failure the target either still has the old content or has the new content, never an empty file.
> </details>

> **Watch it!**
> Critical reading is not nitpicking. A finding is only worth reporting if you can point at the line, say what goes wrong, and say *when*. Before you call something a bug, try to prove yourself wrong: finding #3 above first looked like "every Send stutters" until a closer read showed an early return for untrusted collections. Check the claim, then shrink it to what is actually true.

> **There are no Dumb Questions**
>
> **Q: Is it rude to find problems in code someone else wrote?**
> A: No. Every real codebase has them, and Nexus is no exception. Naming them kindly, with a suggested fix, is how projects get better.
>
> **Q: I can't understand a whole file. Am I doing it wrong?**
> A: No. Read doc comments and signatures first, skip bodies you don't need, and come back later. Nobody reads 48,000 lines in one go.
>
> **Q: Should I rewrite things the way I'd do them?**
> A: Not at first. Make one small change on a scratch branch, run the tests, and see what breaks.

## Glossary

Every word the book taught, with the chapter that introduces it.

| Term | Plain-language meaning | Ch. |
|---|---|---|
| **Arc** | Shared ownership of a value between threads, by counting handles | 18 |
| **Array** | A fixed-length row of values of one type, `[T; N]` | 9 |
| **async / await** | A way to write code that waits without blocking its worker | 19 |
| **Attribute** | A `#[...]` note to the compiler, such as `#[derive(Debug)]` | 6 |
| **Borrow** | Using a value through a reference without owning it: `&T` / `&mut T` | 5 |
| **Box** | A ticket to a value stored in the warehouse (heap); used for `Box<dyn Fn..>` | 13 |
| **`?` operator** | On `Err`/`None` return it from this function now, else unwrap the value | 8 |
| **Cargo** | Rust's project manager and build tool | 1 |
| **Channel** | A post box for sending messages between threads or tasks | 18 |
| **Clone** | Make an explicit independent copy: `.clone()` | 4 |
| **Closure** | A small unnamed function that can capture variables: `\|x\| x + 1` | 13 |
| **Compiler** | The translator from source code to a program (`rustc`) | 1 |
| **Copy** | A small type that is copied automatically instead of moved | 4, 10 |
| **Crate** | One compilable package of code: a binary or a library | 1, 15 |
| **Derive** | Ask the compiler to write a trait's code for you | 6, 10 |
| **Drop** | Cleanup when an owner's scope ends; the trait lets you add your own | 4, 10 |
| **Enum** | A type that is exactly one of several variants | 7 |
| **Expression / statement** | Code that produces a value / code that just does something (`;`) | 3 |
| **Field** | One named part of a struct | 6 |
| **Future** | Work that isn't finished yet, which you can `.await` | 19 |
| **Generic** | Code written with a type blank like `<T>` | 11 |
| **HashMap** | A key-to-value dictionary with no order | 9 |
| **Heap / stack** | The warehouse (any size, grows) / the desk (small, fixed, fast) | 4 |
| **Immediate mode** | A GUI that redraws everything from current data every frame | 20 |
| **Iterator** | Something that hands out items one at a time; adapters and consumers | 13 |
| **JSON** | A plain-text data format of objects, lists, text, numbers | 17 |
| **Lifetime** | How long a reference is valid, named like `'a` | 12 |
| **Macro** | A built-in helper written with `!`, like `println!` | 1 |
| **Match** | Pick a branch by pattern; must cover every case | 7 |
| **Method** | A function attached to a value, called with a dot | 2, 6 |
| **Module** | A named container of code (`mod`), private unless `pub` | 14 |
| **Move** | Handing over ownership; the old name stops working | 4 |
| **Mutable (`mut`)** | Allowed to change | 2 |
| **Mutex** | A lock so only one thread uses a value at a time | 18 |
| **Option** | `Some(value)` or `None`: "there might be nothing" | 8 |
| **Ownership** | Every value has exactly one owner; dropped when the owner ends | 4 |
| **Panic** | The program stops with a message, e.g. `unwrap()` on `None` | 8 |
| **Reference** | A value that points at another without owning it: `&x` | 5 |
| **Result** | `Ok(value)` or `Err(error)`: "this may fail, and here's why" | 8 |
| **Runtime** | The engine (tokio) that runs many futures on a few threads | 19 |
| **Scope** | The region of code where a variable exists | 4 |
| **Send / Sync** | Labels: may move to another thread / may be shared between threads | 18 |
| **serde** | The crate that turns structs into JSON and back | 17 |
| **Shadowing** | A new `let` with an old name makes a new variable | 2 |
| **Slice** | A borrowed look at part of a text or list: `&str`, `&[T]` | 5, 9 |
| **`'static`** | Lives for the whole program, like quoted text | 12 |
| **Struct** | A type that bundles named fields | 6 |
| **Task** | A cheap unit of async work, started with `tokio::spawn` | 19 |
| **Thread** | An independent line of work inside a program | 18 |
| **Trait** | A named list of methods a type can promise; an ability | 10 |
| **Tuple** | A small bundle of values, possibly of different types | 2 |
| **Type** | The kind of a value: number, text, `bool`... | 2 |
| **Unit `()`** | "Nothing": what a statement produces | 3 |
| **Variable** | A name attached to a value | 2 |
| **Vec** | A growable list: `Vec<T>` | 9 |
| **Workspace** | Several crates managed together by Cargo | 15 |

### Words you'll meet next

A few things that appear in other people's Rust code and are good next topics: **`Rc` and `RefCell`** (shared ownership and runtime-checked mutation inside a single thread), **`macro_rules!`** (writing your own macros), **`unsafe`** (code where *you*, not the compiler, vouch for safety; Nexus has none), **blanket implementations** (an `impl` that covers every type with some ability), and **`Pin`** (needed when you write futures by hand).

## Keep learning

* **The Rust Book**: <https://doc.rust-lang.org/book/>. The official, thorough companion to this book.
* **Rust by Example**: <https://doc.rust-lang.org/rust-by-example/>.
* **Rustlings**: <https://github.com/rust-lang/rustlings>. Small broken programs to fix, with instant feedback.
* **Tokio tutorial**: <https://tokio.rs/tokio/tutorial>. Chapter 19, much deeper.
* **egui**: <https://www.egui.rs/>. Try the web demo, then read Nexus's `panels/` code.
* **Standard library docs**: <https://doc.rust-lang.org/std/>. Learn to love the search box (press `s`).
* Command to open the docs of everything you depend on: `cargo doc --open`.

## Bullet Points

* Read Nexus from small and pure (error, types, trust) to big and concurrent (runtime, websocket).
* Break things on a scratch branch; let the compiler and the tests teach you.
* Reading code critically means checking promises, finding twins, and asking "what if it stops here?"
* Good projects have seams. Name them kindly, and weigh them honestly.

## Cheat sheet (the whole book on one card)

```text
let / let mut         variables            struct + impl        data + behaviour
enum + match          one-of + sorting     Option / Result / ?  no null, no hidden exceptions
ownership: move/clone borrowing: & / &mut  lifetimes: 'a        references can't outlive owners
trait + generics      derive + serde       iter().map().collect() closures |x| ..
mod / pub / use       Cargo workspace      Arc<Mutex<T>>        mpsc channel
async/.await + tokio  spawn_blocking       #[test] + cargo test
```

Happy hacking. Go ship something.

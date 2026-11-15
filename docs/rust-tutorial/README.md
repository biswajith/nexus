# Head First Rust, with Nexus

*A friendly, picture-heavy introduction to Rust, taught with a real program: Nexus, a local-first API client (think "Postman, but a single native Rust app").*

![Your journey through this book](img/ch01-learning-path.svg)

---

## Who is this book for?

You have **never seen Rust**. Maybe you have never seen a "systems" language at all. Maybe you know a bit of Python or JavaScript and the words *"ownership"* and *"borrow checker"* sound like something from a law firm.

Good. This book is for you.

You will not build toy programs only. You will keep peeking into **Nexus**, a real project of about 48,000 lines of Rust (and roughly 830 tests), and discover that the scary-looking code is built from the same few ideas you just learned.

## How this book works

Every chapter has the same furniture:

| Device | What it is |
|---|---|
| **Brain Power** | A question at the top of the chapter. Think about it *before* reading on. Wrong guesses help you learn. |
| **Sharpen your pencil** | A short exercise. The answers are at the **end of the same chapter**. Try first! |
| **There are no Dumb Questions** | The questions you were too polite to ask. |
| **Bullet Points** | A recap of what just happened. |
| **Watch it!** | A trap that catches almost every beginner. |
| **Cheat sheet** | One-screen summary at the chapter's end. |

And lots of **pictures** (SVG files in [`img/`](img/)). If a paragraph confuses you, look at the picture, then re-read.

> Nexus snippets are marked with a link such as [`error.rs`](../../crates/nexus-core/src/error.rs). Open the file next to the book. Line numbers drift as the project changes, so snippets are quoted in full; search for them with your editor.

## Table of contents

1. [Hello, Cargo! A tour of Nexus](01-hello-cargo-and-nexus.md)
2. [Variables, types and mutability](02-variables-types-mutability.md)
3. [Functions, structs and `impl`](03-functions-structs-impl.md)
4. [Enums and pattern matching](04-enums-and-pattern-matching.md)
5. [`Option`, `Result`, `?` and errors](05-option-result-errors.md)
6. [Ownership](06-ownership.md)
7. [Borrowing and lifetimes](07-borrowing-and-lifetimes.md)
8. [Traits, generics, derive and serde](08-traits-generics-serde.md)
9. [Collections, iterators and closures](09-collections-iterators-closures.md)
10. [Modules, crates and Cargo workspaces](10-modules-crates-workspaces.md)
11. [Concurrency: threads, `Arc`/`Mutex`, channels](11-concurrency.md)
12. [Async/await, tokio, and talking to the egui UI](12-async-tokio-egui.md)
13. [Testing](13-testing.md)
14. [Where to go next (glossary, reading order, and what a reviewer noticed)](14-where-to-go-next.md)

## Installing Rust

Rust ships with a tool called **rustup** that installs and updates everything.

```bash
# Linux / macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# then restart your terminal and check:
rustc --version
cargo --version
```

(On Windows, download the installer from <https://rustup.rs>.) You get two main tools:

* **`rustc`**, the compiler. It turns `.rs` text files into a program.
* **`cargo`**, the project manager. It runs `rustc` for you, downloads libraries ("crates"), runs tests, and more. You will mostly type `cargo`.

> **Watch it!** Nexus needs a *recent* stable Rust (it uses newer library functions such as `Option::is_none_or`). The repository does not pin a version, so if the build complains about unknown methods, run `rustup update`. This book was checked with `rustc 1.99` / `cargo 1.99`.

## Running and testing Nexus

From the repository's own [README](../../README.md): Nexus is a native Linux app. You need Rust, plus a Wayland or X11 session with OpenGL (Mesa). The usual runtime libraries (libxkbcommon, libwayland or libX11, libEGL, libGL) are expected to be on your machine. No Node.js, no webview.

Get the code and run it:

```bash
cd nexus                    # the repository folder
./start.sh                  # release build + run (first build is slow)
cargo run -p nexus-gui      # debug build + run (faster to compile, enables dev flags)
cargo build --release -p nexus-gui   # binary lands in target/release/nexus
```

Run the test suites (from the repo's [conventions](../../.cursor/rules/project-conventions.mdc)):

```bash
cargo test --workspace                                  # all tests
cargo test -p nexus-core trust                          # only tests with "trust" in their name
cargo fmt --all                                         # auto-format
cargo clippy --workspace --all-targets -- -D warnings   # the strict linter
```

> The first build downloads and compiles many crates and can take minutes. Later builds are incremental and fast.

> **Note on platform:** the code uses Unix file-permission APIs (`std::os::unix`) in [`fs/mod.rs`](../../crates/nexus-core/src/fs/mod.rs), so Nexus builds on Linux/macOS, not Windows.

## Trying the small examples

Many examples in this book are tiny **standalone programs**. Save one as `hello.rs` and run:

```bash
rustc hello.rs && ./hello
```

or make a playground project with `cargo new playground` and paste the code into `src/main.rs`, then `cargo run`.

A few examples need outside crates (`serde`, `tokio`). Those say `// needs-deps` at the top; in a `cargo new` project, add the crates with `cargo add serde --features derive`, `cargo add serde_json`, `cargo add thiserror`, `cargo add tokio --features rt-multi-thread,macros,sync,time`.

Ready? Turn the page to [Chapter 1](01-hello-cargo-and-nexus.md).

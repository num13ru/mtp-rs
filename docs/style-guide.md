# Style Guide

Coding conventions for mtp-rs. Run `just` before committing.

## 1. Formatting

Use `cargo fmt` with project defaults. Line length 100 chars, 4-space indent, trailing commas in multi-line constructs.

## 2. Linting

`cargo clippy --all-targets --all-features -- -D warnings` must pass.

- Fix warnings rather than suppress them
- If suppression is necessary, add an explanatory comment above the `#[allow(...)]`
- Never blanket-allow at crate/module level

## 3. Naming

Standard Rust: `snake_case` for functions/variables/modules, `CamelCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants.

- Descriptive names in public APIs (`transaction_id` not `tx_id`), common abbreviations OK internally (`ctx`, `buf`, `len`, `id`)
- No `_async` suffix on async functions
- Boolean getters: `is_`, `has_`, `can_`, `supports_` prefixes

## 4. Docs

- All public items need `///` doc comments
- Include examples for entry points and non-obvious behavior
- Internal code: comment the **why**, not the **what**; no comment is fine if code is clear
- Cross-reference with `[`TypeName`]` rustdoc syntax

## 5. Error Handling

- Return `Result<T, Error>` using crate's error types
- Use `thiserror` for error definitions
- Error messages should include context for debugging (e.g., "expected {}, got {}")
- Prefer `Result` over panic; only panic for true invariant violations
- Never panic on user input or device responses

## 6. API Design

- Prefer `&str` over `String` in parameters; use `impl Into<String>` for flexibility
- Builder pattern for types with multiple optional configs (implement `Default`, chain with `self`, consume on final method)
- Keep public API minimal; use `pub(crate)` for internal sharing

## 7. Async Code

- Use `async-trait` for async trait methods
- **Runtime-agnostic**: use `futures` crate, not tokio/async-std directly (e.g., `futures::lock::Mutex`, `futures_timer::Delay`)
- Document cancellation safety for operations holding resources

## 8. Testing

- Unit tests: `#[cfg(test)] mod tests` in same file
- Integration tests: `tests/` directory, mark with `#[ignore]` for hardware tests
- Use `serial_test::serial` to prevent concurrent device access
- Use `proptest` for property-based testing where applicable

## 9. Git

- Short imperative commit titles (50 chars max): "Add streaming download support"
- One logical change per commit; don't mix formatting with functional changes
- Run `just` before committing

## 10. Pull Requests

- Run `just` before opening
- Short title (<=50 chars), body with Summary, Changes, and Test plan sections
- One feature/fix per PR

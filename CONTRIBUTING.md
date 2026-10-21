# Contributing

## Workflow

- `main` is always buildable; CI (fmt, clippy with warnings as errors, tests, release build) runs on every push and pull request.
- Before pushing: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- On macOS only the layout engine builds: `cargo test -p oz-core`.

## Where code goes

- Anything that can be decided from window ids and rectangles belongs in `oz-core`, with a test. The compositor should only translate between Wayland and `oz-core`.
- When porting a behaviour from the macOS version, keep the names and test values of the Swift code where they make sense, so the two stay easy to compare.

## Style

- `rustfmt.toml` sets the format (120 columns).
- Comments explain why and what a thing means for the user, not what the next line does.
- Commit messages: a short imperative summary line, optional body explaining the reason.

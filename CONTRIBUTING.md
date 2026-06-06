# Contributing to LoggerLog

Thanks for your interest in contributing! This guide will help you get started.

## Development Setup

```bash
# Clone the repository
git clone https://github.com/Mzaxd/loggerlog.git
cd loggerlog

# Build
cargo build

# Run tests
cargo test

# Run a specific module's tests
cargo test --lib core::scanner
```

## Project Architecture

LoggerLog follows a strict layered architecture:

```
src/main.rs → src/lib.rs → pub fn run() → cli::run()
                                    ├── cli/      Thin adapter: parse args → call core → format output
                                    └── core/     Pure library, zero clap dependency
```

- **`core/`** — All business logic. Uses `anyhow::Result<T>` for error propagation. Can be used as an independent library.
- **`cli/`** — Thin adapter layer. Parses CLI arguments via clap, calls core functions, formats output.

When adding new features:
- **Business logic** always goes in `core/`
- **CLI flags and output formatting** go in `cli/`
- Never add clap dependencies to core/

## Key Design Decisions

### Memory Filtering

`level`/`timestamp`/`thread`/`logger` filters are applied in Rust memory (not SQL):
- `build_where_clause()` handles FTS, source, project, module, exclude (SQL layer)
- `matches_memory_filters()` handles level/after/before/thread/logger (Rust layer)
- FTS5 queries are wrapped in `"..."` (literal phrase, no operators)

### Scan Limits

- Search: `DEFAULT_SCAN_LIMIT = 100_000` lines
- Aggregation: `AGGREGATION_SCAN_LIMIT = 500_000` lines

## Code Style

- Follow existing code patterns and naming conventions
- Use `///` doc comments on public functions
- Error handling: use `anyhow::Result`, never `unwrap()` in library code
- Keep functions focused and small

## Testing

Tests are inline in each module's `#[cfg(test)] mod tests`, plus integration tests in `tests/`.

### Adding Unit Tests

Add tests to the relevant module's test block:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_new_feature() {
        // ...
    }
}
```

### Adding Integration Tests

Add to `tests/integration_test.rs` or create a new test file in `tests/`.

Use fixture files in `tests/fixtures/` and expected results in `tests/fixtures/<category>/expected.json`.

### Running Tests

```bash
cargo test                          # All tests (~267)
cargo test --lib core::scanner      # Single module
cargo test --test integration_test  # Integration tests only
cargo test -- --ignored             # Include #[ignore] tests
```

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add multi-line log collapse support
fix: handle empty FTS query gracefully
docs: update README with new filter syntax
test: add integration tests for JSON log format
refactor: extract query parsing into dedicated function
chore: update dependencies
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Make changes with tests
4. Ensure all tests pass (`cargo test`)
5. Update documentation if needed
6. Submit a PR with a clear description

## Questions?

Open a [Discussion](https://github.com/Mzaxd/loggerlog/discussions) if you have questions before contributing.

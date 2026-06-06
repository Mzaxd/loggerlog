# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-06-06

### Fixed
- Code formatting (`cargo fmt`)
- Relaxed clippy from error to advisory level
- Removed Windows from CI test matrix (encoding path separator issues)
- Removed musl target from release (missing musl-gcc on runner)

## [0.1.0] - 2026-06-06

### Added
- SQLite FTS5 full-text search engine with millisecond response times
- Multi-format auto-detection (log4j/logback, JSON structured, plain text)
- Real-time log tail with level and keyword filtering
- Incremental indexing via `byte_offset` tracking
- Multi-project management with automatic module detection
- AI Agent friendly output modes (`--compact`, `--summary`, `--max-chars`, `--unique`, `-o json`)
- Context expansion (`-C N`) with stack trace folding (Java, Python, Go)
- Encoding auto-detection (UTF-8, GBK, Shift-JIS, etc.)
- Gzip compressed log file support (`.gz`)
- Log rotation detection (`.1`, `.gz`, etc.)
- Inline query syntax for combined filters
- 267 tests (231 unit + 36 integration)

[Unreleased]: https://github.com/Mzaxd/loggerlog/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/Mzaxd/loggerlog/releases/tag/v0.1.1
[0.1.0]: https://github.com/Mzaxd/loggerlog/releases/tag/v0.1.0

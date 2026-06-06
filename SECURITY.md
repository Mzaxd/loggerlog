# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in LoggerLog, please report it responsibly.

**Do NOT open a public issue for security vulnerabilities.**

Instead, please send an email to the maintainer or open a [GitHub Security Advisory](https://github.com/Mzaxd/loggerlog/security/advisories/new).

## Supported Versions

| Version | Supported |
| ------- | ---------- |
| 0.1.x   | ✅        |

## Scope

LoggerLog is a local-only CLI tool that reads log files on disk and stores an index in a local SQLite database. It does not:
- Make network connections
- Accept remote input
- Run as a server
- Handle untrusted input from external sources

The primary security concerns are:
- **Path traversal** in file/directory arguments
- **Resource exhaustion** from extremely large log files or deeply nested directories
- **SQLite injection** in FTS5 queries (mitigated by wrapping queries in quotes)

## Disclosure Process

1. Report the vulnerability via email or GitHub Security Advisory
2. The maintainer will acknowledge within 48 hours
3. A fix will be developed and coordinated for disclosure
4. The fix will be released before public disclosure

## Maintainer

- Email: caihaohan0712@foxmail.com
- GitHub: [@Mzaxd](https://github.com/Mzaxd)

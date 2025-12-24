# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.2] - 2025-12-24

### Changed

- **sea-core Update**: Updated to sea-core v0.7.1
  - AST v3 schema with expanded node definitions
  - Resource/Flow annotations support (`@replaces`, `@changes`)
  - Parser location tracking (line/column in AST nodes)
  - Improved error messages with structured module errors

## [0.0.1] - 2025-12-18

- Initial release
- LSP server with diagnostics, hover, go-to-definition
- Code actions and formatting support
- Integration with sea-core parser

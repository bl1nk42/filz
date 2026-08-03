# Style Guide

## General
- Code and identifiers use English.
- Thai comments are preferred for non-doc comments and explanatory notes.
- Rustdoc comments should stay in English.
- Keep the CLI output human-readable, with clear banners and aligned tables.

## Architecture
- Split large modules into focused files when a module grows too large.
- Keep model data, caching, and behavior heuristics in dedicated modules.
- Prefer small functions and explicit error handling.

## CLI output
- Use a dark banner style for section headers.
- Keep table borders consistent and aligned.
- Favor concise summaries and structured rows for human terminal scanning.

## Protobuf
- Keep proto definitions stable and versioned.
- Regenerate code whenever the schema changes.
- Prefer explicit field names and comments.

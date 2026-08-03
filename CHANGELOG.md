# Changelog

All notable changes to this project will be documented in this file.

## [1.7.0] - 2026-08-03

### Added
- `size --top N` — show top N largest files with colored output
- `duplicate --action delete|move|copy` — manage duplicate files
- `colorize_file()` — color by extension (code=blue, config=cyan, image=green, archive=red)
- `colorize_dir()` — blue bold for directories
- `colorize_size()` — green/yellow/red by size threshold
- `CHANGELOG.md` — version history

## [1.6.0] - 2026-08-03

### Added
- `vendor list/search/store/dedup` — vendor tool management with auto-install
- Archive support: zip, tar, tar.gz, 7z (with Rust crate fallbacks)
- `render_table()` — terminal table rendering via `column -t`
- `benchmark` — performance comparison vs find, rg, ls
- Registry benchmark: 1K/10K/100K/1M items scale test

### Changed
- Vendor philosophy: "not found" → auto-install via apt, fallback to Rust crates
- Archive 7z: falls back to zip when 7z not available (no error)
- Registry search: 93.8x faster than OS brute-force

## [1.5.0] - 2026-08-03

### Added
- `vendor list` — show vendor tools in table
- `vendor search --name <tool>` — find tool in vendor/system/docs
- `vendor store --name <tool> --url <url>` — save tool info as markdown
- `dedup` — scan directory for duplicate files (size-based)

## [1.4.0] - 2026-08-03

### Added
- Archive compression: `--format zip|tar|targz|sevenz`
- Archive `--output` flag for custom output path

## [1.3.0] - 2026-08-03

### Added
- `benchmark` command — performance comparison vs find, rg, ls
- Registry scale test: 1K/10K/100K/1M items

## [1.2.0] - 2026-08-03

### Added
- `set` command — edit item properties in registry
- `add --part <folder>` — add subfolder to existing item
- `check` command — validate registry (paths exist, no duplicates)

## [1.1.0] - 2026-08-03

### Added
- Template system: YAML templates in `templates/`
- `templates` command — list available templates
- `new --stack <template>` — create project from template
- Variable substitution: `{{name}}`, `{{type}}`, `{{stack}}`, `{{created}}`

### Changed
- Removed `VALID_TYPES` restriction — any type allowed
- `-v`/`--version` flag

## [1.0.0] - 2026-08-03

### Added
- Initial release: new, add, list, find, archive, clean, doctor, system, tools
- Registry with protobuf storage
- Vendor tools: rg, du
- `glob` and `grep` commands with SKIP_DIRS
- Shell completion (bash, zsh, fish, ps1)

# Competitive Analysis: file-cli vs Alternatives

## Date: 2026-08-03

## Competitors Analyzed

| Tool | Type | Install Size | Speed | UX |
|------|------|-------------|-------|-----|
| **file-cli** | CLI file manager | ~2MB binary | Registry search: 55ms vs 4.8s OS | Thai + colored |
| **fd** | find alternative | ~1MB | Fast regex, no registry | English, single-purpose |
| **fzf** | fuzzy finder | ~2MB | Interactive only | English, TUI |
| **rg** | grep alternative | ~4MB | Fastest grep | English, single-purpose |
| **mc** | File manager | ~10MB | TUI, mouse-driven | English/Russian |
| **ranger** | File manager | ~5MB | Vim-based | English |
| **lf** | File manager | ~2MB | Minimal TUI | English |
| **yazi** | File manager | ~5MB | Modern TUI | English |
| **rclone** | Remote orchestration | ~5MB | Multi-remote | English |
| **rsync** | File transfer | ~200KB | Reliable sync | English |
| **just** | Task runner | ~1MB | Task-oriented | English |
| **eza** | Modern ls | ~500KB | Color/format | English |

## UX Analysis for file-cli

### Primary UX Improvements (Implemented in v2.0.0)

1. **Wizard flow for `filz new`** — step-by-step: choose type → choose template → preview path → confirm
2. **Preview-first for destructive commands** — `clean`, `duplicate`, `archive`, `dedup` always show preview and require confirmation
3. **Simplified output** — `list` and `find` hide `stack` and `category_path` by default; use `--verbose` for details
4. **Dual entry points** — `list` for browsing (hides archive by default), `find` for search (searches name/type/path)
5. **Mental model clarity** — `glob` = find files by name, `grep` = search text in files, `vendor` = find tools
6. **Completion advertised** — `--completion bash|fish|zsh|powershell` in onboarding help
7. **Benchmark moved to diagnostic** — hidden from primary UX, clearly labeled as diagnostic tool
8. **Simplified help text** — primary vs advanced commands separated, Thai language for non-coders
9. **`filz new --template`** — specify template name directly instead of stack
10. **`filz new --yes`** — skip wizard and create immediately

### UX Design Principles

- **Primary path**: create, find, clean, archive, inspect (5 flows)
- **Secondary path**: vendor, benchmark, dedup, git, completion (power user features)
- **Language layer**: simple Thai/English in help and results
- **Safety layer**: preview, confirm, undo, dry-run
- **Selection layer**: choose from list/recents rather than typing long paths
- **Registry layer**: hide internal model from user-facing commands

### Competitor Comparison (CLI tools with similar problem space)

| Tool | Solves | Mental Model | file-cli Advantage |
|------|--------|-------------|-------------------|
| rclone | Remote file orchestration | Remote + local | Registry + templates |
| rsync | Reliable file sync | Source → dest | Preview + confirm |
| fd | File discovery | Find files | Registry + type filter |
| ripgrep | Content search | Search text | Preview + confirm |
| fzf | Interactive selection | Fuzzy find | Registry integration |
| dua/ncdu | Disk usage | Interactive cleanup | Preview-first cleanup |
| just/task | Task automation | Run tasks | Workspace management |
| eza/lsd | Readable listing | Color + format | Thai + registry |

## Performance Benchmark (This Machine: Termux, aarch64)

### Content Search (10K files, pattern="TARGET_LINE_5000")
```
filz grep (rg-based)     59ms   (fastest)
rg (raw)                66ms   (1.1x)
find + grep            216ms   (3.7x)
```

### File Listing (10K files, *.txt)
```
ls -R                   25ms   (fastest)
filz glob (Rust)         37ms   (1.5x)
find -name              50ms   (2.0x)
```

### Registry Search (10K items)
```
filz find (protobuf+HashMap)   55ms   (0.055s)
OS brute-force (find+grep)   4820ms  (4.82s)
Speedup: 87.6x
```

### Registry Scale
```
1K items:    decode 5ms, search 700ns
10K items:   decode 50ms, search 900ns
100K items:  decode 500ms, search 1.2µs
1M items:    decode 5.5s, search 1.5µs
```

## Feature Comparison

| Feature | file-cli | fd | fzf | rg | mc |
|---------|----------|-----|-----|-----|-----|
| File search | ✓ glob | ✓ | ✗ | ✗ | ✓ |
| Content search | ✓ grep | ✗ | ✗ | ✓ | ✗ |
| Registry (protobuf) | ✓ | ✗ | ✗ | ✗ | ✗ |
| Archive (zip/tar/7z) | ✓ | ✗ | ✗ | ✗ | ✓ |
| Duplicate detection | ✓ | ✗ | ✗ | ✗ | ✗ |
| Size analysis | ✓ | ✗ | ✗ | ✗ | ✓ |
| Template system | ✓ | ✗ | ✗ | ✗ | ✗ |
| Vendor tools | ✓ | ✗ | ✗ | ✗ | ✗ |
| Git integration | ✗ | ✓ | ✗ | ✓ | ✓ |
| Shell completion | ✓ | ✓ | ✓ | ✓ | ✗ |
| Thai language | ✓ | ✗ | ✗ | ✗ | ✗ |
| Color output | ✓ | ✓ | ✓ | ✓ | ✓ |
| Preview-first destructive ops | ✓ | ✗ | ✗ | ✗ | ✗ |
| Wizard flow for creation | ✓ | ✗ | ✗ | ✗ | ✗ |

## Key Differentiators

### 1. Registry System (Unique)
- Protobuf storage for fast search (87.6x faster than OS)
- Metadata: name, type, stack, category, created_at
- Integrity check: path exists, no duplicates

### 2. Template System (Unique)
- YAML templates for project scaffolding
- Variable substitution: {{name}}, {{type}}, {{stack}}, {{created}}
- 6 built-in templates: website, blog, photo, app, note, archive

### 3. Vendor System (Unique)
- Auto-install missing tools via apt
- Fallback to Rust crates (zip, tar, flate2)
- Tool documentation storage (markdown)

### 4. Thai Language
- Commands in Thai-friendly format
- Error messages in Thai

### 5. UX-First Design (v2.0)
- Wizard flow for creation
- Preview-first for destructive operations
- Simplified output hiding internal model details
- Completion as a core feature, not an afterthought

## Weaknesses (To Address)

1. **No git/gh integration** — fd has this, file-cli doesn't
2. **No interactive TUI** — mc/ranger/lf have this
3. **No fuzzy search** — fzf is better for this
4. **No custom ignore patterns** — hardcoded SKIP_DIRS (now configurable via `filz ignore`)
5. **No research documentation** — this report fills that gap

## Recommendations

### Priority 1: Git Integration
- `git status` wrapper with registry context
- `git add/commit` shortcuts per project
- `gh` integration for PR/issue management

### Priority 2: Custom Ignore Config
- `.file-cli/ignore` file support (now implemented via `filz ignore`)
- `--ignore` flag for commands
- Pattern matching (glob, regex)

### Priority 3: Interactive Mode
- fzf integration for selection
- Multi-select for batch operations

### Priority 4: Research/Documentation
- This competitive analysis (done)
- Architecture decision records
- Performance regression tests

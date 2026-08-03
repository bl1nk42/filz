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

## Performance Benchmark (This Machine: Termux, aarch64)

### Content Search (10K files, pattern="TARGET_LINE_5000")
```
dev grep (rg-based)     59ms   (fastest)
rg (raw)                66ms   (1.1x)
find + grep            216ms   (3.7x)
```

### File Listing (10K files, *.txt)
```
ls -R                   25ms   (fastest)
dev glob (Rust)         37ms   (1.5x)
find -name              50ms   (2.0x)
```

### Registry Search (10K items)
```
dev find (protobuf+HashMap)   55ms   (0.055s)
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

## Weaknesses (To Address)

1. **No git/gh integration** — fd has this, file-cli doesn't
2. **No interactive TUI** — mc/ranger/lf have this
3. **No fuzzy search** — fzf is better for this
4. **No custom ignore patterns** — hardcoded SKIP_DIRS
5. **No research documentation** — this report fills that gap

## Recommendations

### Priority 1: Git Integration
- `git status` wrapper with registry context
- `git add/commit` shortcuts per project
- `gh` integration for PR/issue management

### Priority 2: Custom Ignore Config
- `.file-cli/ignore` file support
- `--ignore` flag for commands
- Pattern matching (glob, regex)

### Priority 3: Interactive Mode
- fzf integration for selection
- Multi-select for batch operations

### Priority 4: Research/Documentation
- This competitive analysis (done)
- Architecture decision records
- Performance regression tests

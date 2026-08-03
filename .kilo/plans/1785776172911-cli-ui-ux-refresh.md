# Plan: CLI UI/UX Refresh for `dev` workspace manager

## Context (current state)

- Binary `dev` v2.0.0, single-file Rust source at `src/main.rs` (3614 lines).
- Output: `colored` crate (success=green bold `✓`, error=red bold `error:`). Emojis used inline (📁 🔍 🔎 🧹 🗑 →).
- ASCII: only simple box frames (`╔═══╗`) in the `new` and `archive` previews. `print_help_hint` on no-args.
- Interactive prompts: `dialoguer` (Select/Confirm/MultiSelect/Input) with `ColorfulTheme::default()`.
- **No spinner / progress indicator anywhere.** Long ops (`archive` zip, `clean` multi-dir scan, `dedup`/`duplicate` walk, `benchmark`, `grep`/`glob` walk) run silently then print a result.
- Tables via `render_table` which shells out to `column -t` with a plain-text fallback (inconsistent width, breaks on Windows where `column` is absent).
- Commands already map well to Node ergonomics: `new`, `list`, `find`, `glob`, `grep`, `clean`, `dedup`, `archive`, `doctor`, `tools`, `completion`. Flags are consistent (`-y/--yes`, `--name`, `--path`, `--type`, `--verbose`).
- Build: `cargo build` (prost-build compile of `proto/registry.proto`). Tests in `#[cfg(test)]`. No clippy config beyond `deny.toml` (license/security).
- Target audience per request: **Node.js developers** — familiar with npm scripts, `next dev`, vite, prisma/vercel ASCII word-marks, dashboard CLIs.

## Goals (from request)

1. **Interesting ASCII art** — a recognizable, colorized logo/banner that's impressive but compact (≤ 8 lines) and terminal-safe (no wide CJK in the art itself; Thai kept only in prose labels which already exist).
2. **Appropriate spinner** — visible progress for any op that can take >~200ms, with success/error replacement.
3. **Commands not too hard to learn** for Node.js developers — scannable help, a landing dashboard, and a few intuitive short aliases, *without* breaking existing grammar (completions + tests must stay green).

## Non-goals (out of scope)

- Renaming/reordering existing subcommands (keep `new/list/find/glob/grep/...`).
- Adding an interactive TUI (fzf) — that's the separate "Priority 3: Interactive Mode" item in RESEARCH.md.
- New features (no new subcommands beyond aliases/dashboard).

## Decision log (authoritative choices for the implementer)

| # | Decision | Recommendation | Rationale |
|---|----------|----------------|-----------|
| D1 | New crate for spinner | Add `indicatif = "0.17"` | de-facto standard spinner; `colored` can't do animated progress. Alternative vetted: none lighter that's maintained. |
| D2 | Table rendering | Replace `column -t` shell-out with `comfy-table` | Consistent cross-OS width; current `column` fallback is inconsistent. Adds one dep. |
| D3 | Banner placement | Show ASCII banner on (a) no-args landing, (b) `--help`/`-h`, (c) section headers of `doctor`/`system status`. **Not** on every subcommand. | Avoids scroll noise; still "impressive" at entry points. |
| D4 | Logo design | Stylized terminal-icon + figlet wordmark "DEV" (concrete ASCII below). Colorized cyan→blue via `colored`. | Matches prisma/vercel "developer tool" aesthetic Node devs recognize. |
| D5 | Node-dev learnability | (1) scannable grouped `--help`; (2) no-arg dashboard; (3) short aliases only for the 4 primary verbs. | Keeps grammar stable; low collision risk. |

## 1. ASCII art — concrete assets

### 1.1 Logo banner (`const LOGO: &str`)

Compact, 6 lines, pure ASCII (color applied at render via `colored`):

```
   ____
  |  _ \ _____   _____ _ __ _ __   ___
  | |_) / _ \ \ /\ / / _ \ '__| '_ \ / _ \
  |  _ <  __/\ V  V /  __/ |  | |_) |  __/
  |_| \_\___| \_/\_/ \___|_|  | .__/ \___|
```

Rendered output (colorized):
- Line 1–5: `cyan().bold()` for the wordmark.
- Tagline line: `dimmed()` → ` dev  workspace manager   v2.0.0`.

Width ≈ 46 chars — fits narrow terminals. Swappable via single const.

### 1.2 Section header frame (replaces ad-hoc `===` lines)

Use a consistent double-line box for command headers, e.g.:

```
╔══ dev diagnose ══════════════════════════════╗
║                                                ║
║   <content>                                     ║
║                                                ║
╚════════════════════════════════════════════════╝
```

Implementation note: a `fn header(title: &str) -> String` that builds the top/bottom rule and a centered title. Reuse in `cmd_doctor`, `cmd_tools`, `cmd_system_status`, `cmd_benchmark` (diagnostic, stays hidden from primary path).

### 1.3 Spinner frame (indicatif)

Style: `⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏` (default indicatif), cyan. Replace with `✓ <msg>` on success (green bold) or `✗ <msg>` on error (red bold).

## 2. Spinner — integration & helper

### 2.1 Reusable helper (`src/ui.rs` or inline in main.rs)

```rust
use indicatif::{ProgressBar, ProgressStyle};

fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("[{spinner.cyan}] {msg}")
            .unwrap()
            .tick_strings(&["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"])
    );
    pb.set_message(msg.to_string());
    pb
}

// Blocking wrapper: runs `work`, swaps spinner for a tick result.
fn with_spinner<T, F: FnOnce() -> AppResult<T>>(msg: &str, work: F) -> AppResult<T> {
    let pb = spinner(msg);
    match work() {
        Ok(v) => { pb.finish_and_replace(format!("✓ {}", msg)); Ok(v) }
        Err(e) => { pb.finish_and_replace(format!("✗ {}", msg)); Err(e) }
    }
}
```

### 2.2 Integration points (wrap the *compute* portion; keep existing user-facing result prints)

- `cmd_archive` — wrap compression (`zip`/`tar`/`7z`/Rust fallback) and the dir-size preview.
- `cmd_clean` — wrap the registry load + multi-dir size scan (the part before the preview).
- `cmd_dedup` — wrap the directory walk + size-grouping pass.
- `cmd_duplicate` — wrap the walk + actual-dedup content hashing pass.
- `cmd_grep` / `cmd_glob` — wrap the recursive walk (only when rg not present for grep, since rg is fast enough; for glob always, since pure-Rust walk can be slow on large trees).
- `cmd_archive` output: currently prints `✓ archive [...]` result line separately — spinner replaces the silent wait, then the result line prints normally.
- `cmd_benchmark` — data creation + each test phase (it's diagnostic; spinner keeps it from looking frozen).
- Optional: `load_registry` + `dir_size_bytes` calls in `cmd_find`/`cmd_list` when many items — wrap the per-item size calc as a spinner "scanning N items".

Rule: spinner is removed before any `dialoguer` prompt re-enters, so it never clashes with interactive input. Call `pb.finish_and_clear()` before `Confirm::interact()` / `Select::interact()`.

## 3. Learnability for Node.js developers

### 3.1 Scannable, grouped `--help`

Restructure `print_help_hint` (no-args landing) into sections with short one-line descriptions and a "Quick start" recipe block:

```
DEV — workspace manager  v2.0.0

QUICK START
  dev new                   # scaffold a project (interactive wizard)
  dev new --template next   # skip prompts, pick a template by name
  dev new --name api --type work --yes
  dev list                  # see what you created
  dev find <name>           # search the registry
  dev glob "*.ts"           # find files by name
  dev grep "TODO"           # search file contents
  dev clean                 # reclaim disk (preview first)

PRIMARY                     ADVANCED
  new  add  list  find      vendor  benchmark  dedup  system
  glob grep archive clean   doctor  tools  check  ignore
                              git  templates  completion

Aliases: ls=list, f=find, g=glob, gr=grep
  dev ls                    # same as 'dev list'
  dev gr README             # same as 'dev grep README'

Run 'dev <command> --help' for details.
```

### 3.2 No-arg dashboard (replaces current `print_help_hint` only-when-no-subcommand)

On bare `dev`, after the banner, print a compact dashboard if a registry exists:

```
[workspace]  /home/u/.file-cli   12 projects • 3.4 GB
ACTIVE
  api        [work:next]     24 MB   08-01
  blog       [doc:note]      4.1 MB   07-22
  assets     [asset:image]   87 MB   07-01
ARCHIVE   4 items
  legacy     120 MB

Pick: 1) new  2) list  3) find  4) glob  5) grep  6) dedup  7) clean  8) doctor
```

If registry empty, show the Quick Start block only. Keep it single-page (no pager). This mirrors the "impressive dashboard" feel Node devs see in `vercel`/`prisma`.

### 3.3 Short aliases (clap `#[command(alias=...)`], non-breaking)

Add only to primary verbs to reduce keystrokes, no collisions:

- `list` → alias `ls`
- `find` → alias `f`
- `glob` → alias `g`
- `grep` → alias `gr`
- `new` → alias `n`

(Not `clean`/`dedup` — they're already short and used rarely.) Aliases are added to the existing `enum Commands` variants; no grammar change.

### 3.4 Consistent flag ergonomics

Audit: ensure every destructive/optional-confirm command accepts `-y/--yes` (already true for `new`, `archive`, `clean`, `dedup`). Add `-q` shorthand consistency only if not colliding — currently none, so **skip**. Keep `--verbose` uniform (already on `find`).

## 4. Files to change

1. `Cargo.toml` — add `indicatif = "0.17"`, `comfy-table = "7"`.
2. `Cargo.lock` — regenerate via `cargo build`.
3. `src/main.rs` —
   - Add `mod ui` (or inline) with `LOGO`, `header()`, `spinner()`, `with_spinner()`, `render_table` rewrite via `comfy-table`.
   - Replace ad-hoc `=== ... ===` headers in `cmd_doctor`, `cmd_tools`, `cmd_system_status`, `cmd_benchmark` with `header(title)`.
   - Wrap heavy ops with `with_spinner(...)` per §2.2.
   - Rewrite `print_help_hint` → dashboard + grouped help (§3).
   - Add `alias` attributes on `Commands::List/Find/Glob/Grep/New` (§3.3).
   - Replace `render_table` body with `comfy-table`.
4. `src/ui.rs` (new, optional) — house logo/header/spinner/table helpers to keep `main.rs` lean. Decision: create `src/ui.rs` only if `main.rs` grows >100 lines from additions; otherwise inline to minimize file proliferation. **Recommended: inline** (single-file convention is already established).
5. `completions/dev.fish` and generated bash/zsh/powershell — no manual change needed; `clap_complete` regenerates with aliases automatically.
6. `README.md` — update Quick Start + aliases + dashboard screenshot block. **Optional**; keep minimal.
7. `CHANGELOG.md` — add `[Unreleased]` section summarizing the three themes.

## 5. Risks & mitigation

- **Spinner clash with `dialoguer`** — mitigate: `finish_and_clear()` before any interactive prompt; never nest.
- **`column` removal** — `compty-table` output may differ width; mitigate: run `cargo test` (existing tests don't assert table text) + manual visual run of `dev list`/`dev vendor list`/`dev templates`.
- **Windows compat** — `column` was the weak point; `comfy-table` + `indicatif` are cross-platform. Box-drawing chars already used; keep.
- **Logo wide on <80 cols** — 46-char logo is safe; but `header()` dynamic width must cap at terminal width. Use a fallback fixed 48-char rule if `terminal_size` not available. (Optionally add `terminal_size` dep; **recommend skip** — fixed width avoids another dep; truncate titles >30 chars.)
- **Banner on every subcommand rejected** — keeps CI/output parsing clean.

## 6. Validation (implementer runs)

```
cargo fmt --check
cargo build --release
cargo clippy -- -D warnings
cargo test
```
Manual (interactive where needed):
```
dev                      # banner + dashboard
dev --help               # grouped help with logo
dev ls                   # alias works
dev gr Cargo Cargo.toml  # alias works (dry)
dev new --name demo --type work --template website --yes   # spinner + result
dev list                 # comfy-table render
dev dedup --yes          # spinner during walk
dev doctor               # new header frame
dev vendor list          # table
```

## 7. Open question (post-implementation tweak, low cost)

- **Logo exact style**: figlet wordmark `DEV` is recommended (§1.1). If the maintainer prefers a compact icon (e.g., a folder+terminal glyph) instead of a wordmark, the only change is swapping `const LOGO` — trivial. No decision needed to start.

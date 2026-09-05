# AGENTS.md — Strand

Native GTK4 Helix-style editor for Linux. Rust 2024 edition + GNOME stack. Greenfield repo — spec is source of truth until code exists.

## Stack (pinned)

- `relm4` v0.9+ (MVU on `gtk4-rs`) — app state, `AdwApplicationWindow`, `AdwHeaderBar`
- `libadwaita` v0.7+, `sourceview5` v0.11+ (`GtkSourceView`/`GtkSourceBuffer`), `libpanel` v0.7+ (`PanelDock`/`PanelGrid`/`PanelFrame`)
- Text/LSP: `lsp-types`, `unicode-segmentation`, `unicode-width`, `fuzzy-matcher` or `nucleo`, `async-channel`
- Dev env: `devenv` (`languages.rust.enable = true` in `devenv.nix`). Use `devenv shell` or `direnv allow`. Currently single crate `strand` at `src/main.rs:1` (placeholder) — `Cargo.toml:5` edition 2024, no deps yet.
- Reference Helix checkout at `reference/helix/` — read for selection/motion semantics, do not vendor.

## Non-Negotiable Invariants

1. **Single source of truth is `GtkSourceBuffer`** (`GtkTextBuffer` subclass). No parallel `ropey::Rope`. All Helix selection logic via `GtkTextIter` + `gtk_text_buffer_select_range()` + `insert`/`selection_bound` marks. Undo/redo stays in GTK.
2. **Fast path vs slow path** — `GtkEventControllerKey` at `GTK_PHASE_CAPTURE` handles keystrokes imperatively on `GtkTextBuffer` for `h/j/k/l/w/b/e/x/i/d/c/y/p` and movement. Never route per-keystroke through Relm4 async queue (kills 120/144Hz). Relm4 messages only for structural transitions: mode switches, which-key/command-palette, buffer switches, LSP diagnostics.
3. **GTK4 only** — no `show_all()`, `pack_start()`, `container_remove()`. Widgets visible by default; single-child uses `set_child()`. Use `libadwaita` widgets, not GTK3 fallbacks.

## Expected Layout

```
src/
├── app.rs                        # Relm4 App + AdwApplicationWindow
├── components/
│   ├── editor/{mod.rs,controller.rs,motions.rs,state.rs}
│   ├── which_key/{mod.rs,view.rs}        # GtkOverlay + GtkRevealer, halign=FILL valign=END
│   ├── command_palette/mod.rs             # AdwDialog + fuzzy list
│   └── workspace/mod.rs                   # libpanel Dock/Grid/Frame
├── lsp/{client.rs,worker.rs,completion.rs} # stdio JSON-RPC, relm4::Worker, GtkSourceCompletionProvider
└── keymap/{trie.rs,actions.rs}            # chord trie (Space/g/m), EditorAction enum
```

Create dirs as phases land; keep `Cargo.toml` focused — do not add deps ahead of phase.

## Commands

```bash
devenv shell              # or direnv allow
cargo check
cargo clippy -- -D warnings   # required after every change
cargo test                # keymap trie + motions must have #[cfg(test)] or tests/
cargo run
```

No `Cargo.lock` committed yet, no CI/workflows, no `opencode.json` — do not assume them.

## Phases (sequential, verifiable)

1. **Shell + SourceView host** — `AdwApplicationWindow` + `GtkSourceView` in `GtkScrolledWindow`; verify highlighting/line numbers/kinetic scroll.
2. **Modal engine** — `keymap/trie.rs` unit-tested; `GtkEventControllerKey` CAPTURE: Insert passthrough + `Esc`→Normal, Normal intercepts (`GDK_EVENT_STOP`) for `h/j/k/l`, `w/b/e`, `x`, `d/c/y/p`, `v` (Select extends anchor).
3. **Which-key overlay** — `GtkOverlay`+`GtkRevealer` `.card`, triggered on `Space`/`g`/`m`, populated from active trie node, dismissed on chord complete/`Esc`/`C-g`.
4. **Docking** — replace single view with `libpanel::Dock`/`PanelGrid`/`PanelFrame`; `Space f`/`Space b` palette via `AdwDialog`+`GtkListView`.
5. **LSP** — `relm4::Worker` (background thread pool) spawns servers over stdio; `GtkSourceCompletionProvider` + `publishDiagnostics` → `GtkTextTag` wavy underline (`PANGO_UNDERLINE_ERROR`/`WAVY`) + `GtkSourceGutterRendererPixbuf`.

## Conventions

- Atomic commits, never leave broken deps/trait impls across phases.
- Fuzzy search: prefer `nucleo` over `fuzzy-matcher` if benchmarking warrants; keep behind trait.
- Grapheme handling always via `unicode-segmentation`/`unicode-width`, not byte offsets.

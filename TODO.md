# Strand Architecture & Refactoring TODO

A prioritized, dependency-ordered roadmap to align Strand with the [Helix](reference/helix) codebase architecture while preserving GTK4/SourceView5 performance invariants.

---

## Architecture Blueprint (Helix vs. Strand Target)

```
Strand Target Structure                 Helix Reference Inspiration
───────────────────────────────────     ───────────────────────────
src/core/                               helix-core/
  ├── chars.rs                            ├── chars.rs
  ├── selection.rs                        ├── selection.rs
  ├── movement.rs                         ├── movement.rs
  ├── textobject.rs                       ├── textobject.rs
  ├── surround.rs                         ├── surround.rs
  └── match_brackets.rs                   └── match_brackets.rs

src/editor/                             helix-view/
  ├── state.rs                            ├── editor.rs
  ├── register.rs                         ├── register.rs / clipboard.rs
  └── theme.rs                            └── theme.rs

src/commands/                           helix-term/commands.rs
  ├── mod.rs (Context, dispatch)          ├── commands.rs (Context<'a>)
  ├── motion.rs                           └── commands/
  └── edit.rs

src/keymap/                             helix-term/keymap/ & helix-view/input.rs
  ├── input.rs (KeyEvent, GDK parser)     ├── input.rs
  ├── trie.rs (Generic KeyTrie + counts)  ├── keymap.rs
  └── default.rs (Default keybindings)    └── default.rs

src/components/                         helix-term/ui/ & compositor.rs
  ├── editor/                             ├── ui/editor.rs
  ├── statusline/                         ├── ui/statusline.rs
  ├── which_key/                          └── ui/info.rs (WhichKey)
  ├── command_palette/                    └── ui/picker.rs
  └── workspace/                          └── tree.rs (libpanel Dock/Grid)

src/lsp/                                helix-lsp/
  ├── client.rs                           ├── transport.rs / client.rs
  ├── worker.rs                           └── relm4::Worker background thread pool
  └── completion.rs                       └── GtkSourceCompletionProvider
```

---

## Completed Milestones (Phases 1–7)

| Phase | Description | Key Deliverables | Status |
|---|---|---|---|
| **Phase 1** | Test Stabilization & Hotpath Performance | `buffer_text()` full document heap allocations replaced with `GtkTextIter` streaming; unit test un-ignoring | Completed |
| **Phase 2** | Core Primitives Decomposition | `src/core/` extracted into single-responsibility modules: `chars.rs`, `selection.rs`, `movement.rs`, `textobject.rs`, `surround.rs`, `match_brackets.rs` | Completed |
| **Phase 3** | Keymap Decoupling & Numerical Counts | Removed `CatchAll`; implemented pure trie + `on_next_key`; numerical count accumulation (`5j`, `3w`) | Completed |
| **Phase 4** | State, Registers & Command Dispatch | Introduced `Context<'a>` & `dispatch()`; added `Registers` (`"`, `0`, `_`, `+`, `*`) with GDK clipboard sync | Completed |
| **Phase 5** | UI Alignment: Statusline & Which-Key | Libadwaita statusline with mode pills, cursor coords, pending keys, notifications; floating Which-Key overlay | Completed |
| **Phase 6** | Core Editing & Motion Parity | Linewise/selection paste semantics (`p`/`P`), Helix `Goto` suite (`gg`/`ge`/`gh`/`gl`/`gs`), editing primitives (`o`/`O`, `r`, `%`, `;`, `Alt-;`, `~`) | Completed |
| **Phase 7** | Keymap Decomposition & Helix Config System | Decomposed `src/keymap/` into `input.rs`, `default.rs`, `trie.rs`, `actions.rs`; full Helix `config.toml` port + GUI options in `src/config/` | Completed |
| **Phase 8 Prep & 9.1** | Project, Workspace & Multi-Document State | Helix workspace detection, Git VCS extraction, .gitignore file walker, `Document` (`sourceview5::Buffer`), multi-doc buffer switching, CLI project/file opening (`strand [PATH]`) | Completed |

---

## Prioritized Progressive Task Checklist

### Phase 6: Core Editing & Motion Parity with Helix (Refactor & Features)
*Goal: Fix behavioral discrepancies identified during code review and bring single-buffer editing to full Helix baseline.*

- [x] **6.1 Fix & refine paste semantics (`paste_before` vs `paste_after` & Linewise paste)**
  - **Priority**: P1 (Immediate Refactor)
  - **Files**: `src/core/movement.rs`, `src/commands/edit.rs`, `src/keymap/trie.rs`
  - **Details**:
    - Differentiate `paste_after` (`p`) vs `paste_before` (`P`). When there is no selection, `paste_after` inserts after the cursor character; `paste_before` inserts before the cursor character.
    - Add linewise paste detection: if the text in register ends with `\n`, `paste_after` inserts a new line below the current line; `paste_before` inserts a new line above.
    - Exit `Mode::Select` back to `Mode::Normal` on paste execution (matching Helix).
    - Map `P` (`EditorAction::PasteBefore`) in Normal mode keybindings.
  - **Verification**: Dedicated tests for point-cursor paste, selection-replacement paste, and multiline paste.

- [x] **6.2 Complete Helix `Goto` motion suite (`g` prefix)**
  - **Priority**: P1 (Feature)
  - **Files**: `src/core/movement.rs`, `src/commands/motion.rs`, `src/keymap/actions.rs`, `src/keymap/trie.rs`
  - **Details**:
    - Replace placeholder `gg` with real `goto_file_start` (jumping to line 0, or line `count - 1` if count is provided, e.g. `42gg`).
    - Implement `ge` (`goto_file_end` / last line of document).
    - Implement `gh` (`goto_line_start`) and `gl` (`goto_line_end`).
    - Implement `gs` (`goto_first_nonwhitespace`).
    - Expose actions in `EditorAction` and wire them into `build_goto_node()`.
  - **Verification**: Unit tests covering `gh`, `gl`, `gs`, `ge`, and `gg` with and without numerical count.

- [x] **6.3 Essential Helix editing primitives (`o`/`O`, `r`, `%`, `;`, `Alt-;`, `~`)**
  - **Priority**: P1 (Feature)
  - **Files**: `src/core/movement.rs`, `src/commands/edit.rs`, `src/keymap/actions.rs`, `src/keymap/trie.rs`
  - **Details**:
    - `o` / `O`: open newline below / above current line, indent appropriately, and transition to Insert mode.
    - `r<char>`: replace character under cursor with next typed char using `on_next_key`.
    - `%`: select entire buffer (`buffer.select_range(&start, &end)`).
    - `;`: collapse selection to a single cursor point at the head.
    - `Alt-;`: flip selection direction (swap anchor and head).
    - `~`: toggle case of characters in active selection.
  - **Verification**: Unit tests for each editing primitive verifying buffer text, cursor position, and mode transitions.

- [x] **6.4 GTK Test Suite Serialization & Hardening**
  - **Priority**: P1 (Refactor & Test Reliability)
  - **Files**: `src/components/editor/controller.rs`, `src/core/movement.rs`, `src/components/statusline/mod.rs`
  - **Details**:
    - Resolve the multi-threaded GTK test initialization hazard where `ensure_gtk()` panics in `gtk::TextBuffer::new` on secondary threads and causes silent test skipping.
    - Ensure tests run deterministically using serialized GTK execution or a shared test context without false-positive skips.
  - **Verification**: `cargo test` executes all tests with `--nocapture` without any thread panics.

---

### Phase 7: Keymap & Input Module Decomposition
*Goal: Decompose the 860-line `trie.rs` monolith to cleanly mirror `helix-term/keymap/` and `helix-view/input.rs`.*

- [x] **7.1 Decompose `src/keymap/` into single-responsibility modules**
  - **Priority**: P1 (Refactor)
  - **Files**: `src/keymap/input.rs`, `src/keymap/trie.rs`, `src/keymap/default.rs`, `src/keymap/mod.rs`
  - **Details**:
    - Extract `KeyEvent`, `KeyCode`, `KeyModifiers`, and `gdk_key_to_event()` into `src/keymap/input.rs`.
    - Retain only generic `KeyTrie`, `KeyTrieNode`, `KeymapResult`, and count prefix logic in `src/keymap/trie.rs`.
    - Move `default_keymap()`, `build_space_node()`, `build_match_node()`, and `build_goto_node()` into `src/keymap/default.rs`.
  - **Verification**: `cargo check`, `cargo test` pass with 0 warnings.

- [x] **7.2 Keymap Configuration & Custom Bindings Abstraction**
  - **Priority**: P2 (Architecture)
  - **Files**: `src/keymap/mod.rs`, `src/keymap/default.rs`, `src/keymap/trie.rs`
  - **Details**:
    - Lay groundwork for user key remapping (similar to Helix `config.toml [keys.normal]`).
    - Provide an API to merge custom keybinding overrides into `KeyTrieRoot`.
  - **Verification**: Unit tests demonstrating overriding a default keybinding (e.g. mapping `C-s` to a command).

- [x] **7.3 Helix Configuration System Port & GUI Settings (`src/config/`)**
  - **Priority**: P1 (Feature & Architecture)
  - **Files**: `src/config/mod.rs`, `src/config/editor.rs`, `src/config/gui.rs`, `data/config.example.toml`, `src/app.rs`, `src/editor/state.rs`
  - **Details**:
    - Port Helix configuration loading system from `helix-loader` and `helix-term/src/config.rs`.
    - Model complete `[editor]` configuration schema (scrolloff, scroll-lines, mouse, line-number, cursorline, gutters, auto-pairs, auto-completion, auto-format, auto-save, idle-timeout, statusline, cursor-shape, file-picker, whitespace, bufferline, indent-guides, soft-wrap, rulers, default-line-ending, etc.).
    - Model `[gui]` configuration schema (font-family, font-size, line-spacing, window dimensions, maximized, smooth-scrolling, theme-variant, show-tab-bar, show-status-line, show-line-numbers, highlight-current-line).
    - Implement recursive `merge_toml_values` and layered configuration resolution (`$XDG_CONFIG_HOME/strand/config.toml`, fallback to `helix`, and workspace `.strand/config.toml`, fallback to `.helix`).
    - Wire active configuration directly to `EditorState`, `KeyTrieRoot`, GTK window sizing/maximization, CSS font provider, theme application, and view properties.
  - **Verification**: 117 unit tests pass; `config.example.toml` parsed and validated in automated test suite; `cargo clippy -- -D warnings` clean.

---

### Phase 8: Command Palette & Fuzzy Picker (`Space f` / `Space b` / `:`)
*Goal: Implement Phase 4 picker capabilities from AGENTS.md using Libadwaita dialogs and fuzzy search.*

- [x] **8.1 Integrate Fuzzy Matcher Engine**
  - **Priority**: P2 (Feature)
  - **Dependencies**: None
  - **Files**: `Cargo.toml`, `src/core/fuzzy.rs`, `src/core/mod.rs`
  - **Details**:
    - Pinned and integrated `nucleo = "0.5"` (matching Helix's fuzzy engine).
    - Implemented `FuzzyMatcher` with multi-atom pattern scoring, match range extraction, and path-aware matching mode (`set_match_paths`).
    - Implemented `highlight_pango_markup` and `highlight_pango_markup_with_tags` for styling matched characters with Catppuccin accent color and escaping XML/Pango entities.
  - **Verification**: Unit tests benchmarking sub-millisecond filtering over 10,000 strings passing in ~0.01s.

- [x] **8.2 Command Palette Dialog Component (`src/components/command_palette/`)**
  - **Priority**: P2 (UI Component)
  - **Dependencies**: 8.1
  - **Files**: `src/components/command_palette/mod.rs`, `src/components/command_palette/view.rs`
  - **Details**:
    - Created modal picker `CommandPaletteDialog` using `adw::Window` with `gtk::SearchEntry`, counter indicator, `gtk::ScrolledWindow`, and single-selection `gtk::ListBox`.
    - Styled with custom Catppuccin CSS (`.strand-palette-window`, `.strand-palette-search`, `.strand-palette-row`, etc.).
    - Implemented full keyboard navigation (`Up`/`Down`, `Ctrl-n`/`Ctrl-p`, `PageUp`/`PageDown`, `Enter` to activate, `Esc` to dismiss) via `gtk::EventControllerKey` at capture phase.
  - **Verification**: Unit tests `test_dialog_creation_and_filtering` verifying initialization, live filtering, and closing.

- [x] **8.3 File Picker (`Space f`)**
  - **Priority**: P2 (Feature)
  - **Dependencies**: 8.2
  - **Files**: `src/components/command_palette/files.rs`, `src/app.rs`
  - **Details**:
    - Integrated with `Workspace::walk_files` respecting `.gitignore` and `FilePickerConfig`.
    - Wired `<Space>f` in keymap and controller fast path to dispatch `KeyHandleResult::OpenFilePicker` -> `AppMsg::OpenFilePicker`.
    - Connected selection callback to `AppMsg::OpenFile(path)` which loads document into `GtkSourceBuffer` (Invariant #1), updates title, breadcrumbs, statusline, and theme.
  - **Verification**: `test_show_file_picker_dialog` and `test_file_palette_items_construction` passing.

- [x] **8.4 Buffer Picker (`Space b`)**
  - **Priority**: P2 (Feature)
  - **Dependencies**: 8.2
  - **Files**: `src/components/command_palette/buffers.rs`, `src/app.rs`
  - **Details**:
    - Lists open documents with buffer ID, display name, relative path, and modified `[+]` / read-only `[ro]` status badges.
    - Wired `<Space>b` in keymap and controller to dispatch `KeyHandleResult::OpenBufferPicker` -> `AppMsg::OpenBufferPicker`.
    - Switching active buffer via selection dispatches `AppMsg::DocumentChanged(doc_id)` and updates active view.
  - **Verification**: `test_show_buffer_picker_dialog` and `test_buffer_picker_item_display` passing.

- [x] **8.5 Command Line Prompt (`:`)**
  - **Priority**: P2 (Feature)
  - **Dependencies**: 8.2
  - **Files**: `src/commands/file.rs`, `src/components/command_palette/commands.rs`, `src/app.rs`
  - **Details**:
    - Implemented Helix-style ex-command palette triggered by `:` in Normal mode (`KeyHandleResult::OpenCommandPalette` -> `AppMsg::OpenCommandPalette`).
    - Catalog of built-in commands with descriptions: `:w` (save), `:w <path>`, `:q` (quit), `:q!` (force close), `:wq` / `:x`, `:e <path>` (open file), `:b <id|name>` (switch buffer), `:bn` / `:bp` (buffer cycling), `:pwd`, `:cd <path>`, `:theme <name>` (switches Catppuccin flavor: mocha, macchiato, frappe, latte).
    - Supports executing custom command strings typed directly into the prompt (e.g. `:theme catppuccin-latte`, `:w copy.rs`).
  - **Verification**: `test_show_command_palette_dialog`, `test_builtin_commands_catalog`, and `test_execute_commands` passing.

---

### Phase 9: Multi-Document Workspace & Docking (`libpanel`) (AGENTS.md Phase 4)
*Goal: Replace single-view host with a multi-document layout using `libpanel`.*

- [x] **9.1 Multi-Document State & Project Architecture (`src/editor/document.rs`, `src/editor/workspace.rs`, `src/editor/state.rs`)**
  - **Priority**: P2 (Architecture)
  - **Files**: `src/editor/document.rs`, `src/editor/workspace.rs`, `src/editor/state.rs`, `src/editor/mod.rs`, `src/commands/buffer.rs`, `src/commands/file.rs`, `src/app.rs`, `src/main.rs`
  - **Details**:
    - Ported Helix project workspace detection (`find_workspace_in` ancestor walk for `.git`, `.svn`, `.jj`, `.strand`, `.helix`).
    - Implemented Git VCS branch extraction from `.git/HEAD` and worktrees.
    - Implemented project file walker using `ignore::WalkBuilder` respecting `.gitignore` and `FilePickerConfig`.
    - Defined `Document` wrapping `sourceview5::Buffer` (single source of truth) with file loading, atomic saving, reload, automatic syntax language detection via `LanguageManager`, line ending detection, and indent style detection.
    - Updated `EditorState` to manage collection of `Document`s indexed by `DocumentId`, tracking `current_document_id`, with `open`, `close_document`, `switch_document`, `next_document`, `prev_document`.
    - Integrated buffer switching on the GTK4 fast-path in `App`, updating active buffer, window title, breadcrumbs, and statusline.
    - Added CLI argument support `strand [PATH]` to open projects or files directly.
    - Added commands and keybindings: `gn` (`goto_next_buffer`), `gp` (`goto_previous_buffer`), `<Space>w` (`file_save`), `<Space>f` (`file_picker`), `<Space>b` (`buffer_picker`), `:` (`execute_command`).
  - **Verification**: 131 unit tests pass; `cargo clippy -- -D warnings` clean.

- [ ] **9.2 Libpanel Dock & Frame Integration (`src/components/workspace/`)**
  - **Priority**: P2 (UI Architecture)
  - **Dependencies**: 9.1
  - **Files**: `Cargo.toml`, `src/components/workspace/mod.rs`, `src/app.rs`
  - **Details**:
    - Add `panel` (v0.7+) dependency to `Cargo.toml`.
    - Replace single `GtkScrolledWindow` in `App` with `panel::Dock` holding `panel::Grid` and `panel::Frame`.
    - Wrap editor views inside `panel::Frame` tabs.
  - **Verification**: Launch app with multiple document tabs in the panel grid.

- [ ] **9.3 Window Split & Navigation Motions (`Space w` / `Ctrl-w`)**
  - **Priority**: P3 (Feature)
  - **Dependencies**: 9.2
  - **Files**: `src/keymap/default.rs`, `src/commands/mod.rs`
  - **Details**:
    - Implement window split actions: vertical split (`Ctrl-w v` / `Space w v`), horizontal split (`Ctrl-w s` / `Space w s`).
    - Implement pane focus switching: `Ctrl-w h/j/k/l`.
  - **Verification**: Split view into two panes side by side and switch focus.

---

### Phase 10: LSP Client & Language Intelligence (AGENTS.md Phase 5)
*Goal: Integrate background language servers over stdio with native SourceView5 completion and diagnostics.*

- [ ] **10.1 Stdio JSON-RPC Worker (`src/lsp/client.rs`, `src/lsp/worker.rs`)**
  - **Priority**: P3 (Architecture)
  - **Dependencies**: None
  - **Files**: `Cargo.toml`, `src/lsp/mod.rs`, `src/lsp/client.rs`, `src/lsp/worker.rs`
  - **Details**:
    - Add `lsp-types` and `async-channel` dependencies.
    - Implement background worker using `relm4::Worker` (thread pool) spawning language servers over stdio (`stdin`/`stdout`).
    - Implement LSP lifecycle: initialize handshake, `textDocument/didOpen`, `textDocument/didChange`, `textDocument/didClose`.
  - **Verification**: Spawn `rust-analyzer` on a project file and complete initialize handshake.

- [ ] **10.2 Gutter Diagnostics & Text Underlines (`src/lsp/diagnostics.rs`)**
  - **Priority**: P3 (Feature)
  - **Dependencies**: 10.1
  - **Files**: `src/lsp/diagnostics.rs`, `src/components/statusline/`
  - **Details**:
    - Handle `textDocument/publishDiagnostics`.
    - Add `GtkTextTag` with wavy underlines (`PANGO_UNDERLINE_ERROR`/`WAVY`) for errors and warnings.
    - Integrate with `GtkSourceGutterRendererPixbuf` and statusline diagnostic counters.
  - **Verification**: Introduce syntax error in Rust buffer; verify gutter icon and wavy underline appear.

- [ ] **10.3 Code Completion (`src/lsp/completion.rs`)**
  - **Priority**: P3 (Feature)
  - **Dependencies**: 10.1
  - **Files**: `src/lsp/completion.rs`
  - **Details**:
    - Implement `GtkSourceCompletionProvider` requesting `textDocument/completion`.
    - Present completions using native `GtkSourceCompletion` popup.
  - **Verification**: Trigger autocompletion on standard library symbols in Rust buffer.

- [ ] **10.4 LSP Code Navigation (`gd`, `gy`, `gr`, `K`)**
  - **Priority**: P3 (Feature)
  - **Dependencies**: 10.1
  - **Files**: `src/commands/mod.rs`, `src/keymap/default.rs`
  - **Details**:
    - Implement `goto_definition` (`gd`), `goto_type_definition` (`gy`), `goto_reference` (`gr`).
    - Implement hover documentation popover (`K` in Normal mode).
  - **Verification**: Execute `gd` on a function call to jump to its definition.

---

## Dependency Graph

```mermaid
graph TD
    subgraph "Phase 6: Core Parity & Hardening (P1)"
        T6_1["6.1 Fix Paste Semantics"]
        T6_2["6.2 Complete Goto Motions"]
        T6_3["6.3 Helix Primitives (o, r, %, ;)"]
        T6_4["6.4 GTK Test Suite Hardening"]
    end

    subgraph "Phase 7: Keymap Decomposition (P1)"
        T7_1["7.1 Decompose src/keymap/"]
        T7_2["7.2 Custom Keybindings"]
    end

    subgraph "Phase 8: Command Palette & Fuzzy Picker (P2)"
        T8_1["8.1 Fuzzy Search Engine"] --> T8_2["8.2 Palette Dialog Component"]
        T8_2 --> T8_3["8.3 File Picker (Space f)"]
        T8_2 --> T8_4["8.4 Buffer Picker (Space b)"]
        T8_2 --> T8_5["8.5 Command Mode (:)"]
    end

    subgraph "Phase 9: Multi-Doc & libpanel Docking (P2)"
        T9_1["9.1 Multi-Doc State Model"] --> T9_2["9.2 libpanel Dock & Grid"]
        T9_2 --> T9_3["9.3 Split Panes (Ctrl-w)"]
    end

    subgraph "Phase 10: LSP Integration (P3)"
        T10_1["10.1 Stdio JSON-RPC Worker"] --> T10_2["10.2 Diagnostics & Gutter"]
        T10_1 --> T10_3["10.3 SourceCompletionProvider"]
        T10_1 --> T10_4["10.4 LSP Nav (gd, gr, K)"]
    end

    T6_1 --> T7_1
    T6_2 --> T7_1
    T6_3 --> T7_1
    T7_1 --> T8_3
    T8_4 --> T9_1
    T9_1 --> T9_2
    T9_2 --> T10_2
```

use gtk::prelude::*;

use crate::commands::Context;
use crate::components::editor::controller::KeyHandleResult;
use crate::core as motions;
use crate::keymap::{trie::KeyCode, Mode};

pub fn enter_insert(cx: &mut Context) -> KeyHandleResult {
    cx.state.set_mode(Mode::Insert);
    KeyHandleResult::ModeChanged(Mode::Insert)
}

pub fn enter_insert_after(cx: &mut Context) -> KeyHandleResult {
    motions::move_horizontally(cx.buffer, 1, false);
    cx.state.set_mode(Mode::Insert);
    KeyHandleResult::ModeChanged(Mode::Insert)
}

pub fn insert_at_line_start(cx: &mut Context) -> KeyHandleResult {
    motions::insert_at_line_start(cx.buffer);
    cx.state.set_mode(Mode::Insert);
    KeyHandleResult::ModeChanged(Mode::Insert)
}

pub fn insert_at_line_end(cx: &mut Context) -> KeyHandleResult {
    motions::insert_at_line_end(cx.buffer);
    cx.state.set_mode(Mode::Insert);
    KeyHandleResult::ModeChanged(Mode::Insert)
}

pub fn enter_select(cx: &mut Context) -> KeyHandleResult {
    if cx.state.mode == Mode::Select {
        cx.state.set_mode(Mode::Normal);
        let iter = cx.buffer.iter_at_mark(&cx.buffer.get_insert());
        cx.buffer.place_cursor(&iter);
        KeyHandleResult::ModeChanged(Mode::Normal)
    } else {
        cx.state.set_mode(Mode::Select);
        KeyHandleResult::ModeChanged(Mode::Select)
    }
}

pub fn exit_to_normal(cx: &mut Context) -> KeyHandleResult {
    cx.state.set_mode(Mode::Normal);
    let iter = cx.buffer.iter_at_mark(&cx.buffer.get_insert());
    cx.buffer.place_cursor(&iter);
    KeyHandleResult::ModeChanged(Mode::Normal)
}

pub fn delete_selection(cx: &mut Context) -> KeyHandleResult {
    let txt = motions::yank_selection(cx.buffer);
    motions::delete_selection(cx.buffer);
    if !txt.is_empty() {
        cx.state.registers.write(cx.register(), &txt);
    }
    if cx.state.mode == Mode::Select {
        cx.state.set_mode(Mode::Normal);
        return KeyHandleResult::ModeChanged(Mode::Normal);
    }
    KeyHandleResult::Stop
}

pub fn change_selection(cx: &mut Context) -> KeyHandleResult {
    let txt = motions::yank_selection(cx.buffer);
    motions::delete_selection(cx.buffer);
    if !txt.is_empty() {
        cx.state.registers.write(cx.register(), &txt);
    }
    cx.state.set_mode(Mode::Insert);
    KeyHandleResult::ModeChanged(Mode::Insert)
}

pub fn yank_selection(cx: &mut Context) -> KeyHandleResult {
    let txt = motions::yank_selection(cx.buffer);
    if !txt.is_empty() {
        let reg = cx.register();
        cx.state.registers.write(reg, &txt);
        if reg == '"' {
            cx.state.registers.write('0', &txt);
        }
        cx.set_status("1 selection yanked");
    }
    if cx.state.mode == Mode::Select {
        cx.state.set_mode(Mode::Normal);
        let iter = cx.buffer.iter_at_mark(&cx.buffer.get_insert());
        cx.buffer.place_cursor(&iter);
        return KeyHandleResult::ModeChanged(Mode::Normal);
    }
    KeyHandleResult::Stop
}

pub fn paste_after(cx: &mut Context) -> KeyHandleResult {
    let text = cx.state.registers.read(cx.register()).to_string();
    if !text.is_empty() {
        for _ in 0..cx.count() {
            motions::paste_after(cx.buffer, &text);
        }
        cx.set_status("Pasted");
    }
    if cx.state.mode == Mode::Select {
        cx.state.set_mode(Mode::Normal);
        let iter = cx.buffer.iter_at_mark(&cx.buffer.get_insert());
        cx.buffer.place_cursor(&iter);
        return KeyHandleResult::ModeChanged(Mode::Normal);
    }
    KeyHandleResult::Stop
}

pub fn paste_before(cx: &mut Context) -> KeyHandleResult {
    let text = cx.state.registers.read(cx.register()).to_string();
    if !text.is_empty() {
        for _ in 0..cx.count() {
            motions::paste_before(cx.buffer, &text);
        }
        cx.set_status("Pasted");
    }
    if cx.state.mode == Mode::Select {
        cx.state.set_mode(Mode::Normal);
        let iter = cx.buffer.iter_at_mark(&cx.buffer.get_insert());
        cx.buffer.place_cursor(&iter);
        return KeyHandleResult::ModeChanged(Mode::Normal);
    }
    KeyHandleResult::Stop
}

pub fn yank_to_clipboard(cx: &mut Context) -> KeyHandleResult {
    let txt = motions::yank_selection(cx.buffer);
    if !txt.is_empty() {
        cx.state.registers.write('+', &txt);
        cx.set_status("Yanked to clipboard (+)");
    }
    if cx.state.mode == Mode::Select {
        cx.state.set_mode(Mode::Normal);
        let iter = cx.buffer.iter_at_mark(&cx.buffer.get_insert());
        cx.buffer.place_cursor(&iter);
        return KeyHandleResult::ModeChanged(Mode::Normal);
    }
    KeyHandleResult::Stop
}

pub fn paste_clipboard_after(cx: &mut Context) -> KeyHandleResult {
    let text = cx.state.registers.read('+').to_string();
    if !text.is_empty() {
        for _ in 0..cx.count() {
            motions::paste_after(cx.buffer, &text);
        }
        cx.set_status("Pasted from clipboard (+)");
    }
    if cx.state.mode == Mode::Select {
        cx.state.set_mode(Mode::Normal);
        let iter = cx.buffer.iter_at_mark(&cx.buffer.get_insert());
        cx.buffer.place_cursor(&iter);
        return KeyHandleResult::ModeChanged(Mode::Normal);
    }
    KeyHandleResult::Stop
}

pub fn paste_clipboard_before(cx: &mut Context) -> KeyHandleResult {
    let text = cx.state.registers.read('+').to_string();
    if !text.is_empty() {
        for _ in 0..cx.count() {
            motions::paste_before(cx.buffer, &text);
        }
        cx.set_status("Pasted from clipboard (+)");
    }
    if cx.state.mode == Mode::Select {
        cx.state.set_mode(Mode::Normal);
        let iter = cx.buffer.iter_at_mark(&cx.buffer.get_insert());
        cx.buffer.place_cursor(&iter);
        return KeyHandleResult::ModeChanged(Mode::Normal);
    }
    KeyHandleResult::Stop
}

pub fn undo(cx: &mut Context) -> KeyHandleResult {
    for _ in 0..cx.count() {
        motions::undo(cx.buffer);
    }
    cx.set_status("Undo");
    if cx.state.mode == Mode::Select {
        cx.state.set_mode(Mode::Normal);
        return KeyHandleResult::ModeChanged(Mode::Normal);
    }
    KeyHandleResult::Stop
}

pub fn redo(cx: &mut Context) -> KeyHandleResult {
    for _ in 0..cx.count() {
        motions::redo(cx.buffer);
    }
    cx.set_status("Redo");
    if cx.state.mode == Mode::Select {
        cx.state.set_mode(Mode::Normal);
        return KeyHandleResult::ModeChanged(Mode::Normal);
    }
    KeyHandleResult::Stop
}

pub fn select_register(cx: &mut Context) -> KeyHandleResult {
    cx.state.which_key = Some(crate::components::which_key::WhichKeyData::from_registers(
        &cx.state.registers,
    ));
    cx.on_next_key(|state, _buf, key| {
        state.which_key = None;
        if let KeyCode::Char(ch) = key.code {
            state.selected_register = Some(ch);
        }
        KeyHandleResult::Stop
    });
    KeyHandleResult::Stop
}

pub fn surround_add(cx: &mut Context) -> KeyHandleResult {
    cx.on_next_key(|state, buffer, key| {
        if let KeyCode::Char(ch) = key.code {
            motions::surround_add(buffer, ch);
            if state.mode == Mode::Select {
                state.set_mode(Mode::Normal);
                return KeyHandleResult::ModeChanged(Mode::Normal);
            }
        }
        KeyHandleResult::Stop
    });
    KeyHandleResult::Stop
}

pub fn surround_delete(cx: &mut Context) -> KeyHandleResult {
    cx.on_next_key(|state, buffer, key| {
        if let KeyCode::Char(ch) = key.code {
            motions::surround_delete(buffer, ch);
            if state.mode == Mode::Select {
                state.set_mode(Mode::Normal);
                return KeyHandleResult::ModeChanged(Mode::Normal);
            }
        }
        KeyHandleResult::Stop
    });
    KeyHandleResult::Stop
}

pub fn surround_replace(cx: &mut Context) -> KeyHandleResult {
    cx.on_next_key(|_state, _buffer, key| {
        if let KeyCode::Char(from) = key.code {
            _state.on_next_key(move |state, buffer, key2| {
                if let KeyCode::Char(to) = key2.code {
                    motions::surround_replace(buffer, from, to);
                    if state.mode == Mode::Select {
                        state.set_mode(Mode::Normal);
                        return KeyHandleResult::ModeChanged(Mode::Normal);
                    }
                }
                KeyHandleResult::Stop
            });
        }
        KeyHandleResult::Stop
    });
    KeyHandleResult::Stop
}

pub fn select_textobject_around(cx: &mut Context) -> KeyHandleResult {
    cx.on_next_key(|_state, buffer, key| {
        if let KeyCode::Char(obj) = key.code {
            motions::select_textobject(buffer, obj, false);
        }
        KeyHandleResult::Stop
    });
    KeyHandleResult::Stop
}

pub fn select_textobject_inner(cx: &mut Context) -> KeyHandleResult {
    cx.on_next_key(|_state, buffer, key| {
        if let KeyCode::Char(obj) = key.code {
            motions::select_textobject(buffer, obj, true);
        }
        KeyHandleResult::Stop
    });
    KeyHandleResult::Stop
}

pub fn open_below(cx: &mut Context) -> KeyHandleResult {
    motions::open_below(cx.buffer, cx.count());
    cx.state.set_mode(Mode::Insert);
    KeyHandleResult::ModeChanged(Mode::Insert)
}

pub fn open_above(cx: &mut Context) -> KeyHandleResult {
    motions::open_above(cx.buffer, cx.count());
    cx.state.set_mode(Mode::Insert);
    KeyHandleResult::ModeChanged(Mode::Insert)
}

pub fn replace(cx: &mut Context) -> KeyHandleResult {
    cx.on_next_key(|state, buffer, key| {
        let ch = match key.code {
            KeyCode::Char(c) => Some(c.to_string()),
            KeyCode::Enter => Some("\n".to_string()),
            KeyCode::Tab => Some("\t".to_string()),
            _ => None,
        };
        if let Some(s) = ch {
            motions::replace_char(buffer, &s);
        }
        if state.mode == Mode::Select {
            state.set_mode(Mode::Normal);
            let iter = buffer.iter_at_mark(&buffer.get_insert());
            buffer.place_cursor(&iter);
            return KeyHandleResult::ModeChanged(Mode::Normal);
        }
        KeyHandleResult::Stop
    });
    KeyHandleResult::Stop
}

pub fn select_all(cx: &mut Context) -> KeyHandleResult {
    motions::select_all(cx.buffer);
    KeyHandleResult::Stop
}

pub fn collapse_selection(cx: &mut Context) -> KeyHandleResult {
    motions::collapse_selection(cx.buffer);
    KeyHandleResult::Stop
}

pub fn flip_selection(cx: &mut Context) -> KeyHandleResult {
    motions::flip_selection(cx.buffer);
    KeyHandleResult::Stop
}

pub fn toggle_case(cx: &mut Context) -> KeyHandleResult {
    motions::toggle_case(cx.buffer);
    if cx.state.mode == Mode::Select {
        cx.state.set_mode(Mode::Normal);
        let iter = cx.buffer.iter_at_mark(&cx.buffer.get_insert());
        cx.buffer.place_cursor(&iter);
        return KeyHandleResult::ModeChanged(Mode::Normal);
    }
    KeyHandleResult::Stop
}

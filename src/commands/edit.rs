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
    }
    KeyHandleResult::Stop
}

pub fn paste_before(cx: &mut Context) -> KeyHandleResult {
    let text = cx.state.registers.read(cx.register()).to_string();
    if !text.is_empty() {
        for _ in 0..cx.count() {
            motions::paste_after(cx.buffer, &text);
        }
    }
    KeyHandleResult::Stop
}

pub fn yank_to_clipboard(cx: &mut Context) -> KeyHandleResult {
    let txt = motions::yank_selection(cx.buffer);
    if !txt.is_empty() {
        cx.state.registers.write('+', &txt);
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
    }
    KeyHandleResult::Stop
}

pub fn paste_clipboard_before(cx: &mut Context) -> KeyHandleResult {
    let text = cx.state.registers.read('+').to_string();
    if !text.is_empty() {
        for _ in 0..cx.count() {
            motions::paste_after(cx.buffer, &text);
        }
    }
    KeyHandleResult::Stop
}

pub fn undo(cx: &mut Context) -> KeyHandleResult {
    for _ in 0..cx.count() {
        motions::undo(cx.buffer);
    }
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
    if cx.state.mode == Mode::Select {
        cx.state.set_mode(Mode::Normal);
        return KeyHandleResult::ModeChanged(Mode::Normal);
    }
    KeyHandleResult::Stop
}

pub fn select_register(cx: &mut Context) -> KeyHandleResult {
    cx.on_next_key(|state, _buf, key| {
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

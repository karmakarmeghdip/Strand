use crate::commands::Context;
use crate::components::editor::controller::KeyHandleResult;
use crate::core as motions;

pub fn move_left(cx: &mut Context) -> KeyHandleResult {
    motions::move_horizontally(cx.buffer, -(cx.count() as i32), cx.extend());
    KeyHandleResult::Stop
}

pub fn move_right(cx: &mut Context) -> KeyHandleResult {
    motions::move_horizontally(cx.buffer, cx.count() as i32, cx.extend());
    KeyHandleResult::Stop
}

pub fn move_up(cx: &mut Context) -> KeyHandleResult {
    motions::move_vertically(cx.buffer, -(cx.count() as i32), cx.extend());
    KeyHandleResult::Stop
}

pub fn move_down(cx: &mut Context) -> KeyHandleResult {
    motions::move_vertically(cx.buffer, cx.count() as i32, cx.extend());
    KeyHandleResult::Stop
}

pub fn move_word_forward(cx: &mut Context) -> KeyHandleResult {
    let extend = cx.extend();
    for _ in 0..cx.count() {
        motions::move_word_forward(cx.buffer, extend);
    }
    KeyHandleResult::Stop
}

pub fn move_word_backward(cx: &mut Context) -> KeyHandleResult {
    let extend = cx.extend();
    for _ in 0..cx.count() {
        motions::move_word_backward(cx.buffer, extend);
    }
    KeyHandleResult::Stop
}

pub fn move_word_end(cx: &mut Context) -> KeyHandleResult {
    let extend = cx.extend();
    for _ in 0..cx.count() {
        motions::move_word_end(cx.buffer, extend);
    }
    KeyHandleResult::Stop
}

pub fn select_line(cx: &mut Context) -> KeyHandleResult {
    let extend = cx.extend();
    for _ in 0..cx.count() {
        motions::select_line(cx.buffer, extend);
    }
    KeyHandleResult::Stop
}

pub fn match_brackets(cx: &mut Context) -> KeyHandleResult {
    motions::match_brackets(cx.buffer, cx.state.last_matched_bracket, cx.extend());
    KeyHandleResult::Stop
}

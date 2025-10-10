use crate::input::InputState;
use crossterm::event::KeyCode;

pub fn handle_dialog_input(input: &mut InputState, key_code: KeyCode) -> DialogAction {
    match key_code {
        KeyCode::Esc => DialogAction::Cancel,
        KeyCode::Enter => {
            if !input.is_empty() {
                DialogAction::Confirm
            } else {
                DialogAction::None
            }
        }
        KeyCode::Char(c) => {
            input.insert_char(c);
            DialogAction::None
        }
        KeyCode::Backspace => {
            input.backspace();
            DialogAction::None
        }
        KeyCode::Delete => {
            input.delete();
            DialogAction::None
        }
        KeyCode::Left => {
            input.move_left();
            DialogAction::None
        }
        KeyCode::Right => {
            input.move_right();
            DialogAction::None
        }
        KeyCode::Home => {
            input.move_home();
            DialogAction::None
        }
        KeyCode::End => {
            input.move_end();
            DialogAction::None
        }
        _ => DialogAction::None,
    }
}

pub enum DialogAction {
    None,
    Cancel,
    Confirm,
}

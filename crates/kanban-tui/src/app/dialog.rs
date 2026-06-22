use super::{swap_known_extension, App, DialogMode, StorageBackendChoice};

impl App {
    pub fn open_dialog(&mut self, dialog: DialogMode) {
        self.push_mode(super::AppMode::Dialog(dialog));
    }

    pub fn maybe_push_startup_file_dialog(&mut self) {
        if !self.has_data_file {
            self.choose_storage_backend = StorageBackendChoice::Json;
            self.input
                .set(format!("boards{}", StorageBackendChoice::Json.extension()));
            self.open_dialog(DialogMode::ChooseStorageFile);
        }
    }

    pub fn handle_choose_storage_file_dialog(&mut self, key_code: crossterm::event::KeyCode) {
        use crate::dialog::{handle_dialog_input, DialogAction};
        if matches!(key_code, crossterm::event::KeyCode::Tab) {
            self.choose_storage_backend = self.choose_storage_backend.toggle();
            let new_filename =
                swap_known_extension(self.input.as_str(), self.choose_storage_backend.extension());
            self.input.set(new_filename);
            return;
        }
        match handle_dialog_input(&mut self.input, key_code, false) {
            DialogAction::Confirm => {
                let filename = self.input.as_str().to_string();
                if self.adopt_storage_file(filename) {
                    self.input.clear();
                    self.pop_mode();
                }
                // On failure, leave the dialog open so the user can correct
                // the path and retry; the error banner explains what went wrong.
            }
            DialogAction::Cancel => {
                self.input.clear();
                self.pop_mode();
            }
            DialogAction::None => {}
        }
    }
}

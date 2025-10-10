use crate::events::EventHandler;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

fn which_editor() -> String {
    let editors = if cfg!(target_os = "windows") {
        vec!["nvim", "vim", "nano", "notepad"]
    } else {
        vec!["nvim", "vim", "nano", "vi"]
    };

    for editor in &editors {
        let which_cmd = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };

        if Command::new(which_cmd)
            .arg(editor)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            return editor.to_string();
        }
    }

    if cfg!(target_os = "windows") {
        "notepad".to_string()
    } else {
        "vi".to_string()
    }
}

pub fn edit_in_external_editor(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    event_handler: &EventHandler,
    temp_file: PathBuf,
    initial_content: &str,
) -> io::Result<Option<String>> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| which_editor());

    std::fs::write(&temp_file, initial_content)?;

    event_handler.stop();

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    io::stdout().flush()?;

    let status = Command::new(&editor).arg(&temp_file).status();

    if let Err(ref e) = status {
        tracing::error!("Failed to launch editor '{}': {}", editor, e);
        execute!(io::stdout(), EnterAlternateScreen)?;
        enable_raw_mode()?;
        terminal.clear()?;
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Editor '{}' not found. Please set $EDITOR environment variable.",
                editor
            ),
        ));
    }

    let status = status.unwrap();

    while crossterm::event::poll(std::time::Duration::from_millis(0))? {
        let _ = crossterm::event::read()?;
    }

    execute!(io::stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;

    terminal.clear()?;

    let result = if status.success() {
        let content = std::fs::read_to_string(&temp_file)?;
        Some(content)
    } else {
        None
    };

    let _ = std::fs::remove_file(&temp_file);

    Ok(result)
}

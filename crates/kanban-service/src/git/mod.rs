pub mod config;
mod provider;
mod shell;

pub use provider::{CommitRef, GitProvider};
pub use shell::ShellGitProvider;

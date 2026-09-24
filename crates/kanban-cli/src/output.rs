use serde::Serialize;

#[derive(Serialize)]
pub struct CliResponse<T: Serialize> {
    pub success: bool,
    pub api_version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn output_success<T: Serialize>(data: T) {
    let response = CliResponse {
        success: true,
        api_version: env!("CARGO_PKG_VERSION"),
        data: Some(data),
        error: None,
    };
    println!("{}", serde_json::to_string(&response).unwrap());
}

fn error_envelope(message: &str) -> String {
    let response: CliResponse<()> = CliResponse {
        success: false,
        api_version: env!("CARGO_PKG_VERSION"),
        data: None,
        error: Some(message.to_string()),
    };
    serde_json::to_string(&response).unwrap()
}

/// Writes the `CliResponse` failure envelope to stderr.
pub fn emit_error(message: &str) {
    eprintln!("{}", error_envelope(message));
}

/// Returns the CLI-boundary error for a handler to propagate. The envelope
/// itself is written once, by `CliApp::run_with_args`.
pub fn output_error(message: &str) -> anyhow::Result<()> {
    anyhow::bail!("{}", message)
}

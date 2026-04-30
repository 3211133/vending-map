use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::{
    error::AppError,
    models::image::{AnalysisOutput, SlotAnalysis},
};

pub async fn analyze_vending_image(
    image_bytes: &[u8],
    script_path: &str,
) -> Result<Vec<SlotAnalysis>, AppError> {
    let mut child = Command::new("python3")
        .arg(script_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::upstream(format!("Failed to spawn analyzer: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(image_bytes)
            .await
            .map_err(|e| AppError::upstream(format!("Failed to write image to analyzer: {e}")))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| AppError::upstream(format!("Analyzer process error: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::upstream(format!("Analyzer exited non-zero: {stderr}")));
    }

    let result: AnalysisOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| AppError::upstream(format!("Failed to parse analyzer output: {e}")))?;

    if let Some(err) = result.error {
        return Err(AppError::upstream(format!("Analyzer reported error: {err}")));
    }

    // Normalize temperature values to DB constraint values
    let slots = result
        .slots
        .into_iter()
        .map(|mut s| {
            if !matches!(s.temperature.as_str(), "hot" | "cold" | "warm" | "unknown") {
                s.temperature = "unknown".to_string();
            }
            s
        })
        .collect();

    Ok(slots)
}

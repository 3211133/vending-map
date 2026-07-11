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
        .map_err(|e| AppError::internal(format!("Failed to spawn analyzer: {e}")))?;

    // stdin への書き込みと子プロセスの終了待ちを並行して行う。
    // アナライザが stdin を読み切らずに終了するケース(依存ライブラリ不足など)では
    // 書き込みが EPIPE で失敗するが、その場合も stdout の JSON エラーや
    // 終了コードの方が原因を正しく伝えるため、書き込みエラーでは中断しない。
    let stdin = child.stdin.take();
    let write_stdin = async {
        if let Some(mut stdin) = stdin {
            if let Err(e) = stdin.write_all(image_bytes).await {
                tracing::debug!("Analyzer stdin write aborted: {e}");
            }
        }
    };
    let (output, ()) = tokio::join!(child.wait_with_output(), write_stdin);
    let output = output.map_err(|e| AppError::internal(format!("Analyzer process error: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::internal(format!(
            "Analyzer exited non-zero: {stderr}"
        )));
    }

    let result: AnalysisOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| AppError::internal(format!("Failed to parse analyzer output: {e}")))?;

    if let Some(err) = result.error {
        return Err(AppError::internal(format!(
            "Analyzer reported error: {err}"
        )));
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

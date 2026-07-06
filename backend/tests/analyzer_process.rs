use std::path::PathBuf;

use vending_map_backend::services::image_analysis;

/// テスト用のアナライザスクリプトを一時ファイルとして書き出す。
struct TempScript {
    path: PathBuf,
}

impl TempScript {
    fn new(name: &str, body: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "vending-map-test-{}-{}.py",
            name,
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&path, body).expect("write temp analyzer script");
        Self { path }
    }

    fn path_str(&self) -> &str {
        self.path.to_str().expect("temp path is valid UTF-8")
    }
}

impl Drop for TempScript {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// アナライザが stdin を読み切らずに JSON エラーを返して終了した場合
/// (analyze_image.py の依存ライブラリ不足パスと同じ挙動)でも、
/// EPIPE ではなくアナライザ自身のエラーメッセージが返ることを検証する。
/// パイプバッファ(通常 64 KiB)を確実に超える 1 MiB の入力を使う。
#[tokio::test]
async fn analyzer_error_is_surfaced_when_stdin_is_not_consumed() {
    let script = TempScript::new(
        "no-stdin-read",
        r#"
import json, sys
json.dump({"error": "Missing dependency: cv2", "slots": []}, sys.stdout)
"#,
    );

    let big_image = vec![0u8; 1024 * 1024];
    let err = image_analysis::analyze_vending_image(&big_image, script.path_str())
        .await
        .expect_err("analyzer error should be reported");

    let message = err.to_string();
    assert!(
        message.contains("Missing dependency: cv2"),
        "expected analyzer's own error message, got: {message}"
    );
}

/// stdin を全部読んでスロットを返す正常系。温度の正規化も確認する。
#[tokio::test]
async fn analyzer_output_is_parsed_and_temperature_normalized() {
    let script = TempScript::new(
        "echo-slots",
        r#"
import json, sys
data = sys.stdin.buffer.read()
assert len(data) == 1024 * 1024
json.dump({"slots": [
    {"price_yen": 130, "temperature": "cold"},
    {"price_yen": None, "temperature": "lukewarm"},
]}, sys.stdout)
"#,
    );

    let big_image = vec![0u8; 1024 * 1024];
    let slots = image_analysis::analyze_vending_image(&big_image, script.path_str())
        .await
        .expect("analyzer should succeed");

    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0].price_yen, Some(130));
    assert_eq!(slots[0].temperature, "cold");
    // DB 制約にない値は unknown に正規化される
    assert_eq!(slots[1].temperature, "unknown");
}

/// 非ゼロ終了時は stderr の内容がエラーメッセージに含まれる。
#[tokio::test]
async fn analyzer_nonzero_exit_reports_stderr() {
    let script = TempScript::new(
        "crash",
        r#"
import sys
sys.stdin.buffer.read()
print("boom: unexpected failure", file=sys.stderr)
sys.exit(1)
"#,
    );

    let err = image_analysis::analyze_vending_image(b"tiny", script.path_str())
        .await
        .expect_err("non-zero exit should be an error");

    let message = err.to_string();
    assert!(
        message.contains("boom: unexpected failure"),
        "expected stderr in error message, got: {message}"
    );
}

use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const TIMEOUT: Duration = Duration::from_secs(30);

enum RunError {
    NotFound,
    Timeout,
    NonZeroExit { code: Option<i32>, stderr: String },
    Io(std::io::Error),
}

impl RunError {
    fn to_message(&self) -> String {
        match self {
            RunError::NotFound => {
                "claude コマンドが見つかりませんでした。Claude Code CLI がインストールされ、PATH が通っているか確認してください。".to_string()
            }
            RunError::Timeout => {
                "claude コマンドの応答がタイムアウトしました(30秒)。認証状態などを確認してください。".to_string()
            }
            RunError::NonZeroExit { code, stderr } => {
                let code_str = code.map(|c| c.to_string()).unwrap_or_else(|| "unknown".to_string());
                if stderr.trim().is_empty() {
                    format!("claude コマンドがエラー終了しました(終了コード: {code_str})。")
                } else {
                    format!("claude コマンドがエラー終了しました(終了コード: {code_str})。\n{}", stderr.trim())
                }
            }
            RunError::Io(e) => format!("claude コマンドの実行中にエラーが発生しました: {e}"),
        }
    }
}

fn strip_ansi(bytes: &[u8]) -> String {
    let stripped = strip_ansi_escapes::strip(bytes);
    String::from_utf8_lossy(&stripped).into_owned()
}

async fn run(program: &str, args: &[&str]) -> Result<String, RunError> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            RunError::NotFound
        } else {
            RunError::Io(e)
        }
    })?;

    let output = match timeout(TIMEOUT, child.wait_with_output()).await {
        Ok(res) => res.map_err(RunError::Io)?,
        Err(_) => return Err(RunError::Timeout),
    };

    if !output.status.success() {
        return Err(RunError::NonZeroExit {
            code: output.status.code(),
            stderr: strip_ansi(&output.stderr),
        });
    }

    Ok(strip_ansi(&output.stdout))
}

/// `claude -p "/usage"` を実行し、Usage情報のテキストを取得する。
/// ネイティブ実行ファイルとして直接起動できない場合(npm版シム等)は
/// `cmd /C` 経由でのPATH解決にフォールバックする。
pub async fn fetch_usage() -> Result<String, String> {
    match run("claude", &["-p", "/usage"]).await {
        Ok(out) => Ok(out),
        // cmd フォールバックが失敗した場合、原因(cmd自身のロケール依存メッセージ等)に関わらず
        // 「claude が見つからない」という結論は変わらないため、元のNotFoundメッセージに統一する。
        Err(RunError::NotFound) => run("cmd", &["/C", "claude", "-p", "/usage"])
            .await
            .map_err(|_| RunError::NotFound.to_message()),
        Err(e) => Err(e.to_message()),
    }
}

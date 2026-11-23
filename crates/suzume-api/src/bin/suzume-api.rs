//! Native access to the same protocol used by the WebAssembly binding.
use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};
use suzume_api::{
    logging::{self, LogLimits},
    types::{ApiError, ErrorCode},
};

struct PendingFile(PathBuf);
impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn export_dataset(input: &Path, destination: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if destination.exists() {
        return Err("出力先が存在します。別のファイル名を指定してください。".into());
    }
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary_path = parent.join(format!(".suzume-dataset-{}.pending", std::process::id()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)?;
    let temporary = PendingFile(temporary_path);
    let mut output = BufWriter::new(file);
    let mut reader = BufReader::new(File::open(input)?);
    let result = logging::export_transitions(&mut reader, LogLimits::default(), |transition| {
        serde_json::to_writer(&mut output, transition)
            .map_err(|e| ApiError::new(ErrorCode::LogError, e.to_string()))?;
        output
            .write_all(b"\n")
            .map_err(|e| ApiError::new(ErrorCode::LogError, e.to_string()))
    })?;
    output.flush()?;
    output.get_ref().sync_all()?;
    drop(output);
    // Linking a sibling temporary file publishes atomically and never replaces an existing destination.
    std::fs::hard_link(&temporary.0, destination)?;
    eprintln!(
        "{}操作の学習用遷移を出力しました。終了種別: {:?}",
        result.termination.action_count, result.termination.reason
    );
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] | ["serve"] => suzume_api::transport::serve(&mut io::stdin().lock(), &mut io::stdout().lock())?,
        ["config", path] => {
            let mut text = String::new();
            File::open(path)?.take(suzume_api::MAX_REQUEST_BYTES as u64 + 1).read_to_string(&mut text)?;
            let mut engine = suzume_api::Engine::new();
            let data = engine.execute(suzume_api::protocol::Request::InspectConfig { version: suzume_api::types::Version, toml: text })?;
            writeln!(io::stdout().lock(), "{}", serde_json::to_string_pretty(&data)?)?;
        },
        ["replay", path] => {
            let result = logging::replay(&mut BufReader::new(File::open(path)?), LogLimits::default())?;
            let summary = serde_json::json!({"engine":result.metadata.engine,"action_count":result.termination.action_count,
                "reason":result.termination.reason,"scores":result.termination.scores,"riichi_pot":result.termination.riichi_pot});
            writeln!(io::stdout().lock(), "{}", serde_json::to_string_pretty(&summary)?)?;
        },
        ["dataset", path, "--output", destination] => export_dataset(Path::new(path), Path::new(destination))?,
        ["--help"] | ["-h"] => writeln!(io::stdout().lock(), "suzume-api [serve]\n  版1のJSON要求を標準入力から1行ずつ読み、JSON応答を出力します。\nsuzume-api config PATH\n  TOML設定を検査し、設定と実効AI値を表示します。\nsuzume-api replay PATH\n  完全ログを末尾まで検査・再生して終了状態を表示します。\nsuzume-api dataset PATH --output PATH\n  完全ログから学習用JSONLを作ります。全検証成功後に新しい出力先へ確定します。")?,
        _ => return Err("未対応の引数です。suzume-api --helpで使用方法を確認してください。".into()),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("実行エラー: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_to_remove_a_temporary_file_it_did_not_create() {
        let directory = std::env::temp_dir().join(format!(
            "suzume-conflicting-temporary-{}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let temporary = directory.join(format!(".suzume-dataset-{}.pending", std::process::id()));
        std::fs::write(&temporary, b"existing data").unwrap();
        let result = export_dataset(
            &directory.join("unused-input"),
            &directory.join("output.jsonl"),
        );
        assert!(result.is_err());
        let retained = std::fs::read(&temporary);
        std::fs::remove_dir_all(&directory).unwrap();
        assert_eq!(retained.unwrap(), b"existing data");
    }
}

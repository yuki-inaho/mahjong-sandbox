//! Native access to the same protocol used by the WebAssembly binding.
use std::{
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};
use suzume_api::{
    logging::{self, LogLimits},
    mjai::{self, annotated::AnnotationPolicy},
    protocol::MjaiFormat,
    types::{ApiError, ErrorCode},
};

struct PendingFile(PathBuf);
impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Publishes a fully validated output atomically without replacing an existing path.
fn publish_output<F, T>(destination: &Path, produce: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(&mut BufWriter<File>) -> Result<T, Box<dyn std::error::Error>>,
{
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
    let result = produce(&mut output)?;
    output.flush()?;
    output.get_ref().sync_all()?;
    drop(output);
    std::fs::hard_link(&temporary.0, destination)?;
    Ok(result)
}

fn json_line<T: serde::Serialize>(output: &mut impl Write, value: &T) -> Result<(), ApiError> {
    serde_json::to_writer(&mut *output, value)
        .map_err(|e| ApiError::new(ErrorCode::LogError, e.to_string()))?;
    output
        .write_all(b"\n")
        .map_err(|e| ApiError::new(ErrorCode::LogError, e.to_string()))
}

fn export_dataset(input: &Path, destination: &Path) -> Result<(), Box<dyn std::error::Error>> {
    publish_output(destination, |output| {
        let result = logging::export_transitions(
            &mut BufReader::new(File::open(input)?),
            LogLimits::default(),
            |row| json_line(output, row),
        )?;
        eprintln!(
            "{}操作の学習用遷移を検証しました。終了種別: {:?}",
            result.termination.action_count, result.termination.reason
        );
        Ok(())
    })
}

/// Gzip is identified by its magic bytes, including files named only .mjson.
/// Both compressed input and decompressed records have explicit byte limits.
fn open_mjai(input: &Path) -> Result<Box<dyn BufRead>, Box<dyn std::error::Error>> {
    let maximum = LogLimits::default().max_total_bytes as u64;
    let file = File::open(input)?;
    if file.metadata()?.len() > maximum {
        return Err("mjson compressed/input byte limit exceeded".into());
    }
    let mut reader = BufReader::new(file.take(maximum + 1));
    let magic = reader.fill_buf()?;
    if magic.starts_with(&[0x1f, 0x8b]) {
        Ok(Box::new(BufReader::new(
            flate2::bufread::MultiGzDecoder::new(reader),
        )))
    } else {
        Ok(Box::new(reader))
    }
}

fn convert_mjai(input: &Path, destination: &Path) -> Result<(), Box<dyn std::error::Error>> {
    publish_output(destination, |output| {
        // Validate the entire legacy input before writing any converted events.
        let replay = logging::replay(
            &mut BufReader::new(File::open(input)?),
            LogLimits::default(),
        )?;
        let mut game = suzume::game::Game::new(replay.metadata.seed.value())?;
        let mut writer =
            suzume_api::mjai::MjaiWriter::new(output, &replay.metadata.config, &game, false)?;
        for operation in replay.game.history() {
            game.apply(operation.player, operation.action.clone())?;
            writer.record(operation.player, &operation.action, &game, None)?;
        }
        if game != replay.game {
            return Err("変換中の局面が検証済み再生と一致しません。".into());
        }
        writer.finish(&game, replay.termination.reason)?;
        Ok(())
    })
}

enum MjaiConversion {
    Wrap,
    Unwrap(AnnotationPolicy),
}

fn transcode_mjai(
    input: &Path,
    destination: &Path,
    conversion: MjaiConversion,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = publish_output(destination, |output| {
        let mut reader = open_mjai(input)?;
        let limits = LogLimits::default();
        Ok(match conversion {
            MjaiConversion::Wrap => mjai::annotated::wrap(&mut reader, output, limits)?,
            MjaiConversion::Unwrap(policy) => {
                mjai::annotated::unwrap(&mut reader, output, limits, policy)?
            }
        })
    })?;
    // Report success only after validation, flush, sync and publication succeed.
    let report = serde_json::json!({
        "summary": report.summary,
        "dropped_annotation_records": report.dropped_annotation_records,
    });
    let report_result = (|| -> Result<(), Box<dyn std::error::Error>> {
        writeln!(
            io::stdout().lock(),
            "{}",
            serde_json::to_string_pretty(&report)?
        )?;
        Ok(())
    })();
    report_result.map_err(|cause| {
        format!(
            "出力は {} に公開済みです。結果報告に失敗しました: {cause}",
            destination.display()
        )
        .into()
    })
}

fn mjai_format(arguments: &[&str]) -> Result<MjaiFormat, Box<dyn std::error::Error>> {
    match arguments {
        [] => Ok(MjaiFormat::default()),
        ["--format", format] => Ok(serde_json::from_value(serde_json::Value::String(
            (*format).to_owned(),
        ))?),
        _ => Err("形式は --format mjai または --format annotated_mjai で指定してください。".into()),
    }
}

fn visit_mjai<F>(
    input: &Path,
    format: MjaiFormat,
    consumer: F,
) -> Result<mjai::Summary, Box<dyn std::error::Error>>
where
    F: FnMut(&mjai::ParsedEvent, &mjai::ReplayState) -> Result<(), ApiError>,
{
    let mut reader = open_mjai(input)?;
    let limits = LogLimits::default();
    Ok(match format {
        MjaiFormat::Mjai => mjai::visit(&mut reader, limits, consumer)?,
        MjaiFormat::AnnotatedMjai => mjai::annotated::visit(&mut reader, limits, consumer)?,
    })
}

fn export_observations(
    input: &Path,
    destination: &Path,
    viewer: suzume_api::types::SeatId,
    format: MjaiFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    publish_output(destination, |output| {
        visit_mjai(input, format, |record, state| {
            // Do not include raw events: their hands and draws can be private to other seats.
            let row = serde_json::json!({"schema_version":1,"line":record.line,
                "type":record.raw["type"],"observation":state.observe(Some(viewer))});
            json_line(output, &row)
        })?;
        Ok(())
    })
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
        ["mjai", "analyze", path, format_args @ ..] => {
            let summary=visit_mjai(Path::new(path), mjai_format(format_args)?, |_, _| Ok(()))?;
            writeln!(io::stdout().lock(),"{}",serde_json::to_string_pretty(&summary)?)?;
        },
        ["mjai", "convert", path, "--output", destination] => convert_mjai(Path::new(path),Path::new(destination))?,
        ["mjai", "wrap", path, "--output", destination] => transcode_mjai(Path::new(path),Path::new(destination),MjaiConversion::Wrap)?,
        ["mjai", "unwrap", path, "--output", destination, options @ ..] => {
            let policy = match options {
                [] => AnnotationPolicy::Reject,
                ["--drop-annotations"] => AnnotationPolicy::Drop,
                _ => return Err("注釈を破棄する場合だけ --drop-annotations を指定してください。".into()),
            };
            transcode_mjai(Path::new(path),Path::new(destination),MjaiConversion::Unwrap(policy))?;
        },
        ["mjai", "observations", path, "--seat", seat, "--output", destination, format_args @ ..] => {
            let viewer=seat.parse::<u8>().ok().and_then(suzume_api::types::SeatId::new).ok_or("--seatは0〜3を指定してください。")?;
            export_observations(Path::new(path),Path::new(destination),viewer,mjai_format(format_args)?)?;
        },

        ["--help"] | ["-h"] => writeln!(io::stdout().lock(), "suzume-api mjai analyze PATH [--format mjai|annotated_mjai]\n  指定形式の記録局面を検査・集計します。gzipは内容から判定します。\nsuzume-api mjai wrap INPUT --output OUTPUT\n  原イベントを保持して空の注釈を付けます。\nsuzume-api mjai unwrap INPUT --output OUTPUT [--drop-annotations]\n  標準mjsonへ戻します。非空注釈の破棄には明示指定が必要です。\nsuzume-api mjai convert LEGACY --output PATH\n  旧replayの全検証後にmjsonへ変換します。\nsuzume-api mjai observations PATH --seat 0..3 --output PATH [--format mjai|annotated_mjai]\n  指定席の視点別観測を各イベント後に出力します。\nsuzume-api [serve]\n  版1のJSON要求を標準入力から1行ずつ読み、JSON応答を出力します。\nsuzume-api config PATH\n  TOML設定を検査し、設定と実効AI値を表示します。\nsuzume-api replay PATH\n  完全ログを末尾まで検査・再生して終了状態を表示します。\nsuzume-api dataset PATH --output PATH\n  完全ログから学習用JSONLを作ります。全検証成功後に新しい出力先へ確定します。")?,
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

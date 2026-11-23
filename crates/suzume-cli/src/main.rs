//! 実行入口。設定の解決は共通API、対局・AI判断はcoreへ委譲します。

mod ui;

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;
use suzume::ai::{ConfiguredAi, Decision};
use suzume::game::{Action, Game, Phase};
use suzume_api::config::{AiConfigSpec, AppConfig, Controller, LogFormat};
use suzume_api::decision::DecisionView;
use suzume_api::logging::{EndReason, LogWriter};
use suzume_api::types::Seed;

const SIMULATION_LIMIT: usize = 10_000;

const HELP: &str = "すずめ — Rust製の四人リーチ麻雀\n\n\
使用方法: suzume [--config 設定.toml | --seed 整数] [--plain] [実行モード]\n\n\
実行モード（いずれか1つ）:\n\
  指定なし       人間の席0とCPU3人で東風戦。viewerも席0に固定\n\
  --simulate N   種seedから連続N個の全員CPU対局（1〜10000試合）\n\
  --demo         全員CPUで一局を表示して終了\n\
  --snapshot     初期配牌の画面を表示して終了\n\
  --help, -h     この説明を表示\n\n\
共通の指定:\n\
  --config PATH  共通API形式のTOML設定（最大1MiB）。--seedとの併用不可\n\
  --seed N       0〜18446744073709551615の乱数種。全モードの省略値は0\n\
  --plain        色・画面消去を使わず、制御文字のない行入力で表示\n\
  --log PATH     新しいmjsonファイルへ記録。既存ファイルは上書きしない\n\
  --log-format mjai|mjai_public|replay|public\n\
                 mjaiは分析用、mjai_publicは伏せ牌を省いた共有用。replay/publicは旧形式\n\
                 省略時は設定のlogging.format、設定なしではmjai\n\n\
--simulate/--demoで設定を明示する場合は全員AIが必要です。設定なしは全員Greedy。\n\
TUIと初期表示はviewer=0、demoはviewer=0またはpublicに対応します。\n\
--logは1試合に対応します。--simulate NでNが2以上なら併用できません。\n\
mjai/replayは全手牌を含みます。共有用途はmjai_public/publicを選びます。\n\
対局中: 牌番号で打牌、r 番号でリーチ、/番号で鳴き等を選択。hで操作説明。\n\
標準入出力が端末ではない場合とNO_COLOR設定時は色・画面消去を使いません。\n\
操作上限は設定のgame.max_steps（省略時10000）。Pass・NextRoundも含みます。\n\
上限到達は非0終了です。ログは通常終局、上限到達、手動中断を区別します。\n";

#[derive(Clone, Debug, Eq, PartialEq)]
enum Mode {
    Play,
    Simulate(usize),
    Demo,
    Snapshot,
    Help,
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    mode: Mode,
    seed: Option<u64>,
    config_path: Option<PathBuf>,
    log_path: Option<PathBuf>,
    log_format: Option<LogFormat>,
    plain: bool,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut args = args.into_iter();
        let mut options = Self {
            mode: Mode::Play,
            seed: None,
            config_path: None,
            log_path: None,
            log_format: None,
            plain: false,
        };
        let mut mode_set = false;
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--seed" => {
                    if options.seed.is_some() {
                        return Err("--seedは一度だけ指定できます。".into());
                    }
                    let value = args.next().ok_or("--seedの後に整数が必要です。")?;
                    options.seed = Some(
                        Seed::new(&value)
                            .map_err(|_| "--seedは先頭ゼロや符号のない0以上の64ビット整数で指定してください。")?
                            .value(),
                    );
                }
                "--config" => {
                    if options.config_path.is_some() {
                        return Err("--configは一度だけ指定できます。".into());
                    }
                    options.config_path = Some(
                        args.next()
                            .ok_or("--configの後に設定ファイルが必要です。")?
                            .into(),
                    );
                }
                "--log" => {
                    if options.log_path.is_some() {
                        return Err("--logは一度だけ指定できます。".into());
                    }
                    options.log_path = Some(
                        args.next()
                            .ok_or("--logの後に保存先ファイルが必要です。")?
                            .into(),
                    );
                }
                "--log-format" => {
                    if options.log_format.is_some() {
                        return Err("--log-formatは一度だけ指定できます。".into());
                    }
                    let value = args
                        .next()
                        .ok_or("--log-formatの後にmjai、mjai_public、replay、publicが必要です。")?;
                    options.log_format = Some(match value.as_str() {
                        "mjai" => LogFormat::Mjai,
                        "mjai_public" => LogFormat::MjaiPublic,
                        "replay" => LogFormat::Replay,
                        "public" => LogFormat::Public,
                        _ => return Err(
                            "--log-formatはmjai、mjai_public、replay、publicを指定してください。"
                                .into(),
                        ),
                    });
                }
                "--plain" => {
                    if options.plain {
                        return Err("--plainが重複しています。".into());
                    }
                    options.plain = true;
                }
                "--simulate" => {
                    let value = args.next().ok_or("--simulateの後に試合数が必要です。")?;
                    let count = value
                        .parse::<usize>()
                        .map_err(|_| "試合数は正の整数で指定してください。")?;
                    if !(1..=SIMULATION_LIMIT).contains(&count) {
                        return Err(format!("試合数は1〜{SIMULATION_LIMIT}で指定してください。"));
                    }
                    set_mode(&mut options.mode, &mut mode_set, Mode::Simulate(count))?;
                }
                "--demo" => set_mode(&mut options.mode, &mut mode_set, Mode::Demo)?,
                "--snapshot" => set_mode(&mut options.mode, &mut mode_set, Mode::Snapshot)?,
                "--help" | "-h" => set_mode(&mut options.mode, &mut mode_set, Mode::Help)?,
                _ => {
                    return Err(format!(
                        "未対応の引数です: {argument}\n--helpで使用方法を確認してください。"
                    ))
                }
            }
        }
        if options.config_path.is_some() && options.seed.is_some() {
            return Err(
                "--configと--seedは併用できません。設定ファイルのgame.seedを使用してください。"
                    .into(),
            );
        }
        if options.log_format.is_some() && options.log_path.is_none() {
            return Err("--log-formatには--logの保存先が必要です。".into());
        }
        if options.log_path.is_some() && matches!(options.mode, Mode::Simulate(2..)) {
            return Err(
                "単一の--logと複数試合は併用できません。試合ごとに保存先を分けて実行してください。"
                    .into(),
            );
        }
        if options.config_path.is_none() {
            validate_seed_range(&options.mode, options.seed.unwrap_or(0))?;
        }
        Ok(options)
    }

    fn load_config(&self) -> Result<AppConfig, String> {
        let mut config = if let Some(path) = &self.config_path {
            let file =
                File::open(path).map_err(|error| format!("設定ファイルを開けません: {error}"))?;
            // Read at most one byte beyond the common API bound, before decoding UTF-8.
            let mut bytes = Vec::new();
            file.take((suzume_api::MAX_REQUEST_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("設定ファイルを読み取れません: {error}"))?;
            if bytes.len() > suzume_api::MAX_REQUEST_BYTES {
                return Err("設定ファイルは1MiB以下にしてください。".into());
            }
            let text = String::from_utf8(bytes)
                .map_err(|error| format!("設定ファイルはUTF-8で保存してください: {error}"))?;
            AppConfig::from_toml(&text).map_err(|error| format!("設定が不正です: {error}"))?
        } else {
            let mut config = AppConfig::default();
            config.game.seed = self.seed.unwrap_or(0).into();
            if matches!(self.mode, Mode::Simulate(_) | Mode::Demo) {
                config.players = std::array::from_fn(|_| Controller::Ai {
                    config: AiConfigSpec::default(),
                });
            }
            config
        };
        if let Some(format) = self.log_format {
            config.logging.format = format;
        }
        config
            .validate()
            .map_err(|error| format!("設定が不正です: {error}"))?;
        validate_seed_range(&self.mode, config.game.seed.value())?;
        match self.mode {
            Mode::Play | Mode::Snapshot => {
                if config.game.viewer.seat().map(|seat| seat.get()) != Some(0)
                    || !matches!(config.players[0], Controller::Human)
                    || config.players[1..]
                        .iter()
                        .any(|player| matches!(player, Controller::Human))
                {
                    return Err(
                        "TUIと初期表示は人間の席0、viewer=0、他の3席AIに対応します。".into(),
                    );
                }
            }
            Mode::Simulate(_) | Mode::Demo => {
                if config
                    .players
                    .iter()
                    .any(|player| matches!(player, Controller::Human))
                {
                    return Err("--simulateと--demoの設定は全員AIにしてください。".into());
                }
                if self.mode == Mode::Demo
                    && config
                        .game
                        .viewer
                        .seat()
                        .is_some_and(|seat| seat.get() != 0)
                {
                    return Err(
                        "demoの表示は席0またはpublicに対応します。viewerを一致させてください。"
                            .into(),
                    );
                }
            }
            Mode::Help => {}
        }
        Ok(config)
    }
}

fn validate_seed_range(mode: &Mode, seed: u64) -> Result<(), String> {
    if let Mode::Simulate(count) = mode {
        let count = u64::try_from(*count).map_err(|_| "試合数が大きすぎます。")?;
        if seed.checked_add(count - 1).is_none() {
            return Err("試合数と乱数種の組合せが64ビット整数の範囲を超えます。".into());
        }
    }
    Ok(())
}

fn set_mode(current: &mut Mode, set: &mut bool, new: Mode) -> Result<(), String> {
    if *set {
        return Err("実行モードは一つだけ指定してください。".into());
    }
    *set = true;
    *current = new;
    Ok(())
}

/// A single game, the resolved policies actually used, and its optional stream.
/// This adapter keeps every successful operation and the log sequence together.
struct Session {
    game: Game,
    config: AppConfig,
    policies: [Option<ConfiguredAi>; 4],
    log: Option<LogWriter<io::BufWriter<File>>>,
}

impl Session {
    fn new(config: AppConfig, log_path: Option<&Path>) -> Result<Self, String> {
        config.validate().map_err(|error| error.to_string())?;
        let mut policies = [None; 4];
        for (seat, player) in config.players.iter().enumerate() {
            if let Controller::Ai { config } = player {
                policies[seat] = Some(
                    ConfiguredAi::new(config.resolve().map_err(|error| error.to_string())?)
                        .map_err(|error| error.to_string())?,
                );
            }
        }
        let game = Game::new(config.game.seed.value()).map_err(|error| error.to_string())?;
        let log = match log_path {
            Some(path) => {
                let file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(|error| format!("ログを新規作成できません: {error}"))?;
                Some(
                    LogWriter::new(io::BufWriter::new(file), &config, &game)
                        .map_err(|error| format!("ログを開始できません: {error}"))?,
                )
            }
            None => None,
        };
        Ok(Self {
            game,
            config,
            policies,
            log,
        })
    }

    fn action_count(&self) -> usize {
        self.game.history().len()
    }

    fn reached_limit(&self) -> bool {
        self.action_count() >= self.config.game.max_steps as usize
    }

    fn cpu_decision(&self, player: usize, legal: &[Action]) -> Result<Decision, String> {
        let policy = self
            .policies
            .get(player)
            .and_then(Option::as_ref)
            .ok_or("この席はAIとして設定されていません。")?;
        let observation = self
            .game
            .observation(player)
            .map_err(|error| error.to_string())?;
        let decision = policy
            .decide(&observation, legal)
            .map_err(|error| format!("AI判断に失敗しました: {error}"))?;
        if !legal.contains(&decision.action) {
            return Err("AIが合法手に含まれない操作を返しました。".into());
        }
        Ok(decision)
    }

    fn apply(
        &mut self,
        player: usize,
        action: Action,
        decision: Option<&Decision>,
    ) -> Result<(), String> {
        if self.reached_limit() {
            return Err(self.limit_error());
        }
        let view = if self.log.is_some() {
            decision
                .map(|decision| {
                    DecisionView::from_core(decision).ok_or("AI判断をログの型へ変換できません。")
                })
                .transpose()?
        } else {
            None
        };
        self.game.apply(player, action.clone()).map_err(|error| {
            format!(
                "乱数種{}、操作{}: {error}",
                self.game.seed(),
                self.action_count()
            )
        })?;
        if let Some(log) = &mut self.log {
            log.record(player, &action, &self.game, view.as_ref())
                .map_err(|error| format!("ログの記録に失敗しました: {error}"))?;
        }
        Ok(())
    }

    fn limit_error(&self) -> String {
        format!(
            "乱数種{}: {}操作の上限に到達しました。通常の終局ではありません。",
            self.game.seed(),
            self.config.game.max_steps
        )
    }

    /// Also closes a valid partial log on q, EOF, demo completion or a caller error.
    fn finish(&mut self) -> Result<(), String> {
        let reason = if matches!(self.game.phase(), Phase::MatchEnd) {
            EndReason::Completed
        } else if self.reached_limit() {
            EndReason::Truncated
        } else {
            EndReason::Interrupted
        };
        if let Some(log) = self.log.take() {
            log.finish(&self.game, reason)
                .map_err(|error| format!("ログを完結できません: {error}"))?;
        }
        Ok(())
    }
}

fn main() -> ExitCode {
    let options = match Options::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("入力エラー: {error}");
            return ExitCode::from(2);
        }
    };
    match run(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("実行エラー: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(options: Options) -> Result<(), String> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let terminal = stdin.is_terminal() && stdout.is_terminal();
    let color = terminal && !options.plain && std::env::var_os("NO_COLOR").is_none();
    let mut output = io::BufWriter::new(stdout.lock());
    if options.mode == Mode::Help {
        output.write_all(HELP.as_bytes()).map_err(output_error)?;
        return output.flush().map_err(output_error);
    }
    let config = options.load_config()?;
    match options.mode {
        Mode::Simulate(count) => simulate(count, &config, options.log_path.as_deref(), &mut output),
        _ => {
            let mut session = Session::new(config, options.log_path.as_deref())?;
            let result = match options.mode {
                Mode::Snapshot => show(&session, color, false, &mut output),
                Mode::Play | Mode::Demo => play(
                    &mut session,
                    options.mode == Mode::Demo,
                    color,
                    &mut stdin.lock(),
                    &mut output,
                ),
                Mode::Help | Mode::Simulate(_) => unreachable!("handled above"),
            };
            combine_results(result, session.finish())
        }
    }
}

fn combine_results(result: Result<(), String>, closing: Result<(), String>) -> Result<(), String> {
    match (result, closing) {
        (Err(error), Err(closing)) => Err(format!("{error}\n{closing}")),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn output_error(error: io::Error) -> String {
    format!("出力に失敗しました: {error}")
}

fn show(
    session: &Session,
    color: bool,
    automatic: bool,
    output: &mut impl Write,
) -> Result<(), String> {
    if color {
        output.write_all(b"\x1b[2J\x1b[H").map_err(output_error)?;
    }
    // Public viewers do not receive a seed capable of reconstructing all hands.
    let reveal_hand = session.config.game.viewer.seat().is_some();
    if reveal_hand {
        writeln!(output, "乱数種: {}", session.game.seed()).map_err(output_error)?;
    }
    output
        .write_all(ui::board(&session.game, color, automatic, reveal_hand).as_bytes())
        .map_err(output_error)?;
    output.flush().map_err(output_error)
}

fn play(
    session: &mut Session,
    demo: bool,
    color: bool,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(), String> {
    if demo {
        show(session, color, true, output)?;
        writeln!(output, "全員CPUの一局を開始します。").map_err(output_error)?;
    }
    loop {
        if matches!(session.game.phase(), Phase::MatchEnd) {
            show(session, color, demo, output)?;
            return Ok(());
        }
        if session.reached_limit() {
            return Err(session.limit_error());
        }
        if demo && matches!(session.game.phase(), Phase::RoundEnd(_)) {
            show(session, color, true, output)?;
            return Ok(());
        }
        let player = session.game.next_actor().ok_or("次の手番がありません。")?;
        let legal = session.game.legal_actions(player);
        if legal.is_empty() {
            return Err(format!("席{player}に合法手がありません。"));
        }
        let (action, decision) = if session.policies[player].is_some() {
            let decision = session.cpu_decision(player, &legal)?;
            (decision.action.clone(), Some(decision))
        } else if legal == [Action::Pass] {
            (Action::Pass, None)
        } else {
            show(session, color, false, output)?;
            let action = loop {
                write!(output, "入力 > ").map_err(output_error)?;
                output.flush().map_err(output_error)?;
                let mut text = String::new();
                if input
                    .read_line(&mut text)
                    .map_err(|error| format!("入力を読み取れません: {error}"))?
                    == 0
                {
                    writeln!(output, "\n入力が閉じられたため、対局を終了しました。")
                        .map_err(output_error)?;
                    output.flush().map_err(output_error)?;
                    return Ok(());
                }
                match ui::parse_input(&text, &legal) {
                    Ok(ui::Input::Quit) => {
                        writeln!(output, "対局を終了しました。").map_err(output_error)?;
                        output.flush().map_err(output_error)?;
                        return Ok(());
                    }
                    Ok(ui::Input::Help) => output
                        .write_all(ui::help().as_bytes())
                        .map_err(output_error)?,
                    Ok(ui::Input::Action(action)) => break action,
                    Err(error) => writeln!(output, "{error}").map_err(output_error)?,
                }
            };
            (action, None)
        };
        let visible = action != Action::Pass;
        let label = if demo && !color && session.config.game.viewer.seat().is_some() {
            Some(action.label(&session.game.players()[player].hand))
        } else {
            None
        };
        session.apply(player, action, decision.as_ref())?;
        if demo && !color {
            if let Some(label) = label {
                writeln!(output, "{}: {label}", session.game.players()[player].name)
                    .map_err(output_error)?;
            }
        }
        if demo && color && visible {
            show(session, color, true, output)?;
            std::thread::sleep(Duration::from_millis(90));
        }
    }
}

fn simulate(
    count: usize,
    config: &AppConfig,
    log_path: Option<&Path>,
    output: &mut impl Write,
) -> Result<(), String> {
    let seed = config.game.seed.value();
    writeln!(
        output,
        "自動対局: {count}試合、開始する乱数種{seed}、各試合の上限{}操作",
        config.game.max_steps
    )
    .map_err(output_error)?;
    let mut succeeded = 0;
    for offset in 0..count {
        let current_seed = seed
            .checked_add(offset as u64)
            .ok_or("乱数種が範囲を超えました。")?;
        let mut current_config = config.clone();
        current_config.game.seed = current_seed.into();
        match simulate_one(current_config, log_path) {
            Ok((game, actions)) => {
                succeeded += 1;
                writeln!(
                    output,
                    "{}/{}  seed={}  成功  {}操作  点数={:?}  供託={}点",
                    offset + 1,
                    count,
                    current_seed,
                    actions,
                    game.scores(),
                    game.riichi_pot()
                )
                .map_err(output_error)?;
            }
            Err(error) => {
                writeln!(
                    output,
                    "{}/{}  seed={}  失敗  {}",
                    offset + 1,
                    count,
                    current_seed,
                    error
                )
                .map_err(output_error)?;
            }
        }
        output.flush().map_err(output_error)?;
    }
    writeln!(
        output,
        "結果: 成功{succeeded} / 総数{count}、失敗{}",
        count - succeeded
    )
    .map_err(output_error)?;
    output.flush().map_err(output_error)?;
    if succeeded == count {
        Ok(())
    } else {
        Err(format!(
            "{}試合の検査が失敗しました。失敗を除外せず、上記の全結果を確認してください。",
            count - succeeded
        ))
    }
}

fn simulate_one(config: AppConfig, log_path: Option<&Path>) -> Result<(Game, usize), String> {
    let mut session = Session::new(config, log_path)?;
    let result = simulate_session(&mut session);
    combine_results(result, session.finish())?;
    let actions = session.action_count();
    Ok((session.game, actions))
}

fn simulate_session(session: &mut Session) -> Result<(), String> {
    loop {
        session
            .game
            .validate()
            .map_err(|error| format!("不変条件違反: {error}"))?;
        if matches!(session.game.phase(), Phase::MatchEnd) {
            return Ok(());
        }
        if session.reached_limit() {
            return Err(session.limit_error());
        }
        let player = session.game.next_actor().ok_or("次の手番がありません。")?;
        let legal = session.game.legal_actions(player);
        let decision = session.cpu_decision(player, &legal)?;
        session.apply(player, decision.action.clone(), Some(&decision))?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(args.iter().map(|value| (*value).to_owned()))
    }

    #[test]
    fn cli_accepts_each_documented_mode() {
        assert_eq!(parse(&[]).unwrap().mode, Mode::Play);
        assert_eq!(
            parse(&["--simulate", "32", "--seed", "0"]).unwrap().mode,
            Mode::Simulate(32)
        );
        assert_eq!(parse(&["--demo", "--plain"]).unwrap().mode, Mode::Demo);
        assert_eq!(parse(&["--snapshot", "--seed", "7"]).unwrap().seed, Some(7));
        assert_eq!(parse(&["--help"]).unwrap().mode, Mode::Help);
        assert_eq!(
            parse(&[
                "--config",
                "game.toml",
                "--log",
                "game.jsonl",
                "--log-format",
                "public"
            ])
            .unwrap()
            .log_format,
            Some(LogFormat::Public)
        );
    }

    #[test]
    fn cli_help_and_invalid_arguments() {
        for args in [
            vec!["--unknown"],
            vec!["--seed"],
            vec!["--seed", "-1"],
            vec!["--seed", "no"],
            vec!["--simulate", "0"],
            vec!["--simulate", "-4"],
            vec!["--simulate"],
            vec!["--simulate", "10001"],
            vec!["--demo", "--snapshot"],
            vec!["--seed", "1", "--seed", "2"],
            vec!["--config"],
            vec!["--log"],
            vec!["--log-format", "public"],
            vec!["--log", "a", "--log-format", "other"],
            vec!["--log", "a", "--simulate", "2"],
            vec!["--config", "a", "--seed", "0"],
            vec!["--seed", "0", "--config", "a"],
            vec!["--config", "a", "--config", "b"],
            vec!["--log", "a", "--log", "b"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
        assert!(parse(&["--simulate", "2", "--seed", "18446744073709551615"]).is_err());
        assert!(parse(&["--simulate", "1", "--seed", "18446744073709551615"]).is_ok());
    }
}

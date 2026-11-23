//! 実行入口。引数、端末入出力、自動対局を担当します。

mod ui;

use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use suzume::ai::{GreedyAi, Policy};
use suzume::game::{Action, Game, Phase};

const ACTION_LIMIT: usize = 10_000;
const SIMULATION_LIMIT: usize = 10_000;

const HELP: &str = "すずめ — Rust製の四人リーチ麻雀\n\n\
使用方法: suzume [--seed 整数] [--plain] [実行モード]\n\n\
実行モード（いずれか1つ）:\n\
  指定なし       人間1人とCPU3人で東風戦\n\
  --simulate N   種seedから連続N個の全員CPU対局（1〜10000試合）\n\
  --demo         全員CPUで一局を表示して終了\n\
  --snapshot     初期配牌の画面を表示して終了\n\
  --help, -h     この説明を表示\n\n\
共通の指定:\n\
  --seed N       0〜18446744073709551615の乱数種\n\
                 自動対局の省略値は0、それ以外は現在時刻から生成\n\
  --plain        色・画面消去を使わず、制御文字のない行入力で表示\n\n\
対局中: 牌番号で打牌、r 番号でリーチ、/番号で鳴き等を選択。hで操作説明。\n\
標準入出力が端末ではない場合とNO_COLOR設定時は色・画面消去を使いません。\n\
自動対局は1試合10000操作を上限とし、失敗・操作上限到達を成功に含めません。\n";

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
    plain: bool,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut args = args.into_iter();
        let mut options = Self {
            mode: Mode::Play,
            seed: None,
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
                        value
                            .parse::<u64>()
                            .map_err(|_| "--seedは0以上の64ビット整数で指定してください。")?,
                    );
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
        if let Mode::Simulate(count) = options.mode {
            let count = u64::try_from(count).map_err(|_| "試合数が大きすぎます。")?;
            if options.seed.unwrap_or(0).checked_add(count - 1).is_none() {
                return Err("試合数と乱数種の組合せが64ビット整数の範囲を超えます。".into());
            }
        }
        Ok(options)
    }

    fn effective_seed(&self) -> Result<u64, String> {
        if let Some(seed) = self.seed {
            return Ok(seed);
        }
        if matches!(self.mode, Mode::Simulate(_)) {
            return Ok(0);
        }
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                format!("現在時刻を取得できません。--seedを指定してください: {error}")
            })?;
        Ok(time.as_secs() ^ u64::from(time.subsec_nanos()).rotate_left(32))
    }
}

fn set_mode(current: &mut Mode, set: &mut bool, new: Mode) -> Result<(), String> {
    if *set {
        return Err("実行モードは一つだけ指定してください。".into());
    }
    *set = true;
    *current = new;
    Ok(())
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
    let seed = options.effective_seed()?;
    match options.mode {
        Mode::Simulate(count) => simulate(count, seed, &mut output),
        Mode::Snapshot => {
            let game = Game::new(seed).map_err(|error| error.to_string())?;
            writeln!(output, "乱数種: {seed}").map_err(output_error)?;
            output
                .write_all(ui::board(&game, color, false).as_bytes())
                .map_err(output_error)?;
            output.flush().map_err(output_error)
        }
        Mode::Play | Mode::Demo => {
            let mut input = stdin.lock();
            play(
                seed,
                options.mode == Mode::Demo,
                color,
                &mut input,
                &mut output,
            )
        }
        Mode::Help => Ok(()),
    }
}

fn output_error(error: io::Error) -> String {
    format!("出力に失敗しました: {error}")
}

fn show(game: &Game, color: bool, automatic: bool, output: &mut impl Write) -> Result<(), String> {
    if color {
        output.write_all(b"\x1b[2J\x1b[H").map_err(output_error)?;
    }
    writeln!(output, "乱数種: {}", game.seed()).map_err(output_error)?;
    output
        .write_all(ui::board(game, color, automatic).as_bytes())
        .map_err(output_error)?;
    output.flush().map_err(output_error)
}

fn choose_cpu(game: &Game, player: usize, legal: &[Action]) -> Result<Action, String> {
    let observation = game
        .observation(player)
        .map_err(|error| error.to_string())?;
    let action = GreedyAi
        .choose_action(&observation, legal)
        .ok_or("CPUが合法手を選択できませんでした。")?;
    if !legal.contains(&action) {
        return Err("CPUが合法手に含まれない操作を返しました。".into());
    }
    Ok(action)
}

fn play(
    seed: u64,
    demo: bool,
    color: bool,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(), String> {
    let mut game = Game::new(seed).map_err(|error| error.to_string())?;
    let mut actions = 0;
    if demo {
        show(&game, color, true, output)?;
        writeln!(output, "全員CPUの一局を開始します。").map_err(output_error)?;
    }
    loop {
        if matches!(game.phase(), Phase::MatchEnd)
            || (demo && matches!(game.phase(), Phase::RoundEnd(_)))
        {
            show(&game, color, demo, output)?;
            return Ok(());
        }
        if actions >= ACTION_LIMIT {
            return Err(format!(
                "乱数種{seed}: {ACTION_LIMIT}操作以内に対局が終了しませんでした。"
            ));
        }
        let player = game.next_actor().ok_or("次の手番がありません。")?;
        let legal = game.legal_actions(player);
        if legal.is_empty() {
            return Err(format!("乱数種{seed}: 席{player}に合法手がありません。"));
        }
        let action = if legal == [Action::Pass] {
            Action::Pass
        } else if player != 0 || demo {
            let action = choose_cpu(&game, player, &legal)?;
            if demo && !color {
                writeln!(
                    output,
                    "{}: {}",
                    game.players()[player].name,
                    action.label(&game.players()[player].hand)
                )
                .map_err(output_error)?;
            }
            action
        } else {
            show(&game, color, false, output)?;
            loop {
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
            }
        };
        let visible = action != Action::Pass;
        game.apply(player, action)
            .map_err(|error| format!("乱数種{seed}、操作{actions}: {error}"))?;
        actions += 1;
        if demo && color && visible {
            show(&game, color, true, output)?;
            std::thread::sleep(Duration::from_millis(90));
        }
    }
}

fn simulate(count: usize, seed: u64, output: &mut impl Write) -> Result<(), String> {
    writeln!(
        output,
        "自動対局: {count}試合、開始する乱数種{seed}、各試合の上限{ACTION_LIMIT}操作"
    )
    .map_err(output_error)?;
    let mut succeeded = 0;
    for offset in 0..count {
        let current_seed = seed
            .checked_add(offset as u64)
            .ok_or("乱数種が範囲を超えました。")?;
        match simulate_one(current_seed) {
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

fn simulate_one(seed: u64) -> Result<(Game, usize), String> {
    let mut game = Game::new(seed).map_err(|error| error.to_string())?;
    for actions in 0..=ACTION_LIMIT {
        game.validate()
            .map_err(|error| format!("不変条件違反: {error}"))?;
        if matches!(game.phase(), Phase::MatchEnd) {
            return Ok((game, actions));
        }
        if actions == ACTION_LIMIT {
            break;
        }
        let player = game.next_actor().ok_or("次の手番がありません。")?;
        let legal = game.legal_actions(player);
        let action = choose_cpu(&game, player, &legal)?;
        game.apply(player, action)
            .map_err(|error| error.to_string())?;
    }
    Err(format!("{ACTION_LIMIT}操作の上限に到達しました。"))
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
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
        assert!(parse(&["--simulate", "2", "--seed", "18446744073709551615"]).is_err());
        assert!(parse(&["--simulate", "1", "--seed", "18446744073709551615"]).is_ok());
    }
}

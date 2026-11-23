//! 事前固定した対照評価と、別の種で行う開発用測定の実行入口です。
use std::fs::{self, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;
use suzume::ai::{AiAlgorithm, AiConfig, AiPreset, ConfiguredAi};
use suzume::game::{Game, Phase, RoundOutcome};

const ACTION_LIMIT: u32 = 10_000;
const CANDIDATE: AiConfig = AiConfig {
    algorithm: AiAlgorithm::Ukeire,
    seed: 20261123,
    max_evaluations: 8192,
    lookahead_depth: 2,
    risk_weight: 4,
};
const RANDOM: AiConfig = AiPreset::Beginner.resolve(20261123);
const OPPONENT: AiConfig = AiPreset::Standard.resolve(0);
const SOURCE: &str = concat!(
    include_str!("../ai.rs"),
    include_str!("../ai/config.rs"),
    include_str!("../ai/search.rs"),
    include_str!("../ai/validation.rs"),
    include_str!("../game.rs"),
    include_str!("../hand.rs"),
    include_str!("../tile.rs"),
    include_str!("../scoring.rs"),
    include_str!("../contracts.rs"),
    include_str!("../generated_contracts.rs"),
    include_str!("../rule_constants.rs"),
    include_str!("suzume-ai-eval.rs")
);
const HEADER: &str = "group,wall_seed,seat,algorithm,ai_seed,max_evaluations,lookahead_depth,risk_weight,success,rank,points,wins,dealins,actions,candidate_decisions,candidate_evaluations,elapsed_ms,error\n";
const HELP: &str = "すずめ AI評価\n\n\
  suzume-ai-eval --print-protocol\n\
  suzume-ai-eval --holdout --output 結果.csv [--protocol evaluations/ai-protocol.json]\n\
  suzume-ai-eval --development [--seed 0] [--slot 0] [--algorithm ukeire]\n\
                [--budget 8192] [--depth 2] [--risk 4] [--ai-seed 20261123] [--output 結果.csv]\n\n\
固定評価は種1000〜1007×4席を、候補とRandom対照で各32試合実行します。\n\
開発用には固定評価の種を受け付けません。出力ファイルは上書きしません。\n\
--print-protocolの出力を評価前に固定し、--holdoutはその内容とソース識別値を照合します。\n";

#[derive(Default)]
struct Options {
    mode: Option<String>,
    output: Option<String>,
    protocol: Option<String>,
    seed: Option<u64>,
    slot: Option<usize>,
    algorithm: Option<AiAlgorithm>,
    ai_seed: Option<u64>,
    budget: Option<u32>,
    depth: Option<u8>,
    risk: Option<u16>,
}
impl Options {
    fn parse() -> Result<Self, String> {
        let mut result = Self::default();
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            if matches!(
                arg.as_str(),
                "--help" | "-h" | "--development" | "--holdout" | "--print-protocol"
            ) {
                if result.mode.replace(arg).is_some() {
                    return Err("実行モードの重複です".into());
                }
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| format!("{arg}の値がありません"))?;
            macro_rules! set {
                ($field:ident,$value:expr) => {
                    if result.$field.replace($value).is_some() {
                        return Err(format!("{arg}が重複しています"));
                    }
                };
            }
            match arg.as_str() {
                "--output" => set!(output, value),
                "--protocol" => set!(protocol, value),
                "--seed" => set!(seed, value.parse().map_err(|_| "seedはu64です")?),
                "--slot" => set!(slot, value.parse().map_err(|_| "slotは0〜3です")?),
                "--algorithm" => set!(algorithm, value.parse().map_err(|_| "未知の算法です")?),
                "--ai-seed" => set!(ai_seed, value.parse().map_err(|_| "AI seedはu64です")?),
                "--budget" => set!(budget, value.parse().map_err(|_| "budgetはu32です")?),
                "--depth" => set!(depth, value.parse().map_err(|_| "depthはu8です")?),
                "--risk" => set!(risk, value.parse().map_err(|_| "riskはu16です")?),
                _ => return Err(format!("未知の引数です: {arg}")),
            }
        }
        Ok(result)
    }
    fn custom(&self) -> bool {
        self.seed.is_some()
            || self.slot.is_some()
            || self.algorithm.is_some()
            || self.ai_seed.is_some()
            || self.budget.is_some()
            || self.depth.is_some()
            || self.risk.is_some()
    }
    fn config(&self) -> Result<AiConfig, String> {
        let seed = self.ai_seed.unwrap_or(CANDIDATE.seed);
        let mut config = match self.algorithm.unwrap_or(AiAlgorithm::Ukeire) {
            AiAlgorithm::Random => AiPreset::Beginner.resolve(seed),
            AiAlgorithm::Greedy => AiPreset::Standard.resolve(seed),
            AiAlgorithm::Ukeire => AiPreset::Strong.resolve(seed),
        };
        if let Some(value) = self.budget {
            config.max_evaluations = value;
        }
        if let Some(value) = self.depth {
            config.lookahead_depth = value;
        }
        if let Some(value) = self.risk {
            config.risk_weight = value;
        }
        config.validate().map_err(|error| error.to_string())?;
        Ok(config)
    }
}

fn main() -> ExitCode {
    match Options::parse().and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("AI評価エラー: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(options: Options) -> Result<(), String> {
    match options.mode.as_deref().unwrap_or("--help") {
        "--help" | "-h" => {
            print!("{HELP}");
            return Ok(());
        }
        "--print-protocol" => {
            if options.custom() || options.output.is_some() || options.protocol.is_some() {
                return Err("protocol表示に追加設定は使えません".into());
            }
            print!("{}", protocol());
            return Ok(());
        }
        "--holdout" => {
            if options.custom() {
                return Err("固定評価では設定の上書きを拒否します".into());
            }
            if options.output.is_none() {
                return Err("固定評価の--outputを指定してください".into());
            }
            let path = options
                .protocol
                .as_deref()
                .unwrap_or("evaluations/ai-protocol.json");
            let declared = fs::read_to_string(path)
                .map_err(|error| format!("評価計画を読めません: {error}"))?;
            if declared != protocol() {
                return Err("事前の評価計画と実行コードの設定・識別値が一致しません".into());
            }
        }
        "--development" => {
            if options.protocol.is_some() {
                return Err("開発用測定では固定評価計画を指定しません".into());
            }
            if (1000..=1007).contains(&options.seed.unwrap_or(0)) {
                return Err("固定評価用の種を開発用に使用できません".into());
            }
            if options.slot.unwrap_or(0) > 3 {
                return Err("slotは0〜3です".into());
            }
        }
        _ => return Err("未知の実行モードです".into()),
    }
    let mut output: Box<dyn Write> = if let Some(path) = &options.output {
        if let Some(parent) = Path::new(path)
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        Box::new(BufWriter::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|error| format!("新しい出力ファイルを作れません: {error}"))?,
        ))
    } else {
        Box::new(io::BufWriter::new(io::stdout().lock()))
    };
    output
        .write_all(HEADER.as_bytes())
        .map_err(|error| error.to_string())?;
    let mut failures = 0;
    if options.mode.as_deref() == Some("--holdout") {
        for (group, config) in [("candidate", CANDIDATE), ("reference_random", RANDOM)] {
            for seed in 1000..=1007 {
                for seat in 0..4 {
                    let result = run_match(seed, seat, config);
                    failures += usize::from(!result.success);
                    write_row(&mut output, group, seed, seat, config, &result)
                        .map_err(|error| error.to_string())?;
                    eprintln!(
                        "{group} seed={seed} seat={seat}: success={} rank={:?} points={:?} {}ms",
                        result.success, result.rank, result.points, result.elapsed_ms
                    );
                }
            }
        }
    } else {
        let config = options.config()?;
        let seed = options.seed.unwrap_or(0);
        let seat = options.slot.unwrap_or(0);
        let result = run_match(seed, seat, config);
        failures += usize::from(!result.success);
        write_row(&mut output, "development", seed, seat, config, &result)
            .map_err(|error| error.to_string())?;
    }
    if failures == 0 {
        Ok(())
    } else {
        Err(format!(
            "{failures}試合が失敗しました。全行を結果に残しています"
        ))
    }
}

#[derive(Default)]
struct MatchResult {
    success: bool,
    rank: Option<usize>,
    points: Option<i32>,
    wins: u32,
    dealins: u32,
    actions: u32,
    decisions: u32,
    evaluations: u64,
    elapsed_ms: u128,
    error: Option<String>,
}
fn run_match(seed: u64, seat: usize, config: AiConfig) -> MatchResult {
    let start = Instant::now();
    let mut result = MatchResult::default();
    let mut game = match Game::new(seed) {
        Ok(game) => game,
        Err(error) => {
            result.error = Some(error.to_string());
            return result;
        }
    };
    let candidate = match ConfiguredAi::new(config) {
        Ok(ai) => ai,
        Err(error) => {
            result.error = Some(error.to_string());
            return result;
        }
    };
    let opponent = ConfiguredAi::new(OPPONENT).expect("固定の対照設定は有効です");
    while !matches!(game.phase(), Phase::MatchEnd) {
        if result.actions == ACTION_LIMIT {
            result.error = Some("10000操作の上限に到達しました".into());
            break;
        }
        let Some(actor) = game.next_actor() else {
            result.error = Some("次の手番がありません".into());
            break;
        };
        let view = match game.observation(actor) {
            Ok(view) => view,
            Err(error) => {
                result.error = Some(error.to_string());
                break;
            }
        };
        let legal = game.legal_actions(actor);
        let decision =
            match if actor == seat { &candidate } else { &opponent }.decide(&view, &legal) {
                Ok(decision) => decision,
                Err(error) => {
                    result.error = Some(error.to_string());
                    break;
                }
            };
        if actor == seat {
            result.decisions += 1;
            result.evaluations += u64::from(decision.evaluations);
        }
        if let Err(error) = game.apply(actor, decision.action) {
            result.error = Some(error.to_string());
            break;
        }
        result.actions += 1;
        if let Phase::RoundEnd(round) = game.phase() {
            if let RoundOutcome::Win { winner, from, .. } = round.outcome {
                result.wins += u32::from(winner == seat);
                result.dealins += u32::from(from == Some(seat));
            }
        }
    }
    result.success = matches!(game.phase(), Phase::MatchEnd) && result.error.is_none();
    if result.success {
        result.rank = game
            .ranking()
            .iter()
            .position(|&player| player == seat)
            .map(|rank| rank + 1);
    }
    result.points = Some(game.scores()[seat]);
    result.elapsed_ms = start.elapsed().as_millis();
    result
}

fn write_row(
    output: &mut dyn Write,
    group: &str,
    seed: u64,
    seat: usize,
    config: AiConfig,
    result: &MatchResult,
) -> io::Result<()> {
    let error = result
        .error
        .as_deref()
        .unwrap_or("")
        .replace('"', "\"\"")
        .replace(['\n', '\r'], " ");
    writeln!(
        output,
        "{group},{seed},{seat},{},{},{},{},{},{},{},{},{},{},{},{},{},{},\"{error}\"",
        config.algorithm.as_str(),
        config.seed,
        config.max_evaluations,
        config.lookahead_depth,
        config.risk_weight,
        result.success,
        result
            .rank
            .map(|value| value.to_string())
            .unwrap_or_default(),
        result
            .points
            .map(|value| value.to_string())
            .unwrap_or_default(),
        result.wins,
        result.dealins,
        result.actions,
        result.decisions,
        result.evaluations,
        result.elapsed_ms
    )?;
    output.flush()
}

fn config_json(config: AiConfig) -> String {
    format!("{{\"algorithm\":\"{}\",\"seed\":\"{}\",\"max_evaluations\":{},\"lookahead_depth\":{},\"risk_weight\":{}}}",config.algorithm.as_str(),config.seed,config.max_evaluations,config.lookahead_depth,config.risk_weight)
}
fn protocol() -> String {
    let mut fingerprint = 0xcbf29ce484222325_u64;
    for byte in SOURCE.bytes() {
        fingerprint ^= u64::from(byte);
        fingerprint = fingerprint.wrapping_mul(0x100000001b3);
    }
    format!("{{\n  \"schema_version\": 1,\n  \"protocol_id\": \"suzume-ai-paired-seats-v1\",\n  \"engine_version\": \"{}\",\n  \"source_fnv1a64\": \"{fingerprint:016x}\",\n  \"source_fingerprint_is_cryptographic\": false,\n  \"wall_seeds\": [1000,1001,1002,1003,1004,1005,1006,1007],\n  \"candidate_seats\": [0,1,2,3],\n  \"games_per_group\": 32,\n  \"total_games\": 64,\n  \"action_limit\": {ACTION_LIMIT},\n  \"candidate\": {},\n  \"reference\": {},\n  \"each_opponent\": {},\n  \"acceptance\": \"all scheduled games reported, no failed games excluded; superiority is not a completion requirement\",\n  \"metrics\": [\"rank\",\"points\",\"wins\",\"dealins\",\"actions\",\"candidate_decisions\",\"candidate_evaluations\",\"elapsed_ms\",\"success\",\"error\"],\n  \"budget_unit\": \"uncached shanten calls per decision\",\n  \"paired_seed_limitation\": \"different policies change draws and calls, so trajectories are not identical\"\n}}\n",env!("CARGO_PKG_VERSION"),config_json(CANDIDATE),config_json(RANDOM),config_json(OPPONENT))
}

//! 端末の表示と入力の解釈。対局の規則と状態変更はGameへ委譲します。

use suzume::game::{Action, Game, Phase, RoundOutcome};
use suzume::scoring::{Meld, MeldKind};
use suzume::tile::Tile;

const CONTENT_WIDTH: usize = 88;

#[derive(Debug, Eq, PartialEq)]
pub enum Input {
    Action(Action),
    Help,
    Quit,
}

/// 入力を合法手と照合します。数字は手牌番号、`/数字`は補助操作の番号です。
pub fn parse_input(text: &str, legal: &[Action]) -> Result<Input, String> {
    let text = text.trim();
    let lower = text.to_ascii_lowercase();
    if matches!(lower.as_str(), "q" | "quit" | "終了") {
        return Ok(Input::Quit);
    }
    if matches!(lower.as_str(), "h" | "help" | "?" | "ヘルプ") {
        return Ok(Input::Help);
    }
    let action = if text.is_empty() {
        if legal.contains(&Action::NextRound) {
            Action::NextRound
        } else if legal.contains(&Action::Pass) {
            Action::Pass
        } else {
            return Err("捨てる牌の番号を入力してください。hで操作を確認できます。".into());
        }
    } else if let Some(number) = text.strip_prefix('/') {
        let index = positive_index(number)?;
        special_actions(legal)
            .get(index)
            .ok_or_else(|| "操作番号が範囲外です。画面の /番号 を確認してください。".to_owned())?
            .clone()
    } else if text.chars().all(|c| c.is_ascii_digit()) {
        Action::Discard(positive_index(text)?)
    } else {
        let words: Vec<&str> = lower.split_whitespace().collect();
        match words.as_slice() {
            ["r" | "リーチ", number] => Action::Riichi(positive_index(number)?),
            ["d" | "打牌", number] => Action::Discard(positive_index(number)?),
            ["t" | "tsumo" | "ツモ"] => Action::Tsumo,
            ["ron" | "ロン"] => Action::Ron,
            ["p" | "pon" | "ポン"] => Action::Pon,
            ["s" | "pass" | "見送り"] => Action::Pass,
            ["next" | "次局"] => Action::NextRound,
            ["c" | "chi" | "チー"] => unique_call(legal, true)?,
            ["k" | "kan" | "カン"] => unique_call(legal, false)?,
            _ => {
                return Err(
                    "入力を解釈できません。数字、r 数字、/操作番号、h、qを使用してください。"
                        .into(),
                )
            }
        }
    };
    if legal.contains(&action) {
        Ok(Input::Action(action))
    } else {
        Err(
            "その操作は現在選べません。手番・手牌番号と、画面の可能な操作を確認してください。"
                .into(),
        )
    }
}

fn positive_index(text: &str) -> Result<usize, String> {
    text.trim()
        .parse::<usize>()
        .ok()
        .and_then(|number| number.checked_sub(1))
        .ok_or_else(|| "番号は1以上の整数で指定してください。".into())
}

fn unique_call(legal: &[Action], chi: bool) -> Result<Action, String> {
    let candidates: Vec<&Action> = legal
        .iter()
        .filter(|action| {
            if chi {
                matches!(action, Action::Chi(_))
            } else {
                matches!(action, Action::OpenKan | Action::ClosedKan(_))
            }
        })
        .collect();
    match candidates.as_slice() {
        [action] => Ok((*action).clone()),
        [] => Err("現在その鳴きはできません。".into()),
        _ => Err("候補が複数あります。画面に表示された /操作番号 で選んでください。".into()),
    }
}

fn special_actions(legal: &[Action]) -> Vec<Action> {
    legal
        .iter()
        .filter(|action| !matches!(action, Action::Discard(_) | Action::Riichi(_)))
        .cloned()
        .collect()
}

pub fn help() -> &'static str {
    "操作方法\n\
     ・手牌の下の番号を入力すると、その牌を捨てます。\n\
     ・r 3 : 3番の牌を捨ててリーチします。リーチ可能な番号だけ選べます。\n\
     ・t : ツモ和了、ron : ロン、p : ポン、c : チー、k : カン。\n\
     ・候補が複数あるチー・カンは、画面の /1、/2 などで選びます。\n\
     ・s または空行 : 見送り。自分の打牌中は空行で牌を捨てません。\n\
     ・局の結果画面でEnter : 次局、h : この説明、q : 終了。\n\
     ・萬子は赤、筒子は青、索子は緑、字牌は紫です。\n\
     ・河のm/p/sは萬子/筒子/索子、zの1〜7は東南西北白發中を表します。\n\
     ・河の * はリーチ宣言牌、x は他家に鳴かれた牌です。\n\
     ・フリテン・役なしではロンできません。リーチ後の暗槓は採用していません。\n"
}

/// 対戦相手の伏せた手牌は参照せず、枚数だけを描画します。
pub fn board(game: &Game, color: bool, automatic: bool) -> String {
    let mut lines = Vec::new();
    lines.push(paint("すずめ  /  四人リーチ麻雀", "1;36", color));
    lines.push(if automatic {
        "東風戦・赤牌なし・喰いタンあり・頭ハネ   自動対局はCtrl+Cで終了".into()
    } else {
        "東風戦・赤牌なし・喰いタンあり・頭ハネ   h:操作説明  q:終了".into()
    });
    lines.push(String::new());
    let round = if matches!(game.phase(), Phase::MatchEnd) {
        "対局終了".to_owned()
    } else {
        format!("東{}局", game.round() + 1)
    };
    lines.push(format!(
        "{}  {}本場  供託 {}点  残り {}枚  親: {}",
        round,
        game.honba(),
        game.riichi_pot(),
        game.wall_remaining(),
        game.players()[game.dealer()].name
    ));
    let indicators = game.dora_indicators();
    lines.push(format!(
        "ドラ表示: {}  → ドラ: {}",
        tile_row(&indicators, color),
        tile_row(
            &indicators
                .iter()
                .map(|tile| tile.dora())
                .collect::<Vec<_>>(),
            color
        )
    ));
    separator(&mut lines);

    for seat in [2, 3, 1] {
        let player = &game.players()[seat];
        let turn = if game.next_actor() == Some(seat) {
            " 〈手番〉"
        } else {
            ""
        };
        let riichi = if player.riichi { "  リーチ" } else { "" };
        lines.push(paint(
            &format!(
                "{}家  {}  {:>6}点  伏せ牌{}枚{}{}",
                game.seat_wind(seat).japanese(),
                player.name,
                game.scores()[seat],
                player.hand.len(),
                riichi,
                turn
            ),
            "1",
            color,
        ));
        append_river(&mut lines, game, seat, color);
        append_melds(&mut lines, &player.melds, color);
        lines.push(String::new());
    }

    separator(&mut lines);
    let player = &game.players()[0];
    let who = if automatic {
        "あなた（自動対局）"
    } else {
        "あなた"
    };
    let status = if player.riichi {
        "リーチ中"
    } else if player.is_closed() {
        "門前"
    } else {
        "鳴きあり"
    };
    lines.push(paint(
        &format!(
            "{}家  {}  {:>6}点  {}{}",
            game.seat_wind(0).japanese(),
            who,
            game.scores()[0],
            status,
            if game.is_furiten(0) {
                "  フリテン"
            } else {
                ""
            }
        ),
        "1;33",
        color,
    ));
    lines.push(tile_row(&player.hand, color));
    lines.push(
        (1..=player.hand.len())
            .map(|index| format!(" {:>2}   ", index))
            .collect::<String>(),
    );
    if let Some((0, tile, rinshan)) = game.last_draw() {
        if matches!(game.phase(), Phase::Discard { player: 0 }) {
            lines.push(format!(
                "今回のツモ: {}{}",
                tile_text(tile, color),
                if rinshan { "（嶺上牌）" } else { "" }
            ));
        }
    }
    append_melds(&mut lines, &player.melds, color);
    append_river(&mut lines, game, 0, color);
    separator(&mut lines);

    match game.phase() {
        Phase::RoundEnd(result) => {
            match &result.outcome {
                RoundOutcome::Win {
                    winner,
                    from,
                    score,
                    winning_hand,
                } => {
                    let method = from.map_or_else(
                        || "ツモ".to_owned(),
                        |seat| format!("{}からロン", game.players()[seat].name),
                    );
                    lines.push(paint(
                        &format!("{}の和了 — {}", game.players()[*winner].name, method),
                        "1;33",
                        color,
                    ));
                    lines.push("和了手:".into());
                    lines.push(tile_row(winning_hand, color));
                    let value = if score.yakuman > 0 {
                        format!("{}倍役満", score.yakuman)
                    } else {
                        format!("{}翻 {}符", score.han, score.fu)
                    };
                    lines.push(format!(
                        "{}{}  ドラ {}枚",
                        value,
                        score
                            .limit_name
                            .map_or(String::new(), |name| format!("  {name}")),
                        score.dora
                    ));
                    let yaku: Vec<String> = score
                        .yaku
                        .iter()
                        .map(|yaku| {
                            if score.yakuman > 0 {
                                yaku.name.to_owned()
                            } else {
                                format!("{} {}翻", yaku.name, yaku.han)
                            }
                        })
                        .collect();
                    for chunk in yaku.chunks(4) {
                        lines.push(format!("役: {}", chunk.join(" / ")));
                    }
                }
                RoundOutcome::Draw { tenpai } => {
                    lines.push(paint("流局 — 牌山の通常ツモが終了しました", "1;33", color));
                    for (seat, ready) in tenpai.iter().enumerate() {
                        lines.push(format!(
                            "{}: {}",
                            game.players()[seat].name,
                            if *ready {
                                "テンパイ"
                            } else {
                                "ノーテン"
                            }
                        ));
                    }
                }
            }
            lines.push("点数移動（供託を含む）:".into());
            for (seat, delta) in result.deltas.iter().enumerate() {
                lines.push(format!("  {}  {:+}点", game.players()[seat].name, delta));
            }
            lines.push(if automatic {
                "一局の自動対局が終了しました。".into()
            } else if result.dealer_continues {
                "親が連荘します。Enterで次局へ進みます。".into()
            } else {
                "Enterで次局へ進みます。最終局の終了後は順位を表示します。".into()
            });
        }
        Phase::MatchEnd => {
            lines.push(paint("最終結果", "1;33", color));
            let scores = game.scores();
            let mut seats = [0_usize, 1, 2, 3];
            seats.sort_by_key(|&seat| (std::cmp::Reverse(scores[seat]), seat));
            for (rank, seat) in seats.iter().enumerate() {
                lines.push(format!(
                    "{}位  {}  {}点",
                    rank + 1,
                    game.players()[*seat].name,
                    scores[*seat]
                ));
            }
        }
        _ if automatic => lines.push("全員CPUで自動対局しています。".into()),
        _ => append_actions(&mut lines, game, color),
    }

    if !game.events().is_empty() {
        separator(&mut lines);
        for event in game
            .events()
            .iter()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            lines.push(format!("・{event}"));
        }
    }
    frame(&lines)
}

fn append_actions(lines: &mut Vec<String>, game: &Game, color: bool) {
    let legal = game.legal_actions(0);
    if let Phase::Reaction {
        discarder, tile, ..
    } = game.phase()
    {
        lines.push(format!(
            "{}の打牌: {}",
            game.players()[*discarder].name,
            tile_text(*tile, color)
        ));
    }
    if game.next_actor() != Some(0) {
        lines.push("CPUの手番です。".into());
        return;
    }
    let riichi: Vec<String> = legal
        .iter()
        .filter_map(|action| {
            if let Action::Riichi(index) = action {
                Some((index + 1).to_string())
            } else {
                None
            }
        })
        .collect();
    if !riichi.is_empty() {
        lines.push(format!(
            "リーチ可能な牌番号: {}  （r 番号）",
            riichi.join(", ")
        ));
    }
    let special: Vec<String> = special_actions(&legal)
        .iter()
        .enumerate()
        .map(|(index, action)| format!("/{} {}", index + 1, action.label(&game.players()[0].hand)))
        .collect();
    for chunk in special.chunks(3) {
        lines.push(chunk.join("   "));
    }
    lines.push(if matches!(game.phase(), Phase::Discard { .. }) {
        "数字: 打牌   r 数字: リーチ   /番号: 上記操作   h: 説明   q: 終了".into()
    } else {
        "/番号: 上記操作   Enterまたはs: 見送り   h: 説明   q: 終了".into()
    });
}

fn append_river(lines: &mut Vec<String>, game: &Game, seat: usize, color: bool) {
    let river = &game.players()[seat].discards;
    if river.is_empty() {
        lines.push("河: まだ捨て牌はありません".into());
        return;
    }
    for (index, chunk) in river.chunks(12).enumerate() {
        let mut row = if index == 0 {
            "河: ".to_owned()
        } else {
            "    ".to_owned()
        };
        for discard in chunk {
            let mark = if discard.riichi { "*" } else { "" };
            let called = if discard.called { "x" } else { "" };
            row.push_str(&paint(
                &format!("{}{}{} ", discard.tile.short(), mark, called),
                tile_color(discard.tile),
                color,
            ));
        }
        lines.push(row);
    }
}

fn append_melds(lines: &mut Vec<String>, melds: &[Meld], color: bool) {
    for chunk in melds.chunks(2) {
        lines.push(format!("副露: {}", melds_text(chunk, color)));
    }
}

fn melds_text(melds: &[Meld], color: bool) -> String {
    melds
        .iter()
        .map(|meld| {
            let name = match meld.kind {
                MeldKind::Chi => "チー",
                MeldKind::Pon => "ポン",
                MeldKind::OpenKan => "大明槓",
                MeldKind::ClosedKan => "暗槓",
            };
            format!("{} {}", name, tile_row(&meld.tiles, color))
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

fn tile_color(tile: Tile) -> &'static str {
    match tile.suit() {
        Some(0) => "31",
        Some(1) => "34",
        Some(2) => "32",
        _ => "35",
    }
}

fn tile_text(tile: Tile, color: bool) -> String {
    let label = match tile.suit() {
        Some(suit) => format!("{}{}", tile.rank(), ["萬", "筒", "索"][usize::from(suit)]),
        None => format!(" {}", tile.japanese()),
    };
    paint(
        &format!("[{label}]"),
        &format!("1;47;{}", tile_color(tile)),
        color,
    )
}

fn tile_row(tiles: &[Tile], color: bool) -> String {
    tiles
        .iter()
        .map(|tile| format!("{} ", tile_text(*tile, color)))
        .collect()
}

fn paint(text: &str, code: &str, color: bool) -> String {
    if color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

fn separator(lines: &mut Vec<String>) {
    lines.push("─".repeat(CONTENT_WIDTH));
}

fn frame(lines: &[String]) -> String {
    let mut out = format!("┌{}┐\n", "─".repeat(CONTENT_WIDTH + 2));
    for line in lines {
        let (line, width) = fit_line(line, CONTENT_WIDTH);
        out.push_str(&format!(
            "│ {}{} │\n",
            line,
            " ".repeat(CONTENT_WIDTH - width)
        ));
    }
    out.push_str(&format!("└{}┘\n", "─".repeat(CONTENT_WIDTH + 2)));
    out
}

/// 使用する日本語・罫線・ANSI色指定を対象とした表示幅処理です。
fn fit_line(text: &str, maximum: usize) -> (String, usize) {
    let mut out = String::new();
    let mut width = 0;
    let mut escape = false;
    for character in text.chars() {
        if character == '\x1b' {
            escape = true;
        }
        if escape {
            out.push(character);
            if character == 'm' {
                escape = false;
            }
            continue;
        }
        let cells = if matches!(character as u32, 0x1100..=0x115f | 0x2e80..=0xa4cf | 0xac00..=0xd7a3 | 0xf900..=0xfaff | 0xfe10..=0xfe6f | 0xff01..=0xff60 | 0xffe0..=0xffe6)
        {
            2
        } else {
            1
        };
        if width + cells > maximum {
            if text.contains('\x1b') {
                out.push_str("\x1b[0m");
            }
            break;
        }
        out.push(character);
        width += cells;
    }
    (out, width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_discards_and_riichi_are_distinct() {
        let legal = [Action::Discard(2), Action::Riichi(2), Action::Tsumo];
        assert_eq!(
            parse_input("3", &legal),
            Ok(Input::Action(Action::Discard(2)))
        );
        assert_eq!(
            parse_input("r 3", &legal),
            Ok(Input::Action(Action::Riichi(2)))
        );
        assert_eq!(parse_input("/1", &legal), Ok(Input::Action(Action::Tsumo)));
        assert!(parse_input("0", &legal).is_err());
        assert!(parse_input("4", &legal).is_err());
    }

    #[test]
    fn ambiguous_calls_require_an_explicit_menu_choice() {
        let first = Action::Chi([Tile::new(0).unwrap(), Tile::new(1).unwrap()]);
        let second = Action::Chi([Tile::new(3).unwrap(), Tile::new(4).unwrap()]);
        let legal = [Action::Pass, first.clone(), second.clone()];
        assert!(parse_input("chi", &legal).is_err());
        assert_eq!(parse_input("/2", &legal), Ok(Input::Action(first)));
        assert_eq!(parse_input("/3", &legal), Ok(Input::Action(second)));
        assert_eq!(parse_input("", &legal), Ok(Input::Action(Action::Pass)));
        assert_eq!(
            parse_input("", &[Action::NextRound]),
            Ok(Input::Action(Action::NextRound))
        );
    }

    #[test]
    fn invalid_input_does_not_become_a_discard() {
        let legal = [Action::Discard(0)];
        for text in ["", "-1", "1x", "/0", "184467440737095516160", "r 1", "ron"] {
            assert!(parse_input(text, &legal).is_err(), "{text}");
        }
        assert_eq!(parse_input("h", &legal), Ok(Input::Help));
        assert_eq!(parse_input("q", &legal), Ok(Input::Quit));
    }

    #[test]
    fn japanese_width_and_color_do_not_break_the_frame() {
        assert_eq!(fit_line("東1局", 8), ("東1局".to_owned(), 5));
        assert_eq!(fit_line("\x1b[31m東1局\x1b[0m", 8).1, 5);
        assert_eq!(fit_line("東南西", 4), ("東南".to_owned(), 4));
    }
}

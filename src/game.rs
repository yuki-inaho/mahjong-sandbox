//! 契約付き対局状態。入力・表示・CPUの戦略から独立しています。
//!
//! `apply` は候補状態へ操作を適用し、不変条件を検査してから確定します。
//! 呼出し側の不正な操作や内部エラーによって元の状態が変わることはありません。

use crate::contracts;
use crate::hand::winning_tiles;
use crate::rule_constants::*;
use crate::scoring::{evaluate, Meld, MeldKind, Score, WinContext};
use crate::tile::{counts, Tile};
use std::fmt;

/// 検証済みの席。構築後は0〜3の範囲だけを保持します。
///
/// 外部の整数を狭い型へ変換してから検査せず、元のusize全域を検査します。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Seat(u8);

impl Seat {
    pub const fn new(index: usize) -> Option<Self> {
        if index < PLAYER_COUNT {
            Some(Self(index as u8))
        } else {
            None
        }
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn id(self) -> u8 {
        self.0
    }
}

impl TryFrom<usize> for Seat {
    type Error = GameError;

    fn try_from(index: usize) -> std::result::Result<Self, Self::Error> {
        Self::new(index).ok_or_else(|| GameError("席が範囲外です".into()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameError(pub String);

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for GameError {}
impl From<contracts::ContractError> for GameError {
    fn from(error: contracts::ContractError) -> Self {
        Self(error.to_string())
    }
}
type Result<T> = std::result::Result<T, GameError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Discard {
    pub tile: Tile,
    pub called: bool,
    pub riichi: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Player {
    pub name: String,
    pub hand: Vec<Tile>,
    pub melds: Vec<Meld>,
    pub discards: Vec<Discard>,
    pub riichi: bool,
    pub double_riichi: bool,
    pub ippatsu: bool,
    pub temporary_furiten: bool,
    pub riichi_furiten: bool,
}

impl Player {
    fn new(name: String) -> Self {
        Self {
            name,
            hand: Vec::new(),
            melds: Vec::new(),
            discards: Vec::new(),
            riichi: false,
            double_riichi: false,
            ippatsu: false,
            temporary_furiten: false,
            riichi_furiten: false,
        }
    }

    pub fn is_closed(&self) -> bool {
        self.melds.iter().all(|meld| !meld.is_open())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Discard(usize),
    Riichi(usize),
    Tsumo,
    Ron,
    Pon,
    Chi([Tile; 2]),
    OpenKan,
    ClosedKan(Tile),
    Pass,
    NextRound,
}

impl Action {
    /// 表示用ラベルです。不正な手牌番号もpanicせず理由を表示します。
    /// 操作の合法性は別途 `Game::apply` が判定します。
    pub fn label(&self, hand: &[Tile]) -> String {
        match self {
            Self::Discard(index) | Self::Riichi(index) => {
                match hand.get(*index).zip(index.checked_add(1)) {
                    Some((tile, number)) if matches!(self, Self::Riichi(_)) => {
                        format!("{number}番 {tile}でリーチ")
                    }
                    Some((tile, number)) => format!("{number}番 {tile}を打牌"),
                    None => format!("不正な手牌番号: {index}"),
                }
            }
            Self::Tsumo => "ツモ和了".into(),
            Self::Ron => "ロン和了".into(),
            Self::Pon => "ポン".into(),
            Self::Chi(tiles) => format!("チー（{} {}）", tiles[0], tiles[1]),
            Self::OpenKan => "大明槓".into(),
            Self::ClosedKan(tile) => format!("暗槓（{tile}）"),
            Self::Pass => "見送り".into(),
            Self::NextRound => "次局へ".into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoundOutcome {
    Win {
        winner: usize,
        from: Option<usize>,
        score: Score,
        winning_hand: Vec<Tile>,
    },
    Draw {
        tenpai: [bool; 4],
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoundResult {
    pub outcome: RoundOutcome,
    pub deltas: [i32; 4],
    pub dealer_continues: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Phase {
    Discard {
        player: usize,
    },
    Reaction {
        discarder: usize,
        tile: Tile,
        responses: [Option<Action>; 4],
    },
    RoundEnd(RoundResult),
    MatchEnd,
}

/// CPUへ渡してよい公開情報。山の並びや他家の手牌を含みません。
#[derive(Clone, Debug)]
pub struct PublicPlayer {
    pub points: i32,
    pub melds: Vec<Meld>,
    pub discards: Vec<Discard>,
    pub riichi: bool,
    pub hand_len: usize,
}

#[derive(Clone, Debug)]
pub struct Observation {
    pub seat: usize,
    pub dealer: usize,
    pub hand: Vec<Tile>,
    pub melds: Vec<Meld>,
    pub players: [PublicPlayer; 4],
    pub dora_indicators: Vec<Tile>,
    pub wall_remaining: usize,
    pub last_discard: Option<(usize, Tile)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedAction {
    pub player: usize,
    pub action: Action,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Game {
    players: [Player; 4],
    accounts: [i32; 5],
    wall: Vec<Tile>,
    dead_wall: Vec<Tile>,
    phase: Phase,
    dealer: usize,
    round: usize,
    round_serial: u64,
    honba: u32,
    kan_count: usize,
    call_count: usize,
    seed: u64,
    last_draw: Option<(usize, Tile, bool)>,
    pending_riichi: Option<(usize, bool)>,
    forbidden_discards: Vec<Tile>,
    round_start_points: [i32; 4],
    events: Vec<String>,
    history: Vec<RecordedAction>,
}

impl Game {
    pub fn new(seed: u64) -> Result<Self> {
        if !contracts::declarations_current() {
            return Err(GameError(
                "ルール宣言と生成物が一致しません。生成検査を実行してください".into(),
            ));
        }
        let names = ["あなた", "CPU 青葉", "CPU 琥珀", "CPU 白雪"];
        let mut game = Self {
            players: std::array::from_fn(|index| Player::new(names[index].into())),
            accounts: [
                INITIAL_POINTS,
                INITIAL_POINTS,
                INITIAL_POINTS,
                INITIAL_POINTS,
                0,
            ],
            wall: Vec::new(),
            dead_wall: Vec::new(),
            phase: Phase::MatchEnd,
            dealer: 0,
            round: 0,
            round_serial: 0,
            honba: 0,
            kan_count: 0,
            call_count: 0,
            seed,
            last_draw: None,
            pending_riichi: None,
            forbidden_discards: Vec::new(),
            round_start_points: [INITIAL_POINTS; 4],
            events: Vec::new(),
            history: Vec::new(),
        };
        game.deal()?;
        game.validate()?;
        Ok(game)
    }

    /// 信頼するローカル操作者向けの全情報です。他家の手牌を含みます。
    /// 外部フロントエンド・通常のAIへは `observation` を渡してください。
    pub fn players(&self) -> &[Player; 4] {
        &self.players
    }
    pub fn phase(&self) -> &Phase {
        &self.phase
    }
    pub fn scores(&self) -> [i32; 4] {
        std::array::from_fn(|i| self.accounts[i])
    }
    pub fn riichi_pot(&self) -> i32 {
        self.accounts[4]
    }
    pub fn dealer(&self) -> usize {
        self.dealer
    }
    pub fn round(&self) -> usize {
        self.round
    }
    pub fn honba(&self) -> u32 {
        self.honba
    }
    /// 完全再生用の情報です。この値から山を再構築できるため、共有観測へ含めません。
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn wall_remaining(&self) -> usize {
        self.wall.len()
    }
    pub fn events(&self) -> &[String] {
        &self.events
    }
    pub fn history(&self) -> &[RecordedAction] {
        &self.history
    }
    /// 信頼するローカル表示向け。他家のツモ牌を含み得ます。
    pub fn last_draw(&self) -> Option<(usize, Tile, bool)> {
        self.last_draw
    }
    pub fn round_serial(&self) -> u64 {
        self.round_serial
    }

    /// 0.2.0では不正な席に `None` を返します。別の席へ丸めません。
    pub fn seat_wind(&self, player: usize) -> Option<Tile> {
        let player = Seat::new(player)?;
        Some(tile(
            27 + ((player.index() + PLAYER_COUNT - self.dealer) % PLAYER_COUNT) as u8,
        ))
    }

    pub fn dora_indicators(&self) -> Vec<Tile> {
        (0..=self.kan_count)
            .map(|i| self.dead_wall[4 + i * 2])
            .collect()
    }

    fn ura_indicators(&self) -> Vec<Tile> {
        (0..=self.kan_count)
            .map(|i| self.dead_wall[5 + i * 2])
            .collect()
    }

    pub fn observation(&self, player: usize) -> Result<Observation> {
        if player >= PLAYER_COUNT {
            return Err(GameError("席が範囲外です".into()));
        }
        Ok(Observation {
            seat: player,
            dealer: self.dealer,
            hand: self.players[player].hand.clone(),
            melds: self.players[player].melds.clone(),
            players: std::array::from_fn(|i| PublicPlayer {
                points: self.accounts[i],
                melds: self.players[i].melds.clone(),
                discards: self.players[i].discards.clone(),
                riichi: self.players[i].riichi,
                hand_len: self.players[i].hand.len(),
            }),
            dora_indicators: self.dora_indicators(),
            wall_remaining: self.wall.len(),
            last_discard: match self.phase {
                Phase::Reaction {
                    discarder, tile, ..
                } => Some((discarder, tile)),
                _ => None,
            },
        })
    }

    /// 応答は放銃者に近い席から集めます。解決は全員分が揃ってから行います。
    pub fn next_actor(&self) -> Option<usize> {
        match &self.phase {
            Phase::Discard { player } => Some(*player),
            Phase::Reaction {
                discarder,
                responses,
                ..
            } => (1..PLAYER_COUNT)
                .map(|offset| (discarder + offset) % PLAYER_COUNT)
                .find(|&seat| responses[seat].is_none()),
            Phase::RoundEnd(_) => Some(0),
            Phase::MatchEnd => None,
        }
    }

    pub fn legal_actions(&self, player: usize) -> Vec<Action> {
        if player >= PLAYER_COUNT {
            return Vec::new();
        }
        match &self.phase {
            Phase::Discard { player: current } if player == *current => {
                self.discard_actions(player)
            }
            Phase::Reaction {
                discarder,
                tile,
                responses,
            } if player != *discarder && responses[player].is_none() => {
                self.reaction_actions(player, *discarder, *tile)
            }
            Phase::RoundEnd(_) if player == 0 => vec![Action::NextRound],
            _ => Vec::new(),
        }
    }

    fn discard_actions(&self, player: usize) -> Vec<Action> {
        let p = &self.players[player];
        let mut actions = Vec::new();
        let drawn = self
            .last_draw
            .filter(|&(seat, _, _)| seat == player)
            .map(|(_, t, _)| t);
        if let Some(win_tile) = drawn {
            if self.score(player, true, win_tile).is_some() {
                actions.push(Action::Tsumo);
            }
        }
        for (index, &discard) in p.hand.iter().enumerate() {
            let source = drawn.unwrap_or(discard);
            if !contracts::discard_allowed(p.riichi, source.id(), discard.id())
                || self.forbidden_discards.contains(&discard)
            {
                continue;
            }
            actions.push(Action::Discard(index));
            if !p.riichi
                && p.is_closed()
                && self.accounts[player] >= RIICHI_STICK
                && self.wall.len() >= RIICHI_MIN_WALL
            {
                let mut remaining = p.hand.clone();
                remaining.remove(index);
                let tenpai = !physical_waits(&remaining, &p.melds).is_empty();
                if contracts::can_declare_riichi(
                    p.is_closed(),
                    tenpai,
                    self.accounts[player],
                    self.wall.len(),
                    p.riichi,
                ) {
                    actions.push(Action::Riichi(index));
                }
            }
        }
        if ALLOW_CLOSED_KAN && drawn.is_some() && !p.riichi && self.can_kan() {
            for (id, &count) in counts(&p.hand).iter().enumerate() {
                if count == COPIES_PER_TILE {
                    actions.push(Action::ClosedKan(tile(id as u8)));
                }
            }
        }
        actions
    }

    fn reaction_actions(&self, player: usize, discarder: usize, discarded: Tile) -> Vec<Action> {
        let p = &self.players[player];
        let mut actions = vec![Action::Pass];
        if self.score(player, false, discarded).is_some() && !self.is_furiten(player) {
            actions.push(Action::Ron);
        }
        if p.riichi || self.wall.is_empty() {
            return actions;
        }
        let count = counts(&p.hand)[discarded.id() as usize];
        if ALLOW_PON && count >= 2 {
            actions.push(Action::Pon);
        }
        if ALLOW_OPEN_KAN && count >= 3 && self.can_kan() {
            actions.push(Action::OpenKan);
        }
        if ALLOW_CHI && next(discarder) == player && !discarded.is_honor() {
            let id = discarded.id();
            for start in id.saturating_sub(2)..=id {
                if start / 9 != id / 9 || start % 9 > 6 || id > start + 2 {
                    continue;
                }
                let pair: Vec<Tile> = (start..start + 3)
                    .filter(|&candidate| candidate != id)
                    .map(tile)
                    .collect();
                if pair.len() != 2 || !pair.iter().all(|candidate| p.hand.contains(candidate)) {
                    continue;
                }
                let forbidden = call_forbidden(discarded, Some(start));
                let mut remaining = p.hand.clone();
                for needed in &pair {
                    if let Some(index) = remaining.iter().position(|t| t == needed) {
                        remaining.remove(index);
                    }
                }
                if remaining.iter().any(|t| !forbidden.contains(t)) {
                    actions.push(Action::Chi([pair[0], pair[1]]));
                }
            }
        }
        actions
    }

    pub fn is_furiten(&self, player: usize) -> bool {
        if player >= PLAYER_COUNT {
            return true;
        }
        let p = &self.players[player];
        p.temporary_furiten
            || p.riichi_furiten
            || physical_waits(&p.hand, &p.melds)
                .iter()
                .any(|wait| p.discards.iter().any(|discard| discard.tile == *wait))
    }

    fn can_kan(&self) -> bool {
        self.kan_count < MAX_KANS && !self.wall.is_empty()
    }

    fn score(&self, player: usize, tsumo: bool, win_tile: Tile) -> Option<Score> {
        let p = &self.players[player];
        let mut hand = p.hand.clone();
        if !tsumo {
            hand.push(win_tile);
        }
        let rinshan = tsumo
            && self
                .last_draw
                .map(|(seat, _, kan)| seat == player && kan)
                .unwrap_or(false);
        evaluate(
            &hand,
            &p.melds,
            &WinContext {
                win_tile,
                tsumo,
                seat_wind: self.seat_wind(player)?,
                round_wind: tile(27),
                riichi: p.riichi,
                double_riichi: p.double_riichi,
                ippatsu: p.ippatsu,
                // 嶺上ツモは海底と複合しません。その後の局最後の打牌は河底です。
                last_tile: self.wall.is_empty() && !rinshan,
                rinshan,
                chankan: false,
                dora_indicators: self.dora_indicators(),
                ura_indicators: if p.riichi {
                    self.ura_indicators()
                } else {
                    Vec::new()
                },
            },
        )
    }

    /// 契約: 不正な席・手番・操作はErrを返し、全状態を保ちます。
    /// 成功時は136枚と点数を保存し、次の状態も構造上有効です。
    pub fn apply(&mut self, player: usize, action: Action) -> Result<()> {
        self.validate()?;
        if !self.legal_actions(player).contains(&action) {
            return Err(GameError("現在の手番・手牌では選べない操作です".into()));
        }
        let mut candidate = self.clone_position();
        candidate.apply_inner(player, &action)?;
        candidate.validate()?;
        // 失敗を返す処理を完了してから、過去の領域へ新しい記録だけを追記します。
        // 履歴・表示イベントを候補作成のたびに複製しないため、対局長に比例して
        // 増える記録の反復コピーを避けます。Vecの再確保は通常の償却コストです。
        self.events.append(&mut candidate.events);
        std::mem::swap(&mut self.events, &mut candidate.events);
        self.history.push(RecordedAction { player, action });
        std::mem::swap(&mut self.history, &mut candidate.history);
        *self = candidate;
        Ok(())
    }

    /// 候補に必要な局面だけを複製します。新しいフィールドの追加時は、この
    /// 初期化もコンパイラが要求するため、複製対象からの漏れを検出できます。
    fn clone_position(&self) -> Self {
        Self {
            players: self.players.clone(),
            accounts: self.accounts,
            wall: self.wall.clone(),
            dead_wall: self.dead_wall.clone(),
            phase: self.phase.clone(),
            dealer: self.dealer,
            round: self.round,
            round_serial: self.round_serial,
            honba: self.honba,
            kan_count: self.kan_count,
            call_count: self.call_count,
            seed: self.seed,
            last_draw: self.last_draw,
            pending_riichi: self.pending_riichi,
            forbidden_discards: self.forbidden_discards.clone(),
            round_start_points: self.round_start_points,
            events: Vec::new(),
            history: Vec::new(),
        }
    }
}

fn tile(id: u8) -> Tile {
    Tile::new(id).expect("internal tile index must be valid")
}
fn next(player: usize) -> usize {
    usize::from(contracts::next_seat(player as u8).expect("internal seat must be valid"))
}

fn call_forbidden(called: Tile, sequence_start: Option<u8>) -> Vec<Tile> {
    if ALLOW_KUIKAE {
        return Vec::new();
    }
    let mut forbidden = vec![called];
    if let Some(start) = sequence_start {
        if called.id() == start && start % 9 <= 5 {
            forbidden.push(tile(start + 3));
        }
        if called.id() == start + 2 && start % 9 >= 1 {
            forbidden.push(tile(start - 1));
        }
    }
    forbidden
}

fn physical_waits(hand: &[Tile], melds: &[Meld]) -> Vec<Tile> {
    let mut owned = counts(hand);
    for meld in melds {
        for tile in &meld.tiles {
            owned[tile.id() as usize] += 1;
        }
    }
    winning_tiles(hand, melds.len())
        .into_iter()
        .filter(|t| owned[t.id() as usize] < COPIES_PER_TILE)
        .collect()
}

/// 牌の所有者を変える唯一の基本操作。計数契約を実際のVec操作にも照合します。
fn transfer_tile(
    source: &mut Vec<Tile>,
    destination: &mut Vec<Tile>,
    index: usize,
) -> Result<Tile> {
    let moved = *source
        .get(index)
        .ok_or_else(|| GameError("移動する牌がありません".into()))?;
    let source_count = source.iter().filter(|&&t| t == moved).count() as u8;
    let destination_count = destination.iter().filter(|&&t| t == moved).count() as u8;
    let (after_source, after_destination) = contracts::move_one(source_count, destination_count)
        .ok_or_else(|| GameError("牌の移動が枚数の契約を満たしません".into()))?;
    source.remove(index);
    destination.push(moved);
    if source.iter().filter(|&&t| t == moved).count() != usize::from(after_source)
        || destination.iter().filter(|&&t| t == moved).count() != usize::from(after_destination)
    {
        return Err(GameError("牌の移動後の枚数が契約と一致しません".into()));
    }
    Ok(moved)
}

impl Game {
    fn deal(&mut self) -> Result<()> {
        self.players = std::array::from_fn(|i| Player::new(self.players[i].name.clone()));
        self.wall = (0..TILE_KINDS as u8)
            .flat_map(|id| std::iter::repeat(tile(id)).take(usize::from(COPIES_PER_TILE)))
            .collect();
        let mut rng = SplitMix64(
            self.seed
                .wrapping_add(self.round_serial.wrapping_mul(0x9e3779b97f4a7c15)),
        );
        for index in (1..self.wall.len()).rev() {
            let other = rng.below(index + 1);
            self.wall.swap(index, other);
        }
        self.dead_wall = self.wall.split_off(self.wall.len() - DEAD_WALL);
        self.kan_count = 0;
        self.call_count = 0;
        self.pending_riichi = None;
        self.forbidden_discards.clear();
        self.last_draw = None;
        self.round_start_points = self.scores();
        for _ in 0..13 {
            for offset in 0..PLAYER_COUNT {
                let player = (self.dealer + offset) % PLAYER_COUNT;
                let index = self.wall.len() - 1;
                transfer_tile(&mut self.wall, &mut self.players[player].hand, index)?;
            }
        }
        for player in &mut self.players {
            player.hand.sort_unstable();
        }
        self.events.push(format!(
            "東{}局 {}本場を開始しました",
            self.round + 1,
            self.honba
        ));
        self.draw(self.dealer, false)
    }

    fn draw(&mut self, player: usize, rinshan: bool) -> Result<()> {
        let drawn = if rinshan {
            if !self.can_kan() {
                return Err(GameError("嶺上牌を取得できません".into()));
            }
            let index = self.kan_count;
            let drawn = transfer_tile(&mut self.dead_wall, &mut self.players[player].hand, index)?;
            transfer_tile(&mut self.wall, &mut self.dead_wall, 0)?;
            let replacement = self
                .dead_wall
                .pop()
                .ok_or_else(|| GameError("王牌の補充に失敗しました".into()))?;
            self.dead_wall.insert(index, replacement);
            self.kan_count += 1;
            drawn
        } else {
            if self.wall.is_empty() {
                return self.finish_draw();
            }
            let index = self.wall.len() - 1;
            transfer_tile(&mut self.wall, &mut self.players[player].hand, index)?
        };
        self.players[player].hand.sort_unstable();
        self.players[player].temporary_furiten = false;
        self.last_draw = Some((player, drawn, rinshan));
        self.forbidden_discards.clear();
        self.phase = Phase::Discard { player };
        Ok(())
    }

    fn apply_inner(&mut self, player: usize, action: &Action) -> Result<()> {
        match action {
            Action::Discard(index) => self.discard(player, *index, false),
            Action::Riichi(index) => self.discard(player, *index, true),
            Action::Tsumo => {
                let drawn = self
                    .last_draw
                    .ok_or_else(|| GameError("ツモ牌がありません".into()))?
                    .1;
                self.finish_win(player, None, drawn)
            }
            Action::ClosedKan(kan_tile) => self.closed_kan(player, *kan_tile),
            Action::NextRound => self.advance_round(),
            _ => self.respond(player, action.clone()),
        }
    }

    fn discard(&mut self, player: usize, index: usize, riichi: bool) -> Result<()> {
        let mut tile_holder = Vec::new();
        let discarded = transfer_tile(&mut self.players[player].hand, &mut tile_holder, index)?;
        let p = &mut self.players[player];
        if p.riichi {
            p.ippatsu = false;
        } else {
            // 鳴いてツモを省略した場合も、自分の次の打牌で同巡振聴は終わります。
            // リーチ後の見逃しによる永続振聴は変更しません。
            p.temporary_furiten = false;
        }
        if riichi {
            // 宣言牌で放銃した場合は成立しないため、供託の移動を応答解決まで保留します。
            self.pending_riichi = Some((player, p.discards.is_empty() && self.call_count == 0));
        }
        p.discards.push(Discard {
            tile: discarded,
            called: false,
            riichi,
        });
        self.events.push(format!(
            "{}：{}{}",
            p.name,
            discarded,
            if riichi {
                " リーチ宣言"
            } else {
                " 打牌"
            }
        ));
        self.forbidden_discards.clear();
        let mut responses = std::array::from_fn(|_| None);
        responses[player] = Some(Action::Pass);
        self.phase = Phase::Reaction {
            discarder: player,
            tile: discarded,
            responses,
        };
        Ok(())
    }

    fn respond(&mut self, player: usize, action: Action) -> Result<()> {
        // 役がなくロンできない側の待ちでも、和了形を見送ると同巡振聴になります。
        let completed_shape = match self.phase {
            Phase::Reaction { tile, .. } => {
                physical_waits(&self.players[player].hand, &self.players[player].melds)
                    .contains(&tile)
            }
            _ => false,
        };
        if completed_shape && action != Action::Ron {
            self.players[player].temporary_furiten = true;
            if self.players[player].riichi {
                self.players[player].riichi_furiten = true;
            }
        }
        let complete = match &mut self.phase {
            Phase::Reaction { responses, .. } => {
                responses[player] = Some(action);
                responses.iter().all(Option::is_some)
            }
            _ => return Err(GameError("他家の打牌への応答待ちではありません".into())),
        };
        if complete {
            self.resolve_reactions()?;
        }
        Ok(())
    }

    fn resolve_reactions(&mut self) -> Result<()> {
        let (discarder, discarded, responses) = match &self.phase {
            Phase::Reaction {
                discarder,
                tile,
                responses,
            } => (*discarder, *tile, responses.clone()),
            _ => return Err(GameError("応答状態がありません".into())),
        };
        let order: Vec<usize> = (1..PLAYER_COUNT)
            .map(|offset| (discarder + offset) % PLAYER_COUNT)
            .collect();
        if let Some(&winner) = order
            .iter()
            .find(|&&seat| responses[seat] == Some(Action::Ron))
        {
            if let Some((seat, _)) = self.pending_riichi.take() {
                if let Some(last) = self.players[seat].discards.last_mut() {
                    last.riichi = false;
                }
            }
            return self.finish_win(winner, Some(discarder), discarded);
        }
        if let Some((seat, double)) = self.pending_riichi.take() {
            contracts::transfer_points(&mut self.accounts, seat, 4, RIICHI_STICK)?;
            self.players[seat].riichi = true;
            self.players[seat].double_riichi = double;
            self.players[seat].ippatsu = true;
            self.events
                .push(format!("{}のリーチが成立しました", self.players[seat].name));
        }
        let priority = order
            .iter()
            .copied()
            .find(|&seat| matches!(responses[seat], Some(Action::Pon | Action::OpenKan)))
            .or_else(|| {
                order
                    .iter()
                    .copied()
                    .find(|&seat| matches!(responses[seat], Some(Action::Chi(_))))
            });
        if let Some(caller) = priority {
            let action = responses[caller]
                .clone()
                .ok_or_else(|| GameError("鳴きの選択がありません".into()))?;
            return self.call(caller, discarder, discarded, action);
        }
        self.draw(next(discarder), false)
    }

    fn interrupt_ippatsu(&mut self) {
        self.call_count += 1;
        for player in &mut self.players {
            player.ippatsu = false;
        }
    }

    fn call(
        &mut self,
        player: usize,
        discarder: usize,
        discarded: Tile,
        action: Action,
    ) -> Result<()> {
        self.interrupt_ippatsu();
        let (kind, needed) = match action {
            Action::Pon => (MeldKind::Pon, vec![discarded; 2]),
            Action::OpenKan => (MeldKind::OpenKan, vec![discarded; 3]),
            Action::Chi(pair) => (MeldKind::Chi, pair.to_vec()),
            _ => return Err(GameError("鳴きの種類が不正です".into())),
        };
        let mut tiles = Vec::new();
        for needed_tile in needed {
            let index = self.players[player]
                .hand
                .iter()
                .position(|&t| t == needed_tile)
                .ok_or_else(|| GameError("鳴きに必要な牌がありません".into()))?;
            transfer_tile(&mut self.players[player].hand, &mut tiles, index)?;
        }
        let same = tiles.iter().filter(|&&t| t == discarded).count() as u8;
        contracts::move_one(1, same)
            .ok_or_else(|| GameError("鳴き牌の移動条件を満たしません".into()))?;
        let last = self.players[discarder]
            .discards
            .last_mut()
            .ok_or_else(|| GameError("鳴く対象の捨牌がありません".into()))?;
        if last.called || last.tile != discarded {
            return Err(GameError("捨牌の所有状態が不正です".into()));
        }
        last.called = true;
        tiles.push(discarded);
        tiles.sort_unstable();
        self.forbidden_discards =
            call_forbidden(discarded, (kind == MeldKind::Chi).then_some(tiles[0].id()));
        self.events.push(format!(
            "{}：{} {}",
            self.players[player].name,
            match kind {
                MeldKind::Chi => "チー",
                MeldKind::Pon => "ポン",
                _ => "大明槓",
            },
            discarded
        ));
        self.players[player].melds.push(Meld {
            kind,
            tiles,
            from: Some(discarder),
        });
        self.last_draw = None;
        self.phase = Phase::Discard { player };
        if kind == MeldKind::OpenKan {
            self.draw(player, true)?;
        }
        Ok(())
    }

    fn closed_kan(&mut self, player: usize, kan_tile: Tile) -> Result<()> {
        self.interrupt_ippatsu();
        let mut tiles = Vec::new();
        for _ in 0..4 {
            let index = self.players[player]
                .hand
                .iter()
                .position(|&t| t == kan_tile)
                .ok_or_else(|| GameError("暗槓に必要な4枚がありません".into()))?;
            transfer_tile(&mut self.players[player].hand, &mut tiles, index)?;
        }
        self.players[player].melds.push(Meld {
            kind: MeldKind::ClosedKan,
            tiles,
            from: None,
        });
        self.events
            .push(format!("{}：{} 暗槓", self.players[player].name, kan_tile));
        self.draw(player, true)
    }

    fn finish_win(&mut self, winner: usize, from: Option<usize>, win_tile: Tile) -> Result<()> {
        let score = self
            .score(winner, from.is_none(), win_tile)
            .ok_or_else(|| GameError("役のある和了形ではありません".into()))?;
        if let Some(loser) = from {
            let amount = payment(
                score.base_points,
                if winner == self.dealer { 6 } else { 4 },
                self.honba,
                300,
            )?;
            contracts::transfer_points(&mut self.accounts, loser, winner, amount)?;
        } else {
            for loser in 0..PLAYER_COUNT {
                if loser != winner {
                    let multiplier = if winner == self.dealer || loser == self.dealer {
                        2
                    } else {
                        1
                    };
                    let amount = payment(score.base_points, multiplier, self.honba, 100)?;
                    contracts::transfer_points(&mut self.accounts, loser, winner, amount)?;
                }
            }
        }
        let pot = self.accounts[4];
        if pot > 0 {
            contracts::transfer_points(&mut self.accounts, 4, winner, pot)?;
        }
        let mut winning_hand = self.players[winner].hand.clone();
        if from.is_some() {
            winning_hand.push(win_tile);
            winning_hand.sort_unstable();
        }
        self.events.push(format!(
            "{}：{} {}翻{}符",
            self.players[winner].name,
            if from.is_some() { "ロン" } else { "ツモ" },
            score.han,
            score.fu
        ));
        self.phase = Phase::RoundEnd(RoundResult {
            outcome: RoundOutcome::Win {
                winner,
                from,
                score,
                winning_hand,
            },
            deltas: std::array::from_fn(|i| self.accounts[i] - self.round_start_points[i]),
            dealer_continues: winner == self.dealer,
        });
        Ok(())
    }

    fn finish_draw(&mut self) -> Result<()> {
        let tenpai: [bool; 4] = std::array::from_fn(|i| {
            !physical_waits(&self.players[i].hand, &self.players[i].melds).is_empty()
        });
        let ready = tenpai.iter().filter(|&&t| t).count();
        if ready > 0 && ready < PLAYER_COUNT {
            // 各ノーテン者→各テンパイ者へ同額を移動すると、総額3000点になります。
            let amount = 3000 / (ready * (PLAYER_COUNT - ready)) as i32;
            for loser in 0..PLAYER_COUNT {
                if !tenpai[loser] {
                    for (winner, &is_ready) in tenpai.iter().enumerate() {
                        if is_ready {
                            contracts::transfer_points(&mut self.accounts, loser, winner, amount)?;
                        }
                    }
                }
            }
        }
        self.last_draw = None;
        self.events.push(format!("流局：テンパイ{}人", ready));
        self.phase = Phase::RoundEnd(RoundResult {
            outcome: RoundOutcome::Draw { tenpai },
            deltas: std::array::from_fn(|i| self.accounts[i] - self.round_start_points[i]),
            dealer_continues: tenpai[self.dealer],
        });
        Ok(())
    }

    fn advance_round(&mut self) -> Result<()> {
        let result = match &self.phase {
            Phase::RoundEnd(result) => result.clone(),
            _ => return Err(GameError("局が終了していません".into())),
        };
        if result.dealer_continues || matches!(result.outcome, RoundOutcome::Draw { .. }) {
            self.honba = self
                .honba
                .checked_add(1)
                .ok_or_else(|| GameError("本場数の上限です".into()))?;
        } else {
            self.honba = 0;
        }
        if !result.dealer_continues {
            self.round += 1;
            self.dealer = next(self.dealer);
        }
        if self.round >= ROUND_COUNT || self.accounts[..4].iter().any(|&score| score < 0) {
            let winner = self.ranking()[0];
            let pot = self.accounts[4];
            if pot > 0 {
                contracts::transfer_points(&mut self.accounts, 4, winner, pot)?;
            }
            self.phase = Phase::MatchEnd;
            self.events
                .push(format!("終局：{}が首位です", self.players[winner].name));
            return Ok(());
        }
        self.round_serial = self
            .round_serial
            .checked_add(1)
            .ok_or_else(|| GameError("局数の上限です".into()))?;
        self.deal()
    }

    pub fn ranking(&self) -> Vec<usize> {
        let mut seats: Vec<usize> = (0..PLAYER_COUNT).collect();
        seats.sort_by(|&a, &b| self.accounts[b].cmp(&self.accounts[a]).then(a.cmp(&b)));
        seats
    }

    /// I-01〜I-04: 物理牌、構造枚数、点数、進行状態をすべての公開操作で検査します。
    pub fn validate(&self) -> Result<()> {
        if self.dealer >= PLAYER_COUNT
            || self.kan_count > MAX_KANS
            || self.dead_wall.len() != DEAD_WALL
        {
            return Err(GameError("席・槓・王牌の不変条件違反です".into()));
        }
        if contracts::ledger_total(&self.accounts)
            != i64::from(INITIAL_POINTS) * PLAYER_COUNT as i64
            || self.accounts[4] < 0
            || self.accounts[4] % RIICHI_STICK != 0
        {
            return Err(GameError("持点と供託の保存条件違反です".into()));
        }
        let mut inventory = [0_u16; 34];
        let mut record = |t: Tile| {
            inventory[t.id() as usize] += 1;
        };
        for &t in self.wall.iter().chain(&self.dead_wall) {
            record(t);
        }
        let mut structural = [0_usize; 4];
        for (seat, p) in self.players.iter().enumerate() {
            if p.melds.len() > 4 || p.hand.len() > 14 || (p.riichi && !p.is_closed()) {
                return Err(GameError("手牌と副露の構造が不正です".into()));
            }
            structural[seat] = p.hand.len() + p.melds.len() * 3;
            for &t in &p.hand {
                record(t);
            }
            for meld in &p.melds {
                let expected = match meld.kind {
                    MeldKind::Chi | MeldKind::Pon => 3,
                    _ => 4,
                };
                if meld.tiles.len() != expected {
                    return Err(GameError("副露の枚数が不正です".into()));
                }
                for &t in &meld.tiles {
                    record(t);
                }
            }
            for discard in &p.discards {
                if !discard.called {
                    record(discard.tile);
                }
            }
        }
        if inventory
            .iter()
            .any(|&count| count != u16::from(COPIES_PER_TILE))
        {
            return Err(GameError("各牌4枚・総数136枚の保存条件違反です".into()));
        }
        match &self.phase {
            Phase::Discard { player } => {
                if *player >= PLAYER_COUNT
                    || structural
                        .iter()
                        .enumerate()
                        .any(|(seat, &n)| n != if seat == *player { 14 } else { 13 })
                {
                    return Err(GameError("打牌待ちの手牌枚数が不正です".into()));
                }
            }
            Phase::Reaction {
                discarder,
                tile,
                responses,
            } => {
                if *discarder >= PLAYER_COUNT
                    || structural.iter().any(|&n| n != 13)
                    || responses[*discarder] != Some(Action::Pass)
                    || self.players[*discarder]
                        .discards
                        .last()
                        .map(|d| d.tile != *tile || d.called)
                        .unwrap_or(true)
                {
                    return Err(GameError("打牌応答待ちの状態が不正です".into()));
                }
            }
            Phase::RoundEnd(_) | Phase::MatchEnd => {
                if structural.iter().any(|&n| n != 13 && n != 14)
                    || structural.iter().filter(|&&n| n == 14).count() > 1
                {
                    return Err(GameError("終局時の手牌枚数が不正です".into()));
                }
            }
        }
        Ok(())
    }
}

fn payment(base: u32, multiplier: u32, honba: u32, honba_unit: u32) -> Result<i32> {
    let points = base
        .checked_mul(multiplier)
        .and_then(|n| n.checked_add(99))
        .map(|n| n / 100 * 100)
        .and_then(|n| {
            honba
                .checked_mul(honba_unit)
                .and_then(|bonus| n.checked_add(bonus))
        })
        .ok_or_else(|| GameError("点数計算の上限を超えました".into()))?;
    i32::try_from(points).map_err(|_| GameError("支払点の上限を超えました".into()))
}

/// 再現可能なシャッフル専用。暗号や金銭を賭ける対戦用途には使用しません。
struct SplitMix64(u64);
impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, upper: usize) -> usize {
        let bound = upper as u64;
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.next();
            if value >= threshold {
                return (value % bound) as usize;
            }
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod platform_tests;

#[cfg(test)]
mod core_boundary_tests;

//! Explicitly redacted application views. Never serialize Game or its Debug output.
use crate::{
    actions::action_id,
    types::{ActionId, SeatId, TileId, WireAction},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use suzume::{
    game::{Game, Phase, RoundOutcome},
    scoring::{MeldKind, Score},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MeldType {
    Chi,
    Pon,
    OpenKan,
    ClosedKan,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeldView {
    pub kind: MeldType,
    pub tiles: Vec<TileId>,
    pub from: Option<SeatId>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiscardView {
    pub tile: TileId,
    pub called: bool,
    pub riichi: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlayerView {
    pub seat: SeatId,
    pub name: String,
    pub points: i32,
    pub hand_count: u8,
    pub melds: Vec<MeldView>,
    pub discards: Vec<DiscardView>,
    pub riichi: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum YakuId {
    Kokushi,
    Chiitoitsu,
    Suuankou,
    Suukantsu,
    Daisangen,
    Daisuushii,
    Shousuushii,
    Pinfu,
    WhiteDragon,
    GreenDragon,
    RedDragon,
    SeatWind,
    RoundWind,
    Sanankou,
    Sankantsu,
    Toitoi,
    Shousangen,
    Ryanpeikou,
    Iipeikou,
    SanshokuDoujun,
    SanshokuDoukou,
    Ittsu,
    Chanta,
    Junchan,
    DoubleRiichi,
    Riichi,
    Ippatsu,
    MenzenTsumo,
    Haitei,
    Houtei,
    Rinshan,
    Chankan,
    Tanyao,
    Honroutou,
    Honitsu,
    Chinitsu,
    Tsuuiisou,
    Chinroutou,
    Ryuuiisou,
    Chuuren,
    Unknown,
}
impl YakuId {
    fn from_name(name: &str) -> Self {
        match name {
            "国士無双" => Self::Kokushi,
            "七対子" => Self::Chiitoitsu,
            "四暗刻" => Self::Suuankou,
            "四槓子" => Self::Suukantsu,
            "大三元" => Self::Daisangen,
            "大四喜" => Self::Daisuushii,
            "小四喜" => Self::Shousuushii,
            "平和" => Self::Pinfu,
            "役牌・白" => Self::WhiteDragon,
            "役牌・發" => Self::GreenDragon,
            "役牌・中" => Self::RedDragon,
            "役牌・自風" => Self::SeatWind,
            "役牌・場風" => Self::RoundWind,
            "三暗刻" => Self::Sanankou,
            "三槓子" => Self::Sankantsu,
            "対々和" => Self::Toitoi,
            "小三元" => Self::Shousangen,
            "二盃口" => Self::Ryanpeikou,
            "一盃口" => Self::Iipeikou,
            "三色同順" => Self::SanshokuDoujun,
            "三色同刻" => Self::SanshokuDoukou,
            "一気通貫" => Self::Ittsu,
            "混全帯么九" => Self::Chanta,
            "純全帯么九" => Self::Junchan,
            "ダブル立直" => Self::DoubleRiichi,
            "立直" => Self::Riichi,
            "一発" => Self::Ippatsu,
            "門前清自摸和" => Self::MenzenTsumo,
            "海底摸月" => Self::Haitei,
            "河底撈魚" => Self::Houtei,
            "嶺上開花" => Self::Rinshan,
            "搶槓" => Self::Chankan,
            "断么九" => Self::Tanyao,
            "混老頭" => Self::Honroutou,
            "混一色" => Self::Honitsu,
            "清一色" => Self::Chinitsu,
            "字一色" => Self::Tsuuiisou,
            "清老頭" => Self::Chinroutou,
            "緑一色" => Self::Ryuuiisou,
            "九蓮宝燈" => Self::Chuuren,
            _ => Self::Unknown,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct YakuView {
    pub id: YakuId,
    pub label: String,
    pub han: u8,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScoreView {
    pub han: u8,
    pub fu: u16,
    pub yakuman: u8,
    pub base_points: u32,
    pub dora: u8,
    pub yaku: Vec<YakuView>,
    pub limit_label: Option<String>,
}
impl From<&Score> for ScoreView {
    fn from(score: &Score) -> Self {
        Self {
            han: score.han,
            fu: score.fu,
            yakuman: score.yakuman,
            base_points: score.base_points,
            dora: score.dora,
            yaku: score
                .yaku
                .iter()
                .map(|y| YakuView {
                    id: YakuId::from_name(y.name),
                    label: y.name.into(),
                    han: y.han,
                })
                .collect(),
            limit_label: score.limit_name.map(str::to_owned),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OutcomeView {
    Win {
        winner: SeatId,
        from: Option<SeatId>,
        score: ScoreView,
        winning_hand: Vec<TileId>,
    },
    Draw {
        tenpai: [bool; 4],
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PublicPhase {
    Discard {
        actor: SeatId,
    },
    Reaction {
        discarder: SeatId,
        tile: TileId,
    },
    RoundEnd {
        outcome: OutcomeView,
        deltas: [i32; 4],
        dealer_continues: bool,
    },
    MatchEnd,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LegalAction {
    pub id: ActionId,
    pub action: WireAction,
    pub label: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GameView {
    pub session_id: u32,
    pub revision: u32,
    pub viewer: Option<SeatId>,
    pub actor: Option<SeatId>,
    pub round: u8,
    pub honba: u32,
    pub dealer: SeatId,
    pub riichi_pot: i32,
    pub wall_remaining: u8,
    pub dora_indicators: Vec<TileId>,
    pub players: [PlayerView; 4],
    pub own_hand: Option<Vec<TileId>>,
    pub own_last_draw: Option<TileId>,
    pub own_furiten: Option<bool>,
    pub phase: PublicPhase,
    pub legal_actions: Vec<LegalAction>,
}
fn seat(value: usize) -> SeatId {
    SeatId::new(value as u8).expect("validated core seat")
}
impl GameView {
    pub fn from_game(game: &Game, viewer: Option<SeatId>, session_id: u32, revision: u32) -> Self {
        let viewer_index = viewer.map(|s| usize::from(s.get()));
        let phase = match game.phase() {
            Phase::Discard { player } => PublicPhase::Discard {
                actor: seat(*player),
            },
            Phase::Reaction {
                discarder, tile, ..
            } => PublicPhase::Reaction {
                discarder: seat(*discarder),
                tile: (*tile).into(),
            },
            Phase::MatchEnd => PublicPhase::MatchEnd,
            Phase::RoundEnd(result) => PublicPhase::RoundEnd {
                deltas: result.deltas,
                dealer_continues: result.dealer_continues,
                outcome: match &result.outcome {
                    RoundOutcome::Win {
                        winner,
                        from,
                        score,
                        winning_hand,
                    } => OutcomeView::Win {
                        winner: seat(*winner),
                        from: from.map(seat),
                        score: score.into(),
                        winning_hand: winning_hand.iter().copied().map(Into::into).collect(),
                    },
                    RoundOutcome::Draw { tenpai } => OutcomeView::Draw { tenpai: *tenpai },
                },
            },
        };
        let legal_actions = match viewer_index {
            Some(actor) if Some(actor) == game.next_actor() => game
                .legal_actions(actor)
                .iter()
                .filter_map(|a| {
                    Some(LegalAction {
                        id: action_id(game, actor, a)?,
                        action: WireAction::from_core(a)?,
                        label: a.label(&game.players()[actor].hand),
                    })
                })
                .collect(),
            _ => Vec::new(),
        };
        Self {
            session_id,
            revision,
            viewer,
            actor: game.next_actor().map(seat),
            round: game.round() as u8,
            honba: game.honba(),
            dealer: seat(game.dealer()),
            riichi_pot: game.riichi_pot(),
            wall_remaining: game.wall_remaining() as u8,
            dora_indicators: game.dora_indicators().into_iter().map(Into::into).collect(),
            players: std::array::from_fn(|i| {
                let p = &game.players()[i];
                PlayerView {
                    seat: seat(i),
                    name: p.name.clone(),
                    points: game.scores()[i],
                    hand_count: p.hand.len() as u8,
                    riichi: p.riichi,
                    discards: p
                        .discards
                        .iter()
                        .map(|d| DiscardView {
                            tile: d.tile.into(),
                            called: d.called,
                            riichi: d.riichi,
                        })
                        .collect(),
                    melds: p
                        .melds
                        .iter()
                        .map(|m| MeldView {
                            kind: match m.kind {
                                MeldKind::Chi => MeldType::Chi,
                                MeldKind::Pon => MeldType::Pon,
                                MeldKind::OpenKan => MeldType::OpenKan,
                                MeldKind::ClosedKan => MeldType::ClosedKan,
                            },
                            tiles: m.tiles.iter().copied().map(Into::into).collect(),
                            from: m.from.map(seat),
                        })
                        .collect(),
                }
            }),
            own_hand: viewer_index.map(|i| {
                game.players()[i]
                    .hand
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect()
            }),
            own_last_draw: game.last_draw().and_then(|(i, t, _)| {
                if Some(i) == viewer_index {
                    Some(t.into())
                } else {
                    None
                }
            }),
            own_furiten: viewer_index.map(|i| game.is_furiten(i)),
            phase,
            legal_actions,
        }
    }
}

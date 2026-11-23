use crate::{logging::EndReason, types::SeatId};
use schemars::{gen::SchemaGenerator, schema::Schema, JsonSchema};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use suzume::tile::Tile;

/// Lossless mjai tile spelling. Red fives and unknown tiles remain distinct.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Pai(String);

impl Pai {
    pub fn new(value: &str) -> Result<Self, &'static str> {
        let bytes = value.as_bytes();
        let suited = bytes.len() == 2
            && (b'1'..=b'9').contains(&bytes[0])
            && matches!(bytes[1], b'm' | b'p' | b's');
        if suited
            || matches!(
                value,
                "E" | "S" | "W" | "N" | "P" | "F" | "C" | "5mr" | "5pr" | "5sr" | "?"
            )
        {
            Ok(Self(value.into()))
        } else {
            Err("invalid mjai tile; expected 1m..9s, E/S/W/N/P/F/C, 5mr/5pr/5sr, or ?")
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn unknown() -> Self {
        Self("?".into())
    }
    pub fn is_unknown(&self) -> bool {
        self.0 == "?"
    }
    pub fn is_red(&self) -> bool {
        self.0.ends_with('r')
    }

    /// Kind identity for physical copy accounting only; never a lossy conversion.
    pub fn kind_id(&self) -> Option<u8> {
        match self.as_str() {
            "?" => None,
            "E" => Some(27),
            "S" => Some(28),
            "W" => Some(29),
            "N" => Some(30),
            "P" => Some(31),
            "F" => Some(32),
            "C" => Some(33),
            _ => {
                let bytes = self.0.as_bytes();
                let offset = match bytes[1] {
                    b'm' => 0,
                    b'p' => 9,
                    _ => 18,
                };
                Some(offset + bytes[0] - b'1')
            }
        }
    }
}

impl From<Tile> for Pai {
    fn from(value: Tile) -> Self {
        if value.id() < 27 {
            Self(value.short())
        } else {
            Self(["E", "S", "W", "N", "P", "F", "C"][usize::from(value.id() - 27)].into())
        }
    }
}
impl Serialize for Pai {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for Pai {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(&String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}
impl JsonSchema for Pai {
    fn schema_name() -> String {
        "MjaiPai".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        serde_json::from_value(serde_json::json!({
            "type":"string", "pattern":"^([1-9][mps]|5[mps]r|[ESWNPFC]|\\?)$"
        }))
        .expect("static tile schema")
    }
}

/// Known event fields. Unknown fields are retained in [`ParsedEvent::raw`].
/// Parsing this type alone validates structure, not event order or tile flow.
/// The closed JSON Schema describes this known-field projection, not the raw
/// producer object: consumers must retain extension fields separately.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[schemars(rename = "MjaiEvent", deny_unknown_fields)]
pub enum Event {
    StartGame {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        names: Option<[String; 4]>,
    },
    StartKyoku {
        bakaze: Pai,
        dora_marker: Pai,
        kyoku: u8,
        honba: u32,
        kyotaku: u32,
        oya: SeatId,
        scores: [i32; 4],
        tehais: [Vec<Pai>; 4],
    },
    Tsumo {
        actor: SeatId,
        pai: Pai,
    },
    Dahai {
        actor: SeatId,
        pai: Pai,
        tsumogiri: bool,
    },
    Chi {
        actor: SeatId,
        target: SeatId,
        pai: Pai,
        consumed: [Pai; 2],
    },
    Pon {
        actor: SeatId,
        target: SeatId,
        pai: Pai,
        consumed: [Pai; 2],
    },
    Daiminkan {
        actor: SeatId,
        target: SeatId,
        pai: Pai,
        consumed: [Pai; 3],
    },
    Ankan {
        actor: SeatId,
        consumed: [Pai; 4],
    },
    Kakan {
        actor: SeatId,
        pai: Pai,
        consumed: [Pai; 3],
    },
    Dora {
        dora_marker: Pai,
    },
    Reach {
        actor: SeatId,
    },
    ReachAccepted {
        actor: SeatId,
    },
    Hora {
        actor: SeatId,
        target: SeatId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deltas: Option<[i32; 4]>,
        #[serde(
            default,
            alias = "uradora_markers",
            skip_serializing_if = "Option::is_none"
        )]
        ura_markers: Option<Vec<Pai>>,
    },
    Ryukyoku {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deltas: Option<[i32; 4]>,
    },
    EndKyoku,
    EndGame,
    None,
}

impl Event {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::StartGame { .. } => "start_game",
            Self::StartKyoku { .. } => "start_kyoku",
            Self::Tsumo { .. } => "tsumo",
            Self::Dahai { .. } => "dahai",
            Self::Chi { .. } => "chi",
            Self::Pon { .. } => "pon",
            Self::Daiminkan { .. } => "daiminkan",
            Self::Ankan { .. } => "ankan",
            Self::Kakan { .. } => "kakan",
            Self::Dora { .. } => "dora",
            Self::Reach { .. } => "reach",
            Self::ReachAccepted { .. } => "reach_accepted",
            Self::Hora { .. } => "hora",
            Self::Ryukyoku { .. } => "ryukyoku",
            Self::EndKyoku => "end_kyoku",
            Self::EndGame => "end_game",
            Self::None => "none",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParsedEvent {
    pub line: usize,
    pub event: Event,
    /// Original JSON including producer-specific metadata and optional fields.
    pub raw: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MeldKind {
    Chi,
    Pon,
    Daiminkan,
    Ankan,
    Kakan,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MjaiMeld {
    pub kind: MeldKind,
    pub tiles: Vec<Pai>,
    pub target: Option<SeatId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MjaiDiscard {
    pub pai: Pai,
    pub tsumogiri: bool,
    pub called: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MjaiPlayer {
    pub hand_count: u8,
    pub melds: Vec<MjaiMeld>,
    pub discards: Vec<MjaiDiscard>,
    pub reach_declared: bool,
    pub reach_accepted: bool,
}

/// A prefix observation. It intentionally contains no hidden opponent tile,
/// future event, seed, legal-action mask, or claim of external rule legality.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentObservation {
    pub viewer: Option<SeatId>,
    pub own_hand: Option<Vec<Pai>>,
    pub own_last_draw: Option<Pai>,
    pub players: [MjaiPlayer; 4],
    pub scores: Option<[i32; 4]>,
    pub riichi_sticks: Option<u32>,
    pub bakaze: Option<Pai>,
    pub kyoku: Option<u8>,
    pub honba: Option<u32>,
    pub dealer: Option<SeatId>,
    pub dora_markers: Vec<Pai>,
    pub last_discard: Option<MjaiLastDiscard>,
    /// A hora or ryukyoku has settled the current round; input end alone does not.
    pub round_ended: bool,
    /// The record explicitly declares natural completion, rather than interruption.
    pub game_ended: bool,
    /// An explicit end_game was consumed, including interrupted/truncated records.
    pub record_ended: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MjaiLastDiscard {
    pub actor: SeatId,
    pub pai: Pai,
}

/// Counts refer to accepted events, never internal Suzume operations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub records: u32,
    pub rounds: u32,
    pub wins: u32,
    pub draws: u32,
    pub calls: u32,
    /// Number of raw events containing at least one red tile spelling.
    pub red_events: u32,
    pub final_scores: Option<[i32; 4]>,
    pub riichi_sticks: Option<u32>,
    pub complete: bool,
    pub termination: EndReason,
}

impl Default for Summary {
    fn default() -> Self {
        Self {
            records: 0,
            rounds: 0,
            wins: 0,
            draws: 0,
            calls: 0,
            red_events: 0,
            final_scores: None,
            riichi_sticks: None,
            complete: false,
            termination: EndReason::Interrupted,
        }
    }
}

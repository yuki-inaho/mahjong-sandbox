//! Versioned wire values. Invalid identifiers cannot be constructed by deserialization.
use schemars::{gen::SchemaGenerator, schema::Schema, JsonSchema};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use suzume::{game::Action, tile::Tile};

fn primitive_schema(value: serde_json::Value) -> Schema {
    serde_json::from_value(value).expect("static schema is valid")
}
macro_rules! bounded_integer {
    ($name:ident, $storage:ty, $max:expr) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name($storage);
        impl $name {
            pub fn new(value: $storage) -> Option<Self> { (value <= $max).then_some(Self(value)) }
            pub const fn get(self) -> $storage { self.0 }
        }
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> { self.0.serialize(serializer) }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <$storage>::deserialize(deserializer)?;
                Self::new(value).ok_or_else(|| de::Error::custom(concat!(stringify!($name), " is out of range")))
            }
        }
        impl JsonSchema for $name {
            fn schema_name() -> String { stringify!($name).into() }
            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                primitive_schema(json!({"type":"integer","minimum":0,"maximum":$max}))
            }
        }
    };
}
bounded_integer!(SeatId, u8, 3);
bounded_integer!(TileId, u8, 33);
bounded_integer!(HandIndex, u8, 13);
bounded_integer!(ActionId, u16, 188);
impl From<Tile> for TileId {
    fn from(tile: Tile) -> Self {
        Self(tile.id())
    }
}
impl From<TileId> for Tile {
    fn from(tile: TileId) -> Self {
        Tile::new(tile.0).expect("validated tile")
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Version;
impl Serialize for Version {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        1u8.serialize(s)
    }
}
impl<'de> Deserialize<'de> for Version {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        if u8::deserialize(d)? == 1 {
            Ok(Self)
        } else {
            Err(de::Error::custom("supported schema version is 1"))
        }
    }
}
impl JsonSchema for Version {
    fn schema_name() -> String {
        "Version".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        primitive_schema(json!({"type":"integer","enum":[1]}))
    }
}

/// Canonical unsigned 64-bit decimal text; never represented as a JavaScript number.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Seed(String);
impl Seed {
    pub fn new(text: &str) -> Result<Self, &'static str> {
        let value: u64 = text
            .parse()
            .map_err(|_| "seed must be a canonical u64 decimal string")?;
        if value.to_string() != text {
            return Err("seed must not have a sign or leading zeroes");
        }
        Ok(Self(text.into()))
    }
    pub fn value(&self) -> u64 {
        self.0.parse().expect("validated seed")
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl Default for Seed {
    fn default() -> Self {
        Self("0".into())
    }
}
impl From<u64> for Seed {
    fn from(value: u64) -> Self {
        Self(value.to_string())
    }
}
impl Serialize for Seed {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}
impl<'de> Deserialize<'de> for Seed {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(&String::deserialize(d)?).map_err(de::Error::custom)
    }
}
impl JsonSchema for Seed {
    fn schema_name() -> String {
        "Seed".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        primitive_schema(
            json!({"type":"string","pattern":"^(0|[1-9][0-9]{0,19})$","format":"uint64-decimal"}),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WireAction {
    Discard { index: HandIndex },
    Riichi { index: HandIndex },
    Tsumo,
    Ron,
    Pon,
    Chi { tiles: [TileId; 2] },
    OpenKan,
    ClosedKan { tile: TileId },
    Pass,
    NextRound,
}
impl WireAction {
    pub fn to_core(&self) -> Action {
        match self {
            Self::Discard { index } => Action::Discard(usize::from(index.get())),
            Self::Riichi { index } => Action::Riichi(usize::from(index.get())),
            Self::Tsumo => Action::Tsumo,
            Self::Ron => Action::Ron,
            Self::Pon => Action::Pon,
            Self::Chi { tiles } => Action::Chi([tiles[0].into(), tiles[1].into()]),
            Self::OpenKan => Action::OpenKan,
            Self::ClosedKan { tile } => Action::ClosedKan((*tile).into()),
            Self::Pass => Action::Pass,
            Self::NextRound => Action::NextRound,
        }
    }
    pub fn from_core(action: &Action) -> Option<Self> {
        let index = |value: usize| u8::try_from(value).ok().and_then(HandIndex::new);
        Some(match action {
            Action::Discard(i) => Self::Discard { index: index(*i)? },
            Action::Riichi(i) => Self::Riichi { index: index(*i)? },
            Action::Tsumo => Self::Tsumo,
            Action::Ron => Self::Ron,
            Action::Pon => Self::Pon,
            Action::Chi(tiles) => Self::Chi {
                tiles: [tiles[0].into(), tiles[1].into()],
            },
            Action::OpenKan => Self::OpenKan,
            Action::ClosedKan(tile) => Self::ClosedKan {
                tile: (*tile).into(),
            },
            Action::Pass => Self::Pass,
            Action::NextRound => Self::NextRound,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    InvalidConfig,
    NoGame,
    StaleSession,
    StaleRevision,
    WrongActor,
    IllegalAction,
    InvalidAi,
    StepLimit,
    WrongMode,
    InternalError,
    LogError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}
impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ApiError {}

/// Exact signed 64-bit values in analysis output, including beyond JS safe integers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedValue(String);
impl From<i64> for SignedValue {
    fn from(value: i64) -> Self {
        Self(value.to_string())
    }
}
impl Serialize for SignedValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}
impl<'de> Deserialize<'de> for SignedValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        let n: i64 = text
            .parse()
            .map_err(|_| de::Error::custom("expected canonical i64 decimal string"))?;
        if n.to_string() != text {
            return Err(de::Error::custom("noncanonical i64 decimal string"));
        }
        Ok(Self(text))
    }
}
impl JsonSchema for SignedValue {
    fn schema_name() -> String {
        "SignedValue".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        primitive_schema(
            json!({"type":"string","pattern":"^(0|-?[1-9][0-9]{0,18})$","format":"int64-decimal"}),
        )
    }
}

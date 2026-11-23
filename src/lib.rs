//! 独自実装の四人リーチ麻雀。端末表示に依存しない対局ライブラリです。
//!
//! ルール宣言、契約、判定、対局遷移、意思決定を別の責務に分離しています。
#![forbid(unsafe_code)]

pub mod ai;
pub mod contracts;
pub mod game;
pub mod hand;
pub mod rule_constants;
pub mod scoring;
pub mod tile;

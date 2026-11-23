use serde_json::Value;
use std::io::{self, Cursor, Write};
use suzume::ai::{GreedyAi, Policy};
use suzume::game::{Action, Game, Phase};
use suzume_api::config::AppConfig;
use suzume_api::logging::{EndReason, LogLimits};
use suzume_api::mjai::{analyze, MjaiWriter};

fn values(bytes: &[u8]) -> Vec<Value> {
    std::str::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn initial_dealer_draw_is_separate_from_thirteen_tile_hands() {
    let config = AppConfig::default();
    let game = Game::new(0).unwrap();
    let bytes = MjaiWriter::new(Vec::new(), &config, &game, false)
        .unwrap()
        .finish(&game, EndReason::Interrupted)
        .unwrap();
    let rows = values(&bytes);
    assert_eq!(rows[0]["type"], "start_game");
    assert_eq!(rows[1]["type"], "start_kyoku");
    for hand in rows[1]["tehais"].as_array().unwrap() {
        assert_eq!(hand.as_array().unwrap().len(), 13);
    }
    assert_eq!(rows[2]["type"], "tsumo");
    assert_eq!(rows[2]["actor"], 0);
    let summary = analyze(&mut Cursor::new(bytes), LogLimits::default()).unwrap();
    assert!(!summary.complete);
    assert_eq!(summary.termination, EndReason::Interrupted);
    assert_eq!(summary.final_scores, Some(game.scores()));
}

#[test]
fn partial_reactions_emit_no_fictitious_draw_or_call() {
    let config = AppConfig::default();
    let mut game = Game::new(0).unwrap();
    let mut writer = MjaiWriter::new(Vec::new(), &config, &game, false).unwrap();
    game.apply(0, Action::Discard(0)).unwrap();
    writer.record(0, &Action::Discard(0), &game, None).unwrap();
    let actor = game.next_actor().unwrap();
    game.apply(actor, Action::Pass).unwrap();
    writer.record(actor, &Action::Pass, &game, None).unwrap();
    let rows = values(&writer.finish(&game, EndReason::Interrupted).unwrap());
    assert_eq!(rows.iter().filter(|row| row["type"] == "tsumo").count(), 1);
    assert_eq!(rows.iter().filter(|row| row["type"] == "dahai").count(), 1);
    assert!(rows.iter().all(|row| row["type"] != "none"));
}

#[test]
fn complete_native_games_match_mjai_rounds_and_final_scores() {
    for seed in 0..4 {
        let mut config = AppConfig::default();
        config.game.seed = seed.into();
        let mut game = Game::new(seed).unwrap();
        let mut writer = MjaiWriter::new(Vec::new(), &config, &game, false).unwrap();
        let mut rounds = 0;
        while !matches!(game.phase(), Phase::MatchEnd) {
            let actor = game.next_actor().unwrap();
            let action = GreedyAi
                .choose_action(
                    &game.observation(actor).unwrap(),
                    &game.legal_actions(actor),
                )
                .unwrap();
            game.apply(actor, action.clone()).unwrap();
            if matches!(game.phase(), Phase::RoundEnd(_)) {
                rounds += 1;
            }
            writer.record(actor, &action, &game, None).unwrap();
            assert!(game.history().len() < 10_000);
        }
        let bytes = writer.finish(&game, EndReason::Completed).unwrap();
        let summary = analyze(&mut Cursor::new(bytes), LogLimits::default()).unwrap();
        assert!(summary.complete, "seed {seed}");
        assert_eq!(summary.rounds, rounds);
        assert_eq!(summary.final_scores, Some(game.scores()), "seed {seed}");
        assert_eq!(
            summary.riichi_sticks,
            Some((game.riichi_pot() / 1000) as u32)
        );
    }
}

#[test]
fn public_mjai_masks_all_initial_hands_and_draws() {
    let config = AppConfig::default();
    let game = Game::new(0).unwrap();
    let bytes = MjaiWriter::new(Vec::new(), &config, &game, true)
        .unwrap()
        .finish(&game, EndReason::Interrupted)
        .unwrap();
    let rows = values(&bytes);
    assert!(rows[1]["tehais"].as_array().unwrap().iter().all(|hand| hand
        .as_array()
        .unwrap()
        .iter()
        .all(|tile| tile == "?")));
    assert_eq!(rows[2]["pai"], "?");
    let text = std::str::from_utf8(&bytes).unwrap();
    for forbidden in ["seed", "decision", "hand_index", "before_sha256"] {
        assert!(!text.contains(forbidden));
    }
    analyze(&mut Cursor::new(bytes), LogLimits::default()).unwrap();
}

#[test]
fn writer_rejects_skipped_operations_and_wrong_finish_reason() {
    let config = AppConfig::default();
    let mut game = Game::new(0).unwrap();
    let mut writer = MjaiWriter::new(Vec::new(), &config, &game, false).unwrap();
    game.apply(0, Action::Discard(0)).unwrap();
    game.apply(1, Action::Pass).unwrap();
    assert!(writer.record(1, &Action::Pass, &game, None).is_err());
    let game = Game::new(0).unwrap();
    assert!(MjaiWriter::new(Vec::new(), &config, &game, false)
        .unwrap()
        .finish(&game, EndReason::Completed)
        .is_err());
}

struct FailingWriter {
    remaining: usize,
}
impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("injected failure"));
        }
        let size = bytes.len().min(self.remaining);
        self.remaining -= size;
        Ok(size)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn partial_io_failure_is_reported() {
    let game = Game::new(0).unwrap();
    assert!(MjaiWriter::new(
        FailingWriter { remaining: 7 },
        &AppConfig::default(),
        &game,
        false
    )
    .is_err());
}

struct ControlledWriter(std::rc::Rc<std::cell::Cell<usize>>);
impl Write for ControlledWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let remaining = self.0.get();
        if remaining == 0 {
            return Err(io::Error::other("injected record write failure"));
        }
        let amount = remaining.min(bytes.len());
        self.0.set(remaining - amount);
        Ok(amount)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn partial_record_write_poison_cannot_be_retried() {
    let config = AppConfig::default();
    let mut game = Game::new(0).unwrap();
    let remaining = std::rc::Rc::new(std::cell::Cell::new(1_000_000));
    let mut writer =
        MjaiWriter::new(ControlledWriter(remaining.clone()), &config, &game, false).unwrap();
    game.apply(0, Action::Discard(0)).unwrap();
    remaining.set(5);
    assert!(writer.record(0, &Action::Discard(0), &game, None).is_err());
    remaining.set(1_000_000);
    assert!(writer
        .record(0, &Action::Discard(0), &game, None)
        .unwrap_err()
        .message
        .contains("unusable"));
}

#[test]
fn reach_acceptance_precedes_call_from_a_lower_numbered_seat() {
    // Seed 6, operation 345: seat 0 calls the declaration discard from seat 2.
    // The physical seat loop order must not reorder acceptance after the call.
    let mut config = AppConfig::default();
    config.game.seed = 6_u64.into();
    let mut game = Game::new(6).unwrap();
    let mut writer = MjaiWriter::new(Vec::new(), &config, &game, false).unwrap();
    for _ in 0..345 {
        let actor = game.next_actor().unwrap();
        let action = GreedyAi
            .choose_action(
                &game.observation(actor).unwrap(),
                &game.legal_actions(actor),
            )
            .unwrap();
        game.apply(actor, action.clone()).unwrap();
        writer.record(actor, &action, &game, None).unwrap();
    }
    let bytes = writer.finish(&game, EndReason::Interrupted).unwrap();
    let rows = values(&bytes);
    let acceptance = rows
        .iter()
        .rposition(|row| row["type"] == "reach_accepted" && row["actor"] == 2)
        .unwrap();
    assert_eq!(rows[acceptance + 1]["type"], "pon");
    assert_eq!(rows[acceptance + 1]["actor"], 0);
    let summary = analyze(&mut Cursor::new(bytes), LogLimits::default()).unwrap();
    assert_eq!(summary.final_scores, Some(game.scores()));
}

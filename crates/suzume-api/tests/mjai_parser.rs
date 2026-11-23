//! Synthetic contracts complement the separately fetched real-game corpus.
use serde_json::{json, Value};
use std::io::Cursor;
use suzume_api::{
    logging::{EndReason, LogLimits},
    mjai::{analyze, parse_event, visit, Pai, ParsedEvent, ReplayState},
    types::SeatId,
};

fn start() -> Vec<Value> {
    vec![
        json!({"type":"start_game","names":["A","B","C","D"],"rating":[1.5,2.0]}),
        json!({"type":"start_kyoku","bakaze":"E","dora_marker":"4p","kyoku":1,
            "honba":0,"kyotaku":0,"oya":0,"scores":[25000,25000,25000,25000],
            "tehais":[["1m","1m","2m","3m","4m","5mr","6m","7m","8m","9m","E","P","C"],
                ["?","?","?","?","?","?","?","?","?","?","?","?","?"],
                ["?","?","?","?","?","?","?","?","?","?","?","?","?"],
                ["?","?","?","?","?","?","?","?","?","?","?","?","?"]]}),
    ]
}

fn complete() -> Vec<Value> {
    let mut values = start();
    values.extend([
        json!({"type":"tsumo","actor":0,"pai":"9s"}),
        json!({"type":"reach","actor":0}),
        json!({"type":"dahai","actor":0,"pai":"9s","tsumogiri":true}),
        json!({"type":"reach_accepted","actor":0}),
        json!({"type":"tsumo","actor":1,"pai":"?"}),
        json!({"type":"dahai","actor":1,"pai":"5p","tsumogiri":true}),
        json!({"type":"hora","actor":0,"target":1,"deltas":[9000,-8000,0,0],"ura_markers":["W"]}),
        json!({"type":"end_kyoku"}),
        json!({"type":"end_game"}),
    ]);
    values
}

fn text(values: &[Value]) -> String {
    values.iter().map(|value| format!("{value}\n")).collect()
}

fn inspect(values: &[Value]) -> Result<suzume_api::mjai::Summary, suzume_api::types::ApiError> {
    analyze(&mut Cursor::new(text(values)), LogLimits::default())
}

#[test]
fn accounts_for_accepted_reach_and_winning_deposit_without_inventing_scores() {
    let summary = inspect(&complete()).unwrap();
    assert_eq!(summary.records, 11);
    assert_eq!(summary.rounds, 1);
    assert_eq!(summary.wins, 1);
    assert_eq!(summary.final_scores, Some([33000, 17000, 25000, 25000]));
    assert_eq!(summary.riichi_sticks, Some(0));
    assert!(summary.complete);
    assert_eq!(summary.termination, EndReason::Completed);
}

#[test]
fn preserves_red_unknown_and_extension_data_but_masks_other_hands() {
    let mut seen = 0;
    visit(
        &mut Cursor::new(text(&complete())),
        LogLimits::default(),
        |record, state| {
            if record.line == 1 {
                assert_eq!(record.raw["rating"][0], 1.5);
            }
            if record.line == 3 {
                let own = state.observe(SeatId::new(0));
                assert_eq!(own.own_hand.as_ref().unwrap().len(), 14);
                assert!(own
                    .own_hand
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(|tile| tile.as_str() == "5mr"));
                assert_eq!(own.players[1].hand_count, 13);
                let spectator = state.observe(None);
                assert!(spectator.own_hand.is_none());
                assert!(!serde_json::to_string(&spectator).unwrap().contains("5mr"));
                seen += 1;
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(seen, 1);
    assert_eq!(Pai::new("?").unwrap().as_str(), "?");
}

#[test]
fn accepts_original_uradora_alias_without_erasing_raw_fields() {
    let mut values = complete();
    values[8].as_object_mut().unwrap().remove("ura_markers");
    values[8]["uradora_markers"] = json!(["5mr"]);
    values[8]["extra"] = json!({"score_label":"満貫"});
    visit(
        &mut Cursor::new(text(&values)),
        LogLimits::default(),
        |record, _| {
            if record.line == 9 {
                assert_eq!(record.raw["extra"]["score_label"], "満貫");
            }
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn rejects_duplicate_keys_before_typed_deserialization() {
    for line in [
        "{\"type\":\"start_game\",\"type\":\"end_game\"}\n",
        "{\"type\":\"start_game\",\"extra\":{\"a\":1,\"a\":2}}\n",
    ] {
        let error = analyze(&mut Cursor::new(line), LogLimits::default()).unwrap_err();
        assert!(error.message.contains("line 1"), "{error}");
        assert!(error.message.contains("duplicate"), "{error}");
    }
}

#[test]
fn rejects_invalid_tile_actor_and_three_player_input() {
    for tile in ["0m", "10s", "4mr", "5z", "E ", "", "１m"] {
        assert!(Pai::new(tile).is_err(), "{tile}");
    }
    let mut values = complete();
    values[2]["actor"] = json!(4);
    assert!(inspect(&values).unwrap_err().message.contains("line 3"));
    values = complete();
    values[1]["tehais"].as_array_mut().unwrap().pop();
    assert!(inspect(&values).is_err());
}

#[test]
fn rejects_missing_end_and_events_after_completion() {
    let mut values = complete();
    values.pop();
    assert!(inspect(&values).unwrap_err().message.contains("end_game"));
    values = complete();
    values.push(json!({"type":"none"}));
    assert!(inspect(&values).unwrap_err().message.contains("line 12"));
    values = complete();
    values.remove(9);
    assert!(inspect(&values).is_err());
}

#[test]
fn semantic_failures_include_line_and_event_type() {
    let mut values = complete();
    values[4]["pai"] = json!("2s");
    let error = inspect(&values).unwrap_err();
    assert!(error.message.contains("line 5"), "{error}");
    assert!(error.message.contains("dahai"), "{error}");
    values = complete();
    values[2]["actor"] = json!(2);
    let error = inspect(&values).unwrap_err();
    assert!(error.message.contains("line 3"), "{error}");
}

#[test]
fn rejects_tedashi_when_the_only_matching_known_tile_was_just_drawn() {
    let mut values = complete();
    values[4]["tsumogiri"] = json!(false);
    let error = inspect(&values).unwrap_err();
    assert!(error.message.contains("line 5"));
    assert!(error.message.contains("tsumogiri"));
}

#[test]
fn unknown_tedashi_retains_the_known_just_drawn_tile() {
    for pai in ["5m", "5mr"] {
        let mut values = start();
        values[1]["tehais"] = json!([vec!["?"; 13], vec!["?"; 13], vec!["?"; 13], vec!["?"; 13]]);
        values.extend([
            json!({"type":"tsumo","actor":0,"pai":pai}),
            json!({"type":"dahai","actor":0,"pai":pai,"tsumogiri":false}),
            json!({"type":"end_game","_suzume":{"reason":"interrupted"}}),
        ]);
        visit(
            &mut Cursor::new(text(&values)),
            LogLimits::default(),
            |record, state| {
                if record.event.kind() == "dahai" {
                    let hand = state.observe(SeatId::new(0)).own_hand.unwrap();
                    assert_eq!(hand.len(), 13);
                    assert_eq!(
                        hand.iter().filter(|tile| tile.as_str() == pai).count(),
                        1,
                        "the known draw must remain after a hand discard"
                    );
                    assert_eq!(hand.iter().filter(|tile| tile.is_unknown()).count(), 12);
                }
                Ok(())
            },
        )
        .unwrap();
    }
}

#[test]
fn unknown_tsumogiri_consumes_the_known_just_drawn_tile() {
    let mut values = start();
    values[1]["tehais"] = json!([vec!["?"; 13], vec!["?"; 13], vec!["?"; 13], vec!["?"; 13]]);
    values.extend([
        json!({"type":"tsumo","actor":0,"pai":"5m"}),
        json!({"type":"dahai","actor":0,"pai":"5m","tsumogiri":true}),
        json!({"type":"end_game","_suzume":{"reason":"interrupted"}}),
    ]);
    visit(
        &mut Cursor::new(text(&values)),
        LogLimits::default(),
        |record, state| {
            if record.event.kind() == "dahai" {
                let hand = state.observe(SeatId::new(0)).own_hand.unwrap();
                assert_eq!(hand.len(), 13);
                assert!(hand.iter().all(Pai::is_unknown));
            }
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn tedashi_with_an_identical_known_copy_preserves_the_draw() {
    let mut values = start();
    values[1]["tehais"][0] =
        json!(["5m", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?"]);
    values.extend([
        json!({"type":"tsumo","actor":0,"pai":"5m"}),
        json!({"type":"dahai","actor":0,"pai":"5m","tsumogiri":false}),
        json!({"type":"end_game","_suzume":{"reason":"interrupted"}}),
    ]);
    visit(
        &mut Cursor::new(text(&values)),
        LogLimits::default(),
        |record, state| {
            if record.event.kind() == "dahai" {
                let hand = state.observe(SeatId::new(0)).own_hand.unwrap();
                assert_eq!(hand.iter().filter(|tile| tile.as_str() == "5m").count(), 1);
                assert_eq!(hand.iter().filter(|tile| tile.is_unknown()).count(), 12);
            }
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn ron_after_tsumo_is_rejected_without_changing_the_settled_state() {
    let mut values = start();
    values[1]["tehais"] = json!([vec!["?"; 13], vec!["?"; 13], vec!["?"; 13], vec!["?"; 13]]);
    values.extend([
        json!({"type":"tsumo","actor":0,"pai":"1m"}),
        json!({"type":"hora","actor":0,"target":0,"deltas":[3000,-1000,-1000,-1000]}),
    ]);
    let mut state = ReplayState::new();
    for (line, raw) in values.iter().enumerate() {
        state
            .apply(&ParsedEvent {
                line: line + 1,
                event: parse_event(&raw.to_string()).unwrap(),
                raw: raw.clone(),
            })
            .unwrap();
    }
    let before = state.observe(SeatId::new(0));
    let before_summary = state.summary().clone();
    let invalid = json!({"type":"hora","actor":1,"target":0,"deltas":[-1000,1000,0,0]});
    let error = state
        .apply(&ParsedEvent {
            line: 5,
            event: parse_event(&invalid.to_string()).unwrap(),
            raw: invalid.clone(),
        })
        .unwrap_err();
    assert!(error.message.contains("line 5"));
    assert_eq!(state.observe(SeatId::new(0)), before);
    assert_eq!(state.summary(), &before_summary);
    values.extend([
        invalid,
        json!({"type":"end_kyoku"}),
        json!({"type":"end_game"}),
    ]);
    assert!(inspect(&values).is_err());
}

#[test]
fn multiple_ron_on_one_discard_still_accumulates_each_settlement() {
    let mut values = complete();
    values.insert(
        9,
        json!({"type":"hora","actor":2,"target":1,"deltas":[0,-2000,2000,0]}),
    );
    let summary = inspect(&values).unwrap();
    assert_eq!(summary.wins, 2);
    assert_eq!(summary.final_scores, Some([33000, 15000, 27000, 25000]));
    assert_eq!(summary.riichi_sticks, Some(0));
}

#[test]
fn explicit_interruption_is_distinct_from_clean_completion_and_missing_footer() {
    let mut values = start();
    values.push(json!({"type":"end_game","_suzume":{"reason":"interrupted"}}));
    let summary = inspect(&values).unwrap();
    assert!(!summary.complete);
    assert_eq!(summary.termination, EndReason::Interrupted);
    values.last_mut().unwrap()["_suzume"]["reason"] = json!("truncated");
    assert_eq!(inspect(&values).unwrap().termination, EndReason::Truncated);
    values.last_mut().unwrap()["_suzume"]["reason"] = json!("completed");
    assert!(inspect(&values).is_err());
}

#[test]
fn observations_distinguish_input_end_from_natural_round_and_game_end() {
    for reason in ["interrupted", "truncated"] {
        let mut values = start();
        values.push(json!({"type":"end_game","_suzume":{"reason":reason}}));
        visit(
            &mut Cursor::new(text(&values)),
            LogLimits::default(),
            |record, state| {
                if record.event.kind() == "end_game" {
                    let observation = state.observe(None);
                    assert!(!observation.round_ended, "the active round was not settled");
                    assert!(
                        !observation.game_ended,
                        "an input interruption is not a natural game end"
                    );
                    assert!(observation.record_ended);
                }
                Ok(())
            },
        )
        .unwrap();
    }
    visit(
        &mut Cursor::new(text(&complete())),
        LogLimits::default(),
        |record, state| {
            let observation = state.observe(None);
            if record.line == 1 {
                assert!(!observation.round_ended);
                assert!(!observation.record_ended);
            }
            if record.event.kind() == "hora" {
                assert!(observation.round_ended);
                assert!(!observation.game_ended);
                assert!(!observation.record_ended);
            }
            if record.event.kind() == "end_game" {
                assert!(observation.round_ended);
                assert!(observation.game_ended);
                assert!(observation.record_ended);
            }
            Ok(())
        },
    )
    .unwrap();
    let mut values = complete();
    values.last_mut().unwrap()["_suzume"] = json!({"reason":"interrupted"});
    visit(
        &mut Cursor::new(text(&values)),
        LogLimits::default(),
        |record, state| {
            if record.event.kind() == "end_game" {
                let observation = state.observe(None);
                assert!(observation.round_ended);
                assert!(!observation.game_ended);
                assert!(observation.record_ended);
            }
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn missing_score_deltas_are_unknown_instead_of_assumed_zero() {
    let mut values = complete();
    values[8].as_object_mut().unwrap().remove("deltas");
    let summary = inspect(&values).unwrap();
    assert_eq!(summary.final_scores, None);
    assert_eq!(summary.riichi_sticks, None);
}

#[test]
fn rejects_duplicate_reach_acceptance_and_score_overflow() {
    let mut values = complete();
    values.insert(6, json!({"type":"reach_accepted","actor":0}));
    assert!(inspect(&values).is_err());
    values = complete();
    values[8]["deltas"] = json!([2147483647, -2147482647, 0, 0]);
    assert!(inspect(&values).is_err());
}

#[test]
fn supports_crlf_and_missing_last_newline() {
    let input = text(&complete()).trim_end().replace('\n', "\r\n");
    assert!(analyze(&mut Cursor::new(input), LogLimits::default()).is_ok());
}

#[test]
fn enforces_stream_limits_and_rejects_invalid_utf8() {
    let input = text(&complete());
    for limits in [
        LogLimits {
            max_line_bytes: 16,
            ..LogLimits::default()
        },
        LogLimits {
            max_records: 4,
            ..LogLimits::default()
        },
        LogLimits {
            max_total_bytes: 40,
            ..LogLimits::default()
        },
        LogLimits {
            max_line_bytes: 0,
            ..LogLimits::default()
        },
    ] {
        assert!(analyze(&mut Cursor::new(&input), limits).is_err());
    }
    assert!(analyze(&mut Cursor::new([0xff, b'\n']), LogLimits::default()).is_err());
    assert!(analyze(&mut Cursor::new("\n"), LogLimits::default()).is_err());
}

#[test]
fn does_not_call_consumer_after_an_invalid_event() {
    let mut values = complete();
    values[4]["pai"] = json!("2s");
    let mut calls = 0;
    assert!(visit(
        &mut Cursor::new(text(&values)),
        LogLimits::default(),
        |_, _| {
            calls += 1;
            Ok(())
        }
    )
    .is_err());
    assert_eq!(calls, 4);
}

#[test]
fn rejects_impossible_fifth_copy_in_known_initial_hands() {
    let mut values = complete();
    values[1]["tehais"][1] =
        json!(["1m", "1m", "1m", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?"]);
    assert!(inspect(&values).is_err());
}

#[test]
fn rejects_a_137th_retained_tile_including_unknowns_without_mutation() {
    let mut values = start();
    values[1]["tehais"] = json!([vec!["?"; 13], vec!["?"; 13], vec!["?"; 13], vec!["?"; 13]]);
    values[1]["dora_marker"] = json!("C");
    for turn in 0..84 {
        let kind = turn / 4;
        let pai = format!("{}{}", kind % 9 + 1, ["m", "p", "s"][kind / 9]);
        values.extend([
            json!({"type":"tsumo","actor":turn % 4,"pai":"?"}),
            json!({"type":"dahai","actor":turn % 4,"pai":pai,"tsumogiri":true}),
        ]);
    }
    values.push(json!({"type":"end_game","_suzume":{"reason":"interrupted"}}));

    // 52 concealed tiles + 83 uncalled discards + 1 indicator = 136.
    // This inventory check deliberately does not claim to enforce wall timing.
    let mut boundary = values[..168].to_vec();
    boundary.push(json!({"type":"end_game","_suzume":{"reason":"interrupted"}}));
    assert!(inspect(&boundary).is_ok());

    let mut state = ReplayState::new();
    for (line, raw) in values.iter().take(168).enumerate() {
        state
            .apply(&ParsedEvent {
                line: line + 1,
                event: parse_event(&raw.to_string()).unwrap(),
                raw: raw.clone(),
            })
            .unwrap();
    }
    let before: Vec<_> = (0..4)
        .map(|seat| state.observe(SeatId::new(seat)))
        .collect();
    let before_summary = state.summary().clone();
    let invalid = &values[168];
    let error = state
        .apply(&ParsedEvent {
            line: 169,
            event: parse_event(&invalid.to_string()).unwrap(),
            raw: invalid.clone(),
        })
        .unwrap_err();
    assert!(error.message.contains("136"), "{error}");
    assert_eq!(state.summary(), &before_summary);
    for (seat, observation) in before.into_iter().enumerate() {
        assert_eq!(state.observe(SeatId::new(seat as u8)), observation);
    }
    let error = inspect(&values).unwrap_err();
    assert!(error.message.contains("line 169 (tsumo)"), "{error}");
}

#[test]
fn counts_a_called_discard_only_in_the_meld_at_the_136_tile_boundary() {
    let mut values = start();
    values[1]["tehais"] = json!([vec!["?"; 13], vec!["?"; 13], vec!["?"; 13], vec!["?"; 13]]);
    values[1]["dora_marker"] = json!("C");
    values.extend([
        json!({"type":"tsumo","actor":0,"pai":"?"}),
        json!({"type":"dahai","actor":0,"pai":"1m","tsumogiri":true}),
        json!({"type":"pon","actor":1,"target":0,"pai":"1m","consumed":["1m","1m"]}),
        json!({"type":"dahai","actor":1,"pai":"2m","tsumogiri":false}),
    ]);
    for turn in 0..82 {
        let kind = turn / 4 + 2;
        let pai = format!("{}{}", kind % 9 + 1, ["m", "p", "s"][kind / 9]);
        values.extend([
            json!({"type":"tsumo","actor":(turn + 2) % 4,"pai":"?"}),
            json!({"type":"dahai","actor":(turn + 2) % 4,"pai":pai,"tsumogiri":true}),
        ]);
    }
    values.push(json!({"type":"end_game","_suzume":{"reason":"interrupted"}}));
    visit(
        &mut Cursor::new(text(&values)),
        LogLimits::default(),
        |record, state| {
            if record.event.kind() == "end_game" {
                let observation = state.observe(None);
                assert!(observation.players[0].discards[0].called);
                let concealed: usize = observation
                    .players
                    .iter()
                    .map(|p| usize::from(p.hand_count))
                    .sum();
                let melds: usize = observation
                    .players
                    .iter()
                    .flat_map(|p| &p.melds)
                    .map(|m| m.tiles.len())
                    .sum();
                let rivers = observation
                    .players
                    .iter()
                    .flat_map(|p| &p.discards)
                    .filter(|d| !d.called)
                    .count();
                assert_eq!(
                    concealed + melds + rivers + observation.dora_markers.len(),
                    136
                );
            }
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn rejected_late_settlement_does_not_mutate_the_accepted_prefix() {
    let values = complete();
    let mut state = ReplayState::new();
    for (line, raw) in values.iter().take(8).enumerate() {
        state
            .apply(&ParsedEvent {
                line: line + 1,
                event: parse_event(&raw.to_string()).unwrap(),
                raw: raw.clone(),
            })
            .unwrap();
    }
    let before = state.observe(SeatId::new(0));
    let before_summary = state.summary().clone();
    let invalid = json!({"type":"hora","actor":0,"target":1,"deltas":[0,2147483647,-2147482647,0]});
    assert!(state
        .apply(&ParsedEvent {
            line: 9,
            event: parse_event(&invalid.to_string()).unwrap(),
            raw: invalid
        })
        .is_err());
    assert_eq!(state.observe(SeatId::new(0)), before);
    assert_eq!(state.summary(), &before_summary);
    state
        .apply(&ParsedEvent {
            line: 9,
            event: parse_event(&values[8].to_string()).unwrap(),
            raw: values[8].clone(),
        })
        .unwrap();
    assert_eq!(
        state.summary().final_scores,
        Some([33000, 17000, 25000, 25000])
    );
}

#[test]
fn validates_native_final_deposit_payout_without_arbitrary_score_override() {
    let mut values = complete();
    values[8] = json!({"type":"ryukyoku","deltas":[0,0,0,0]});
    values[10] = json!({"type":"end_game","scores":[24000,26000,25000,25000],"_suzume":{"reason":"completed","kyotaku":0}});
    let summary = inspect(&values).unwrap();
    assert_eq!(summary.final_scores, Some([24000, 26000, 25000, 25000]));
    assert_eq!(summary.riichi_sticks, Some(0));
    values[10]["scores"] = json!([25000, 25000, 25000, 25000]);
    assert!(
        inspect(&values).is_err(),
        "a trailing player cannot receive the final deposit"
    );
    values[10]["scores"] = json!([24000, 27000, 25000, 25000]);
    assert!(inspect(&values).is_err(), "cannot create points");
    values[10]["scores"] = json!([24000, 26000, 25000, 25000]);
    values[10]["_suzume"]["kyotaku"] = json!(2);
    assert!(inspect(&values).is_err(), "cannot create deposits");
}

#[test]
fn rejects_meld_shapes_and_calls_from_the_wrong_seat() {
    let mut values = start();
    values.extend([
        json!({"type":"tsumo","actor":0,"pai":"9s"}),
        json!({"type":"dahai","actor":0,"pai":"1m","tsumogiri":false}),
        json!({"type":"pon","actor":1,"target":0,"pai":"1m","consumed":["1m","2m"]}),
        json!({"type":"end_game","_suzume":{"reason":"interrupted"}}),
    ]);
    assert!(inspect(&values).is_err());
    values[4] = json!({"type":"chi","actor":2,"target":0,"pai":"1m","consumed":["2m","3m"]});
    assert!(inspect(&values).is_err());
    values[4] = json!({"type":"pon","actor":1,"target":0,"pai":"1m","consumed":["1m","1m"]});
    assert_eq!(inspect(&values).unwrap().calls, 1);
}

#[test]
fn consumes_four_tiles_and_requires_replacement_draw_after_ankan() {
    let mut values = start();
    values[1]["tehais"][0] =
        json!(["1m", "1m", "1m", "1m", "4m", "5mr", "6m", "7m", "8m", "9m", "E", "P", "C"]);
    values.extend([
        json!({"type":"tsumo","actor":0,"pai":"9s"}),
        json!({"type":"ankan","actor":0,"consumed":["1m","1m","1m","1m"]}),
        json!({"type":"dora","dora_marker":"W"}),
        json!({"type":"tsumo","actor":0,"pai":"6s"}),
        json!({"type":"end_game","_suzume":{"reason":"interrupted"}}),
    ]);
    assert_eq!(inspect(&values).unwrap().calls, 1);
    values[5] = json!({"type":"dahai","actor":0,"pai":"9s","tsumogiri":true});
    assert!(inspect(&values).is_err());
}

#[test]
fn rejects_scores_changed_without_a_recorded_settlement_between_rounds() {
    let mut values = complete();
    values[10] = start()[1].clone();
    assert!(inspect(&values)
        .unwrap_err()
        .message
        .contains("scores disagree"));
}

#[test]
fn accepts_exact_limits_without_an_off_by_one_rejection() {
    let input = text(&complete());
    let limits = LogLimits {
        max_line_bytes: input.lines().map(str::len).max().unwrap(),
        max_total_bytes: input.len(),
        max_records: 11,
    };
    assert_eq!(
        analyze(&mut Cursor::new(input), limits).unwrap().records,
        11
    );
}

struct ErrorAtEof(Cursor<Vec<u8>>);
impl std::io::Read for ErrorAtEof {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buffer)
    }
}
impl std::io::BufRead for ErrorAtEof {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let bytes = self.0.fill_buf()?;
        if bytes.is_empty() {
            Err(std::io::Error::other("injected terminal read failure"))
        } else {
            Ok(bytes)
        }
    }
    fn consume(&mut self, count: usize) {
        self.0.consume(count);
    }
}

#[test]
fn requires_clean_eof_even_after_end_game() {
    let input = ErrorAtEof(Cursor::new(text(&complete()).into_bytes()));
    let error = analyze(&mut { input }, LogLimits::default()).unwrap_err();
    assert!(error.message.contains("terminal read failure"));
}

#[test]
fn propagates_consumer_failure_instead_of_returning_complete_success() {
    let error = visit(
        &mut Cursor::new(text(&complete())),
        LogLimits::default(),
        |_, _| {
            Err(suzume_api::types::ApiError::new(
                suzume_api::types::ErrorCode::LogError,
                "injected output failure",
            ))
        },
    )
    .unwrap_err();
    assert!(error.message.contains("line 1"));
    assert!(error.message.contains("injected output failure"));
}

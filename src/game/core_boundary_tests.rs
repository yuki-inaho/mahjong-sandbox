use super::*;

#[test]
fn action_labels_handle_invalid_indexes_without_panicking() {
    let hand = [Tile::new(0).unwrap()];
    for action in [
        Action::Discard(1),
        Action::Discard(usize::MAX),
        Action::Riichi(usize::MAX),
    ] {
        let label = std::panic::catch_unwind(|| action.label(&hand));
        assert!(
            label.is_ok(),
            "untrusted action labels must not index unchecked: {action:?}"
        );
        assert!(label.unwrap().contains("不正"));
    }
}

#[test]
fn seat_lookup_handles_the_full_usize_domain_without_panicking() {
    let game = Game::new(0).unwrap();
    for seat in [0, 3, 4, usize::MAX] {
        assert!(std::panic::catch_unwind(|| game.seat_wind(seat)).is_ok());
    }
    assert_eq!(game.seat_wind(4), None);
    assert_eq!(game.seat_wind(usize::MAX), None);
    assert_eq!(game.seat_wind(0).unwrap().id(), 27);
}

#[test]
fn validated_seat_rejects_before_narrowing_the_integer() {
    for value in 0..=255 {
        assert_eq!(Seat::new(value).is_some(), value < PLAYER_COUNT);
        assert_eq!(Seat::try_from(value).is_ok(), value < PLAYER_COUNT);
    }
    assert!(Seat::new(usize::MAX).is_none());
    for index in 0..PLAYER_COUNT {
        let seat = Seat::new(index).unwrap();
        assert_eq!(seat.index(), index);
        assert_eq!(usize::from(seat.id()), index);
    }
}

#[test]
fn committing_an_action_keeps_existing_log_storage() {
    let mut game = Game::new(42).unwrap();
    for _ in 0..60 {
        let seat = game.next_actor().unwrap();
        let action = game.legal_actions(seat).into_iter().next().unwrap();
        game.apply(seat, action).unwrap();
    }
    // 空き容量を先に確保し、履歴の再確保と全履歴の複製を区別します。
    // 時間の速さではなく、確定時に既存の追記用領域を維持する契約です。
    game.history.reserve(128);
    game.events.reserve(128);
    let history_pointer = game.history.as_ptr();
    let events_pointer = game.events.as_ptr();
    let history_before = game.history.clone();
    let events_before = game.events.clone();
    let seat = game.next_actor().unwrap();
    let action = game.legal_actions(seat).into_iter().next().unwrap();
    game.apply(seat, action.clone()).unwrap();
    assert_eq!(
        game.history.as_ptr(),
        history_pointer,
        "committing must not clone all previous actions"
    );
    assert_eq!(
        game.events.as_ptr(),
        events_pointer,
        "committing must not clone all previous event strings"
    );
    assert_eq!(&game.history[..history_before.len()], history_before);
    assert_eq!(&game.events[..events_before.len()], events_before);
    assert_eq!(
        game.history.last(),
        Some(&RecordedAction {
            player: seat,
            action
        })
    );
    let before_rejection = game.clone();
    assert!(game.apply(usize::MAX, Action::Discard(usize::MAX)).is_err());
    assert_eq!(game, before_rejection);
}

//! A deterministic eight-action recording used by the documented parser examples.
use suzume::{ai::ConfiguredAi, game::Game};
use suzume_api::{
    config::{AiConfigSpec, AppConfig, Controller, LogFormat, Preset, PublicViewer, Viewer},
    decision::DecisionView,
    logging::{EndReason, LogWriter},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = AppConfig::default();
    config.logging.format = LogFormat::Replay;
    config.game.max_steps = 8;
    config.game.viewer = Viewer::Public(PublicViewer::Public);
    config.players = std::array::from_fn(|seat| Controller::Ai {
        config: AiConfigSpec {
            preset: Some(Preset::Beginner),
            seed: (seat as u64).into(),
            ..Default::default()
        },
    });
    let mut game = Game::new(config.game.seed.value())?;
    let mut log = LogWriter::new(std::io::stdout().lock(), &config, &game)?;
    for _ in 0..config.game.max_steps {
        let actor = game.next_actor().ok_or("unexpected early match end")?;
        let Controller::Ai { config: ai } = &config.players[actor] else {
            unreachable!("all controllers are AI")
        };
        let decision = ConfiguredAi::new(ai.resolve()?)?
            .decide(&game.observation(actor)?, &game.legal_actions(actor))?;
        let wire_decision = DecisionView::from_core(&decision).ok_or("unrepresentable action")?;
        game.apply(actor, decision.action.clone())?;
        log.record(actor, &decision.action, &game, Some(&wire_decision))?;
    }
    drop(log.finish(&game, EndReason::Truncated)?);
    Ok(())
}

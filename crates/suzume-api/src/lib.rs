//! Versioned interfaces for applications, analysis and research.
//! The core remains independent of serialization, configuration and terminal I/O.
#![forbid(unsafe_code)]
pub mod actions;
pub mod config;
pub mod decision;
pub mod environment;
pub mod logging;
pub mod mjai;
pub mod protocol;
pub mod transport;
pub mod types;
pub mod view;

use config::{AppConfig, Controller};
use decision::DecisionView;
use environment::Environment;
use protocol::{MjaiFormat, Request, Response, ResponseData};
use suzume::{
    ai::ConfiguredAi,
    game::{Game, Phase},
};
use types::{ApiError, ErrorCode, Version};
use view::GameView;

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MAX_REQUEST_BYTES: usize = 1_048_576;

struct Session {
    game: Game,
    config: AppConfig,
    id: u32,
    revision: u32,
}
impl Session {
    fn view(&self) -> GameView {
        GameView::from_game(
            &self.game,
            self.config.game.viewer.seat(),
            self.id,
            self.revision,
        )
    }
    fn check(&self, id: u32, revision: u32) -> Result<(), ApiError> {
        check_revision(self.id, self.revision, id, revision)?;
        if self.revision >= self.config.game.max_steps
            && !matches!(self.game.phase(), Phase::MatchEnd)
        {
            return Err(ApiError::new(
                ErrorCode::StepLimit,
                "configured operation limit reached",
            ));
        }
        Ok(())
    }
}
fn check_revision(
    current_id: u32,
    current_revision: u32,
    id: u32,
    revision: u32,
) -> Result<(), ApiError> {
    if id != current_id {
        return Err(ApiError::new(
            ErrorCode::StaleSession,
            "session_id does not identify the current game",
        ));
    }
    if revision != current_revision {
        return Err(ApiError::new(
            ErrorCode::StaleRevision,
            "expected_revision does not match the current state",
        ));
    }
    Ok(())
}

/// Transport-independent dispatcher. Each instance owns one local application mode.
/// This is not a network authentication or multi-user authorization service.
#[derive(Default)]
pub struct Engine {
    session: Option<Session>,
    environment: Option<Environment>,
    last_id: u32,
}
impl Engine {
    pub fn new() -> Self {
        Self::default()
    }
    fn identifiers(&self) -> (Option<u32>, Option<u32>) {
        if let Some(s) = &self.session {
            (Some(s.id), Some(s.revision))
        } else if let Some(e) = &self.environment {
            (Some(e.session_id()), Some(e.revision()))
        } else {
            (None, None)
        }
    }
    pub fn request(&mut self, input: &str) -> String {
        let parsed = if input.len() > MAX_REQUEST_BYTES {
            Err(ApiError::new(
                ErrorCode::InvalidRequest,
                "request exceeds 1048576 bytes",
            ))
        } else {
            serde_json::from_str::<Request>(input)
                .map_err(|e| ApiError::new(ErrorCode::InvalidRequest, e.to_string()))
        };
        let result = parsed.and_then(|request| self.execute(request));
        let response = match result {
            Ok(data) => Response::Ok {
                version: Version,
                data: Box::new(data),
            },
            Err(error) => {
                let (session_id, revision) = self.identifiers();
                Response::Error {
                    version: Version,
                    code: error.code,
                    message: error.message,
                    session_id,
                    revision,
                }
            }
        };
        serde_json::to_string(&response)
            .expect("wire values contain no non-finite floats or non-string map keys")
    }
    fn next_id(&self) -> Result<u32, ApiError> {
        self.last_id.checked_add(1).ok_or_else(|| {
            ApiError::new(
                ErrorCode::StepLimit,
                "session identifier exhausted; create a new Engine",
            )
        })
    }
    fn start(&mut self, config: AppConfig) -> Result<ResponseData, ApiError> {
        config.validate()?;
        let id = self.next_id()?;
        let game = Game::new(config.game.seed.value())
            .map_err(|e| ApiError::new(ErrorCode::InternalError, e.to_string()))?;
        let session = Session {
            game,
            config,
            id,
            revision: 0,
        };
        let state = session.view();
        self.session = Some(session);
        self.environment = None;
        self.last_id = id;
        Ok(ResponseData::Game {
            state: Box::new(state),
        })
    }
    fn session_mut(&mut self) -> Result<&mut Session, ApiError> {
        self.session
            .as_mut()
            .ok_or_else(|| ApiError::new(ErrorCode::NoGame, "start an application game first"))
    }
    pub fn execute(&mut self, request: Request) -> Result<ResponseData, ApiError> {
        match request {
            Request::New { config, .. } => self.start(config),
            Request::LoadConfig { toml, .. } => self.start(AppConfig::from_toml(&toml)?),
            Request::InspectConfig { toml, .. } => {
                let config = AppConfig::from_toml(&toml)?;
                let effective_ai = config.effective_ai()?;
                Ok(ResponseData::Config {
                    config: Box::new(config),
                    effective_ai,
                })
            }
            Request::AnalyzeMjai {
                mjson,
                viewer,
                format,
                ..
            } => {
                if mjson.len() > MAX_REQUEST_BYTES {
                    return Err(ApiError::new(
                        ErrorCode::InvalidRequest,
                        "mjson exceeds 1048576 bytes; use the native file interface for larger records",
                    ));
                }
                let mut observation = None;
                let mut reader = std::io::Cursor::new(mjson.as_bytes());
                let limits = logging::LogLimits::default();
                let observe = |record: &mjai::ParsedEvent, state: &mjai::ReplayState| {
                    if matches!(record.event, mjai::Event::EndGame) {
                        observation = Some(Box::new(state.observe(viewer)));
                    }
                    Ok(())
                };
                let summary = match format {
                    MjaiFormat::Mjai => mjai::visit(&mut reader, limits, observe)?,
                    MjaiFormat::AnnotatedMjai => {
                        mjai::annotated::visit(&mut reader, limits, observe)?
                    }
                };
                Ok(ResponseData::Mjai {
                    summary,
                    observation: observation.ok_or_else(|| {
                        ApiError::new(ErrorCode::LogError, "mjson has no final observation")
                    })?,
                })
            }
            Request::View { .. } => {
                let session = self.session_mut()?;
                Ok(ResponseData::Game {
                    state: Box::new(session.view()),
                })
            }
            Request::Apply {
                session_id,
                expected_revision,
                action,
                ..
            } => {
                let session = self.session_mut()?;
                session.check(session_id, expected_revision)?;
                let viewer = session.config.game.viewer.seat().ok_or_else(|| {
                    ApiError::new(ErrorCode::WrongActor, "spectators cannot apply actions")
                })?;
                let actor = usize::from(viewer.get());
                if session.game.next_actor() != Some(actor)
                    || !matches!(session.config.players[actor], Controller::Human)
                {
                    return Err(ApiError::new(
                        ErrorCode::WrongActor,
                        "only the fixed human viewer may act on its turn",
                    ));
                }
                session
                    .game
                    .apply(actor, action.to_core())
                    .map_err(|e| ApiError::new(ErrorCode::IllegalAction, e.to_string()))?;
                session.revision += 1;
                Ok(ResponseData::Game {
                    state: Box::new(session.view()),
                })
            }
            Request::Step {
                session_id,
                expected_revision,
                ..
            } => {
                let session = self.session_mut()?;
                session.check(session_id, expected_revision)?;
                let actor = session
                    .game
                    .next_actor()
                    .ok_or_else(|| ApiError::new(ErrorCode::WrongActor, "game has ended"))?;
                let config = match &session.config.players[actor] {
                    Controller::Ai { config } => config.resolve()?,
                    Controller::Human => {
                        return Err(ApiError::new(
                            ErrorCode::WrongActor,
                            "human input is required",
                        ))
                    }
                };
                let ai = ConfiguredAi::new(config)
                    .map_err(|e| ApiError::new(ErrorCode::InvalidAi, e.to_string()))?;
                let observation = session
                    .game
                    .observation(actor)
                    .map_err(|e| ApiError::new(ErrorCode::InternalError, e.to_string()))?;
                let decision = ai
                    .decide(&observation, &session.game.legal_actions(actor))
                    .map_err(|e| ApiError::new(ErrorCode::InvalidAi, e.to_string()))?;
                session
                    .game
                    .apply(actor, decision.action)
                    .map_err(|e| ApiError::new(ErrorCode::IllegalAction, e.to_string()))?;
                session.revision += 1;
                Ok(ResponseData::Game {
                    state: Box::new(session.view()),
                })
            }
            Request::Analyze {
                session_id,
                expected_revision,
                config,
                ..
            } => {
                let session = self.session_mut()?;
                session.check(session_id, expected_revision)?;
                let viewer = session.config.game.viewer.seat().ok_or_else(|| {
                    ApiError::new(ErrorCode::WrongActor, "spectators have no private analysis")
                })?;
                let actor = usize::from(viewer.get());
                if session.game.next_actor() != Some(actor) {
                    return Err(ApiError::new(
                        ErrorCode::WrongActor,
                        "analysis is restricted to the viewer turn",
                    ));
                }
                let ai = ConfiguredAi::new(config.resolve()?)
                    .map_err(|e| ApiError::new(ErrorCode::InvalidAi, e.to_string()))?;
                let observation = session
                    .game
                    .observation(actor)
                    .map_err(|e| ApiError::new(ErrorCode::InternalError, e.to_string()))?;
                let decision = ai
                    .decide(&observation, &session.game.legal_actions(actor))
                    .map_err(|e| ApiError::new(ErrorCode::InvalidAi, e.to_string()))?;
                let decision = DecisionView::from_core(&decision).ok_or_else(|| {
                    ApiError::new(
                        ErrorCode::InternalError,
                        "decision is outside wire action domain",
                    )
                })?;
                Ok(ResponseData::Analysis {
                    session_id,
                    revision: session.revision,
                    decision,
                })
            }
            Request::EnvironmentReset {
                seed, max_steps, ..
            } => {
                let id = self.next_id()?;
                let environment = Environment::new(&seed, max_steps, id)?;
                let transition = environment.state();
                self.environment = Some(environment);
                self.session = None;
                self.last_id = id;
                Ok(ResponseData::Environment {
                    transition: Box::new(transition),
                })
            }
            Request::EnvironmentView { .. } => {
                let environment = self.environment.as_ref().ok_or_else(|| {
                    ApiError::new(ErrorCode::WrongMode, "reset the training environment first")
                })?;
                Ok(ResponseData::Environment {
                    transition: Box::new(environment.state()),
                })
            }
            Request::EnvironmentStep {
                session_id,
                expected_revision,
                action_id,
                ..
            } => {
                let environment = self.environment.as_mut().ok_or_else(|| {
                    ApiError::new(ErrorCode::WrongMode, "reset the training environment first")
                })?;
                check_revision(
                    environment.session_id(),
                    environment.revision(),
                    session_id,
                    expected_revision,
                )?;
                Ok(ResponseData::Environment {
                    transition: Box::new(environment.step(action_id)?),
                })
            }
        }
    }
}

#[cfg(test)]
mod mjai_api_tests {
    use super::*;
    use serde_json::{json, Value};

    fn record() -> String {
        let mut hands = vec![vec!["?"; 13]; 4];
        hands[1][0] = "9s";
        [
            json!({"type":"start_game"}),
            json!({"type":"start_kyoku","bakaze":"E","dora_marker":"1p","kyoku":1,
                "honba":0,"kyotaku":0,"oya":0,"scores":[25000,25000,25000,25000],"tehais":hands}),
            json!({"type":"tsumo","actor":0,"pai":"5mr"}),
            json!({"type":"dahai","actor":0,"pai":"5mr","tsumogiri":true}),
            json!({"type":"ryukyoku","deltas":[0,0,0,0]}),
            json!({"type":"end_kyoku"}),
            json!({"type":"end_game"}),
        ]
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
    }

    fn call(engine: &mut Engine, request: Value) -> Value {
        serde_json::from_str(&engine.request(&request.to_string())).unwrap()
    }

    #[test]
    fn mjai_analysis_preserves_session_and_redacts_other_hands() {
        let mut engine = Engine::new();
        let initial = call(&mut engine, json!({"version":1,"op":"new"}));
        let response = call(
            &mut engine,
            json!({"version":1,"op":"analyze_mjai","mjson":record(),"viewer":0}),
        );
        assert_eq!(response["status"], "ok", "{response}");
        assert_eq!(response["data"]["kind"], "mjai");
        assert_eq!(response["data"]["summary"]["records"], 7);
        assert_eq!(response["data"]["summary"]["rounds"], 1);
        assert_eq!(response["data"]["summary"]["red_events"], 2);
        assert_eq!(
            response["data"]["observation"]["own_hand"]
                .as_array()
                .unwrap()
                .len(),
            13
        );
        assert!(!response.to_string().contains("9s"));
        assert_eq!(call(&mut engine, json!({"version":1,"op":"view"})), initial);
        let public = call(
            &mut engine,
            json!({"version":1,"op":"analyze_mjai","mjson":record()}),
        );
        assert_eq!(public["status"], "ok");
        assert!(public["data"]["observation"]["own_hand"].is_null());
    }

    #[test]
    fn mjai_analysis_reports_input_position_without_mutating_environment() {
        let mut engine = Engine::new();
        let initial = call(
            &mut engine,
            json!({"version":1,"op":"environment_reset","seed":"1"}),
        );
        let response = call(
            &mut engine,
            json!({"version":1,"op":"analyze_mjai",
            "mjson":"{\"type\":\"start_game\"}\n{\"type\":\"tsumo\",\"actor\":0,\"pai\":\"1m\"}"}),
        );
        assert_eq!(response["code"], "log_error", "{response}");
        assert!(response["message"]
            .as_str()
            .unwrap()
            .contains("line 2 (tsumo)"));
        assert_eq!(
            call(&mut engine, json!({"version":1,"op":"environment_view"})),
            initial
        );
        let invalid = call(
            &mut engine,
            json!({"version":1,"op":"analyze_mjai","mjson":record(),"viewer":4}),
        );
        assert_eq!(invalid["code"], "invalid_request");
        let oversized = call(
            &mut engine,
            json!({"version":1,"op":"analyze_mjai","mjson":" ".repeat(MAX_REQUEST_BYTES)}),
        );
        assert_eq!(oversized["code"], "invalid_request");
    }
}

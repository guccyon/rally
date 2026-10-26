//! live-lesson: 同期レッスン（specs/context-map.yaml）。
//!
//! 同期レッスン（glossary の live_lesson）の通話セッション（[`LiveLessonSession`]）と、終了時に記録する
//! 接続品質の生の指標（[`ConnectionQuality`]）。通話品質スコアの式はここに置かない（セマンティック層の
//! 1 か所。docs/adr/0002 Q1）。

mod connection_quality;
mod live_lesson_session;
mod session_id;
mod session_status;

pub use connection_quality::ConnectionQuality;
pub use live_lesson_session::LiveLessonSession;
pub use session_id::SessionId;
pub use session_status::SessionStatus;

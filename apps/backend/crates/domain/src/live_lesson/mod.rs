//! live-lesson: 同期レッスン（specs/context-map.yaml）。
//!
//! 同期レッスン（glossary の live_lesson）の通話セッション（[`LiveLessonSession`]。結び付いた予約は [`SessionBooking`]）と、終了時に記録する
//! 接続品質の生の指標（[`ConnectionQuality`]）。通話品質スコアの式はここに置かない（セマンティック層の
//! 1 か所。docs/adr/0002 Q1）。本部が見る、終わっていないまま時間の過ぎたセッションの基準は [`OverdueSessionCriteria`]、
//! 期間とコーチ・生徒で見るセッションの記録の条件は [`SessionHistoryCriteria`]。

mod connection_quality;
mod live_lesson_session;
mod overdue_session_criteria;
mod session_booking;
mod session_history_criteria;
mod session_id;
mod session_status;

pub use connection_quality::ConnectionQuality;
pub use live_lesson_session::LiveLessonSession;
pub use overdue_session_criteria::OverdueSessionCriteria;
pub use session_booking::SessionBooking;
pub use session_history_criteria::SessionHistoryCriteria;
pub use session_id::SessionId;
pub use session_status::SessionStatus;

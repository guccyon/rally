//! messaging: メッセージ（specs/context-map.yaml）。
//!
//! 生徒と担当コーチの 1 対 1 のチャット。スレッド（glossary の message_thread。[`MessageThread`]）が集約で、
//! メッセージ（glossary の message。[`Message`]）の送信時刻と既読を持つ。生徒が送ってよいか（月額はチャットが
//! 送り放題）の規則は [`crate::entitlement::ContractType::unlimited_video_and_chat`] の 1 か所に置き、スレッドは
//! 答えに従う。通知（notification）と添付ファイルは扱わない。

mod message;
mod message_body;
mod message_id;
mod message_sender;
mod message_thread;
mod message_thread_id;

pub use message::Message;
pub use message_body::MessageBody;
pub use message_id::MessageId;
pub use message_sender::MessageSender;
pub use message_thread::MessageThread;
pub use message_thread_id::MessageThreadId;

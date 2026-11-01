//! messaging のユースケースとポート。

mod chat_gate;
mod list_message_threads;
mod list_messages;
mod list_unanswered_message_threads;
mod mark_message_thread_read;
pub mod message_id_uuid;
pub mod message_thread_id_uuid;
mod message_thread_subscriber;
mod message_thread_subscriber_error;
mod open_message_thread;
mod repository;
mod send_message;
mod thread_coach_display_name_lookup;
mod thread_student_display_name_lookup;
mod unanswered_thread_list;
mod update_message_thread_error;

pub use chat_gate::ChatGate;
pub use list_message_threads::{
    ListMessageThreads, ListMessageThreadsError, ListMessageThreadsQuery,
};
pub use list_messages::{ListMessages, ListMessagesError, ListMessagesQuery};
pub use list_unanswered_message_threads::ListUnansweredMessageThreads;
pub use mark_message_thread_read::{
    MarkMessageThreadRead, MarkMessageThreadReadCommand, MarkMessageThreadReadError,
};
pub use message_thread_subscriber::MessageThreadSubscriber;
pub use message_thread_subscriber_error::MessageThreadSubscriberError;
pub use open_message_thread::{
    OpenMessageThread, OpenMessageThreadCommand, OpenMessageThreadError,
};
pub use repository::MessageThreadRepository;
pub use send_message::{SendMessage, SendMessageCommand, SendMessageError, SentMessage};
pub use thread_coach_display_name_lookup::ThreadCoachDisplayNameLookup;
pub use thread_student_display_name_lookup::ThreadStudentDisplayNameLookup;
pub use unanswered_thread_list::UnansweredThreadList;
pub use update_message_thread_error::UpdateMessageThreadError;

//! 生徒と担当コーチのチャット（`/message-threads`）。送信は entitlement
//! にチャットを送ってよいかを問い合わせる（`EntitlementChatGate`。照会だけで回数を消費しない）。

use application::{
    entitlement::EntitlementChatGate,
    messaging::{ListMessageThreads, ListMessages, MarkMessageThreadRead, SendMessage},
    profile::ProfileStudentDisplayNameLookup,
};
use axum::Router;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::messaging_router(
        ListMessageThreads::new(
            ports.threads(),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
        ),
        ListMessages::new(ports.threads()),
        SendMessage::new(
            ports.threads(),
            EntitlementChatGate::new(ports.entitlements()),
            ports.analytics(),
            ports.events(),
            ports.clock(),
        ),
        MarkMessageThreadRead::new(ports.threads(), ports.clock()),
        ports.tokens(),
    )
}

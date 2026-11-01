//! 生徒と担当コーチのチャット（`/message-threads`）。送信は entitlement
//! にチャットを送ってよいかを問い合わせる（`EntitlementChatGate`。照会だけで回数を消費しない）。

use application::{
    entitlement::EntitlementChatGate,
    messaging::{ListMessageThreads, ListMessages, MarkMessageThreadRead, SendMessage},
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::messaging_router(
        ListMessageThreads::new(ports.threads()),
        ListMessages::new(ports.threads()),
        SendMessage::new(
            ports.threads(),
            EntitlementChatGate::new(ports.entitlements()),
            ports.analytics(),
            ports.events(),
            ports.clock(),
        ),
        MarkMessageThreadRead::new(ports.threads(), ports.clock()),
        MockTokenService,
    )
}

//! レッスンの完了記録（`POST /lessons/completions`）。通話セッション・動画の保存先と同じ中身を読み、`LessonCompleted`
//! を同じイベントバスで発行する。

use application::lesson::RecordLessonCompletion;
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::lessons_router(
        RecordLessonCompletion::new(
            ports.sessions(),
            ports.videos(),
            ports.completions(),
            ports.analytics(),
            ports.events(),
            ports.clock(),
        ),
        MockTokenService,
    )
}

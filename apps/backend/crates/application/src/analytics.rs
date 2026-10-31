//! 行動イベント（specs/analytics/events.yaml）を発火するポート。
//!
//! Gen1 の行動トラッキングはモック。発火の失敗でユースケースを失敗させないので、戻り値はない。

use std::{future::Future, time::SystemTime};

use domain::{
    async_lesson::{VideoId, VideoReviewId},
    catalog::Sport,
    entitlement::SubscriptionId,
    identity::AccountId,
    lesson::{LessonCompletionId, LessonType},
    live_lesson::SessionId,
    messaging::{MessageId, MessageSender, MessageThreadId},
    review::{LessonReviewId, Rating},
    scheduling::{BookingId, SlotId},
};

/// 行動イベント。変種は events.yaml の `name` に 1 対 1 で対応し、属性は `required` と同じ。共通の任意属性
/// （`common_optional`。`release_id`・`app_version`）はイベントごとに変わらないので変種に持たせず、配信先が付ける。
///
/// `call.ended` のパケット損失率が浮動小数点数なので `Eq` は実装しない。
#[derive(Debug, Clone, PartialEq)]
pub enum AnalyticsEvent {
    /// `account.registered`
    AccountRegistered {
        student_id: AccountId,
        sport: Sport,
        occurred_at: SystemTime,
    },
    /// `slot.published`（KPI「コーチの予約枠充足率」の入力）
    SlotPublished {
        coach_id: AccountId,
        slot_id: SlotId,
        starts_at: SystemTime,
        occurred_at: SystemTime,
    },
    /// `booking.created`（体験予約を含む。KPI「初回レッスンまでの日数」の入力）
    BookingCreated {
        student_id: AccountId,
        coach_id: AccountId,
        booking_id: BookingId,
        slot_id: SlotId,
        is_trial: bool,
        occurred_at: SystemTime,
    },
    /// `booking.confirmed`（KPI「コーチの予約枠充足率」の入力）
    BookingConfirmed {
        student_id: AccountId,
        coach_id: AccountId,
        booking_id: BookingId,
        slot_id: SlotId,
        occurred_at: SystemTime,
    },
    /// `booking.cancelled`（KPI「コーチの予約枠充足率」の入力）。回数を消費するかの判定は持たない
    /// （境界間イベント `BookingCancelled` の `consumes_quota` に載せる）
    BookingCancelled {
        student_id: AccountId,
        coach_id: AccountId,
        booking_id: BookingId,
        slot_id: SlotId,
        /// キャンセルした時刻から開始時刻までの時間（時間単位で切り捨て。開始後のキャンセルは負）
        hours_before_start: i64,
        occurred_at: SystemTime,
    },
    /// `call.started`
    CallStarted {
        student_id: AccountId,
        coach_id: AccountId,
        booking_id: BookingId,
        session_id: SessionId,
        occurred_at: SystemTime,
    },
    /// `call.ended`（KPI「通話品質スコア」の入力）。接続品質の生の指標だけを持ち、スコアの式はセマンティック層の
    /// 1 か所に置く（docs/adr/0002 Q1）
    CallEnded {
        student_id: AccountId,
        coach_id: AccountId,
        booking_id: BookingId,
        session_id: SessionId,
        /// 通話の長さ（秒。切り捨て）
        duration_sec: u64,
        /// 失われたパケットの百分率（0〜100）
        packet_loss_rate: f64,
        jitter_ms: u32,
        round_trip_ms: u32,
        disconnect_count: u32,
        occurred_at: SystemTime,
    },
    /// `video.submitted`（KPI「添削の返却リードタイム」の入力）
    VideoSubmitted {
        student_id: AccountId,
        coach_id: AccountId,
        video_id: VideoId,
        sport: Sport,
        occurred_at: SystemTime,
    },
    /// `video_review.returned`（KPI「添削の返却リードタイム」の入力）。リードタイムをセマンティック層で
    /// `occurred_at - submitted_at` と計算できるよう、動画を送付した時刻も持つ
    VideoReviewReturned {
        student_id: AccountId,
        coach_id: AccountId,
        video_id: VideoId,
        review_id: VideoReviewId,
        /// 動画を送付した時刻（`video.submitted` の `occurred_at` と同じ）
        submitted_at: SystemTime,
        occurred_at: SystemTime,
    },
    /// `lesson.completed`（KPI「初回レッスンまでの日数」の入力。`is_first_lesson` は north star
    /// 「初回レッスン後の 1 ヶ月継続率」の起点）
    LessonCompleted {
        student_id: AccountId,
        coach_id: AccountId,
        lesson_id: LessonCompletionId,
        lesson_type: LessonType,
        /// その生徒にとって初回のレッスンか（domain の `LessonCompletion` が判定する）
        is_first_lesson: bool,
        occurred_at: SystemTime,
    },
    /// `subscription.started`（KPI「初回レッスン後の 1 ヶ月継続率」の入力）
    SubscriptionStarted {
        student_id: AccountId,
        subscription_id: SubscriptionId,
        sport: Sport,
        occurred_at: SystemTime,
    },
    /// `message.sent`（生徒と担当コーチのチャット。どちらが送ったかは `sender_role`）
    MessageSent {
        student_id: AccountId,
        coach_id: AccountId,
        thread_id: MessageThreadId,
        message_id: MessageId,
        /// 送ったのがスレッドの生徒かコーチか
        sender_role: MessageSender,
        occurred_at: SystemTime,
    },
    /// `review.submitted`（生徒が完了したレッスンに付けた評価）
    ReviewSubmitted {
        student_id: AccountId,
        coach_id: AccountId,
        lesson_id: LessonCompletionId,
        review_id: LessonReviewId,
        rating: Rating,
        occurred_at: SystemTime,
    },
}

impl AnalyticsEvent {
    /// events.yaml の `name`。
    pub const fn name(&self) -> &'static str {
        match self {
            AnalyticsEvent::AccountRegistered { .. } => "account.registered",
            AnalyticsEvent::SlotPublished { .. } => "slot.published",
            AnalyticsEvent::BookingCreated { .. } => "booking.created",
            AnalyticsEvent::BookingConfirmed { .. } => "booking.confirmed",
            AnalyticsEvent::BookingCancelled { .. } => "booking.cancelled",
            AnalyticsEvent::CallStarted { .. } => "call.started",
            AnalyticsEvent::CallEnded { .. } => "call.ended",
            AnalyticsEvent::VideoSubmitted { .. } => "video.submitted",
            AnalyticsEvent::VideoReviewReturned { .. } => "video_review.returned",
            AnalyticsEvent::LessonCompleted { .. } => "lesson.completed",
            AnalyticsEvent::SubscriptionStarted { .. } => "subscription.started",
            AnalyticsEvent::MessageSent { .. } => "message.sent",
            AnalyticsEvent::ReviewSubmitted { .. } => "review.submitted",
        }
    }
}

pub trait AnalyticsSink: Send + Sync {
    fn emit(&self, event: AnalyticsEvent) -> impl Future<Output = ()> + Send;
}

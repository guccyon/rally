//! 利用者に見せる文言（API の `message`）。domain の値やエラーの変種から組み立てる。

use application::scheduling::slot_id_uuid;
use domain::{
    DomainError,
    identity::Role,
    lesson::LessonType,
    messaging::{MessageContent, Stamp},
    scheduling::{BookingStatus, SlotStatus},
};

use crate::{contract::sport_to_contract, entitlement_error};

/// 役割の呼び名。
pub(crate) const fn role_label(role: Role) -> &'static str {
    match role {
        Role::Student => "生徒",
        Role::Coach => "コーチ",
        Role::Headquarters => "本部",
    }
}

/// 枠に結びつける・動画で選んだレッスンメニューがない（admin-web の `publishSlot`・student-app の `submitVideo` の 422）。
pub(crate) const LESSON_MENU_NOT_FOUND: &str = "指定したレッスンメニューがない";

/// 不変条件の違反の説明。
pub(crate) fn domain_error(error: &DomainError) -> String {
    match error {
        DomainError::EmptyDisplayName => "表示名が空".to_string(),
        DomainError::DisplayNameTooLong { max } => format!("表示名は {max} 文字まで"),
        DomainError::InvalidEmail => "メールアドレスの形が不正".to_string(),
        DomainError::CoachBioTooLong { max } => format!("自己紹介文は {max} 文字まで"),
        DomainError::RoleAttributeMismatch { role, sports } => {
            format!("{}にスポーツ {sports} 個は持てない", role_label(*role))
        }
        DomainError::SlotEndsBeforeItStarts => "スロットの開始が終了より前でない".to_string(),
        DomainError::SlotOverlaps { existing } => format!(
            "同じコーチの枠（{}）と期間が重なる",
            slot_id_uuid::format(*existing)
        ),
        DomainError::SlotOfAnotherCoach => "別のコーチの枠は扱えない".to_string(),
        DomainError::SlotNotOpen { status } => {
            format!("スロットが公開中でない（{}）", slot_status_label(*status))
        }
        DomainError::SlotCapacityOutOfRange { min, max } => {
            format!("スロットの定員は {min}〜{max} の整数")
        }
        DomainError::SlotNotBooked { status } => {
            format!("スロットが予約済みでない（{}）", slot_status_label(*status))
        }
        DomainError::SlotAlreadyBooked => "スロットはすでに予約されている".to_string(),
        DomainError::SlotAlreadyBookedByStudent => "このスロットはすでに予約している".to_string(),
        DomainError::SlotSeatsOverCapacity { capacity, active } => {
            format!("スロットの有効な予約は定員 {capacity} まで（{active} 件）")
        }
        DomainError::SlotStatusMismatchesSeats { status } => format!(
            "スロットの状態と予約の数が合わない（{}）",
            slot_status_label(*status)
        ),
        DomainError::TrialAlreadyUsed => "体験予約は初回の予約だけ".to_string(),
        DomainError::BookingNotPending { status } => {
            format!("確定待ちの予約でない（{}）", booking_status_label(*status))
        }
        DomainError::BookingAlreadyCancelled => "予約はキャンセル済み".to_string(),
        DomainError::BookingOfAnotherStudent => "別の生徒の予約は扱えない".to_string(),
        DomainError::BookingOfAnotherSlot => "予約と別のスロットは扱えない".to_string(),
        DomainError::LiveLessonQuotaExceeded => {
            entitlement_error::LIVE_LESSON_QUOTA_EXCEEDED.to_string()
        }
        DomainError::NotASessionParticipant => "予約の参加者でない".to_string(),
        DomainError::BookingNotConfirmed { status } => {
            format!("確定した予約でない（{}）", booking_status_label(*status))
        }
        DomainError::LiveLessonSessionAlreadyStarted => {
            "この予約の通話セッションはすでにある".to_string()
        }
        DomainError::LiveLessonSessionAlreadyEnded => "通話セッションは終了済み".to_string(),
        DomainError::SessionEndsBeforeItStarts => "通話セッションの終了が開始より前".to_string(),
        DomainError::InvalidPacketLossRate => "パケット損失率は 0〜100（%）".to_string(),
        DomainError::NotAVideoParty => "動画の当事者でない".to_string(),
        DomainError::NotTheVideoCoach => "動画の送り先のコーチでない".to_string(),
        DomainError::VideoReviewAlreadyReturned => "動画の添削は返却済み".to_string(),
        DomainError::VideoReviewNotAllowed { sport } => format!(
            "スポーツ {} の動画は決済が確定するまで添削に着手できない",
            sport_to_contract(*sport)
        ),
        DomainError::ReviewReturnedBeforeSubmission => "添削の返却が動画の送付より前".to_string(),
        DomainError::EmptyReviewComment => "添削コメントが空".to_string(),
        DomainError::ReviewCommentTooLong { max } => format!("添削コメントは {max} 文字まで"),
        DomainError::LiveLessonSessionNotEnded => "通話セッションがまだ終了していない".to_string(),
        DomainError::EmptyAiFeedback => "AI の一次フィードバックが空".to_string(),
        DomainError::AiFeedbackAlreadyRecorded => "AI の一次フィードバックは付与済み".to_string(),
        DomainError::AiFeedbackBeforeSubmission => {
            "AI の一次フィードバックの作成が動画の送付より前".to_string()
        }
        DomainError::VideoReviewNotReturned => "動画の添削がまだ返されていない".to_string(),
        DomainError::CompletionBeforeLessonEnded => {
            "レッスンの完了の記録がレッスンの終わりより前".to_string()
        }
        DomainError::LessonAlreadyCompleted => "このレッスンの完了は記録済み".to_string(),
        DomainError::EmptyLessonMenuName => "レッスンメニューの名前が空".to_string(),
        DomainError::LessonMenuNameTooLong { max } => {
            format!("レッスンメニューの名前は {max} 文字まで")
        }
        DomainError::LessonMenuDescriptionTooLong { max } => {
            format!("レッスンメニューの説明は {max} 文字まで")
        }
        DomainError::LessonDurationOutOfRange { min, max, step } => {
            format!("1 回の時間は {min}〜{max} 分の {step} 分刻み")
        }
        DomainError::LessonDurationMismatchesType { lesson_type } => match lesson_type {
            LessonType::Live => "同期レッスンには 1 回の時間が要る".to_string(),
            LessonType::Async => "非同期レッスンは 1 回の時間を持てない".to_string(),
        },
        DomainError::SportNotOfferedByCoach { sport } => format!(
            "コーチの提供スポーツでない（{}）",
            sport_to_contract(*sport)
        ),
        DomainError::NotTheLessonMenuCoach => "レッスンメニューのコーチでない".to_string(),
        DomainError::LessonMenuOfAnotherCoach => {
            "枠に結びつけられるのは自分のレッスンメニューだけ".to_string()
        }
        DomainError::LessonMenuNotPublished => {
            "公開中でないレッスンメニューは枠に結びつけられない".to_string()
        }
        DomainError::LessonMenuNotLive => {
            "非同期レッスンのメニューは枠に結びつけられない（同期レッスンのメニューだけ）"
                .to_string()
        }
        DomainError::LessonMenuSportMismatch { menu, slot } => format!(
            "レッスンメニューのスポーツ（{}）が枠のスポーツ（{}）と違う",
            sport_to_contract(*menu),
            sport_to_contract(*slot)
        ),
        DomainError::SlotLengthMismatchesLessonMenu { minutes } => {
            format!("枠の長さがレッスンメニューの 1 回の時間（{minutes} 分）と違う")
        }
        DomainError::VideoLessonMenuOfAnotherCoach => {
            "動画で選べるのは送り先のコーチのレッスンメニューだけ".to_string()
        }
        DomainError::VideoLessonMenuNotPublished => {
            "公開中でないレッスンメニューは動画で選べない".to_string()
        }
        DomainError::LessonMenuNotAsync => {
            "同期レッスンのメニューは動画で選べない（非同期レッスンのメニューだけ）".to_string()
        }
        DomainError::VideoLessonMenuSportMismatch { menu, video } => format!(
            "レッスンメニューのスポーツ（{}）が動画のスポーツ（{}）と違う",
            sport_to_contract(*menu),
            sport_to_contract(*video)
        ),
    }
}

/// メッセージの本文（student-app・admin-web の `Message.body`）。本文の種類は本文そのもの、添削依頼・添削の返却・
/// スタンプは種類（`Message.kind`）を知らない版のクライアントがそのまま表示できる短い説明（スタンプは「スタンプ: 呼び名」）。
pub(crate) fn message_content_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(body) => body.as_str().to_string(),
        MessageContent::VideoReviewRequest { .. } => "動画の添削を依頼しました".to_string(),
        MessageContent::VideoReviewReturn { .. } => "動画の添削を返しました".to_string(),
        MessageContent::Stamp(stamp) => format!("スタンプ: {}", stamp_label(*stamp)),
    }
}

/// スタンプの呼び名（契約の `StampId` の説明と同じ）。
const fn stamp_label(stamp: Stamp) -> &'static str {
    match stamp {
        Stamp::Thanks => "ありがとう",
        Stamp::Good => "いいね",
        Stamp::GotIt => "了解",
        Stamp::LetsGo => "がんばろう",
        Stamp::NiceShot => "ナイスショット",
        Stamp::Tired => "おつかれさま",
        Stamp::Question => "質問",
        Stamp::Ok => "OK",
    }
}

/// スロットの状態の呼び名（`specs/glossary.yaml` の `slot` の `states`。admin-web の画面と同じ）。
const fn slot_status_label(status: SlotStatus) -> &'static str {
    match status {
        SlotStatus::Open => "公開中",
        SlotStatus::Booked => "予約済み",
        SlotStatus::Closed => "終了",
    }
}

/// 予約の状態の呼び名（`specs/glossary.yaml` の `booking` の `states`。両アプリの画面と同じ）。
const fn booking_status_label(status: BookingStatus) -> &'static str {
    match status {
        BookingStatus::Pending => "確定待ち",
        BookingStatus::Confirmed => "確定済み",
        BookingStatus::Cancelled => "キャンセル済み",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::catalog::Sport;

    #[test]
    fn describes_each_domain_error() {
        assert_eq!(domain_error(&DomainError::EmptyDisplayName), "表示名が空");
        assert_eq!(
            domain_error(&DomainError::DisplayNameTooLong { max: 50 }),
            "表示名は 50 文字まで"
        );
        assert_eq!(
            domain_error(&DomainError::InvalidEmail),
            "メールアドレスの形が不正"
        );
        assert_eq!(
            domain_error(&DomainError::CoachBioTooLong { max: 400 }),
            "自己紹介文は 400 文字まで"
        );
        assert_eq!(
            domain_error(&DomainError::RoleAttributeMismatch {
                role: Role::Student,
                sports: 2
            }),
            "生徒にスポーツ 2 個は持てない"
        );
        assert_eq!(
            domain_error(&DomainError::SlotEndsBeforeItStarts),
            "スロットの開始が終了より前でない"
        );
        assert_eq!(
            domain_error(&DomainError::SlotOverlaps {
                existing: domain::scheduling::SlotId::from_u128(1)
            }),
            "同じコーチの枠（00000000-0000-0000-0000-000000000001）と期間が重なる"
        );
        assert_eq!(
            domain_error(&DomainError::SlotOfAnotherCoach),
            "別のコーチの枠は扱えない"
        );
        assert_eq!(
            domain_error(&DomainError::SlotNotOpen {
                status: SlotStatus::Booked
            }),
            "スロットが公開中でない（予約済み）"
        );
        assert_eq!(
            domain_error(&DomainError::SlotNotOpen {
                status: SlotStatus::Closed
            }),
            "スロットが公開中でない（終了）"
        );
        assert_eq!(
            domain_error(&DomainError::SlotAlreadyBooked),
            "スロットはすでに予約されている"
        );
        assert_eq!(
            domain_error(&DomainError::SlotAlreadyBookedByStudent),
            "このスロットはすでに予約している"
        );
        assert_eq!(
            domain_error(&DomainError::SlotCapacityOutOfRange { min: 1, max: 6 }),
            "スロットの定員は 1〜6 の整数"
        );
        assert_eq!(
            domain_error(&DomainError::SlotSeatsOverCapacity {
                capacity: 1,
                active: 2
            }),
            "スロットの有効な予約は定員 1 まで（2 件）"
        );
        assert_eq!(
            domain_error(&DomainError::SlotStatusMismatchesSeats {
                status: SlotStatus::Booked
            }),
            "スロットの状態と予約の数が合わない（予約済み）"
        );
        assert_eq!(
            domain_error(&DomainError::TrialAlreadyUsed),
            "体験予約は初回の予約だけ"
        );
        assert_eq!(
            domain_error(&DomainError::BookingNotPending {
                status: BookingStatus::Confirmed
            }),
            "確定待ちの予約でない（確定済み）"
        );
        assert_eq!(
            domain_error(&DomainError::SlotNotBooked {
                status: SlotStatus::Open
            }),
            "スロットが予約済みでない（公開中）"
        );
        assert_eq!(
            domain_error(&DomainError::BookingAlreadyCancelled),
            "予約はキャンセル済み"
        );
        assert_eq!(
            domain_error(&DomainError::BookingOfAnotherStudent),
            "別の生徒の予約は扱えない"
        );
        assert_eq!(
            domain_error(&DomainError::BookingOfAnotherSlot),
            "予約と別のスロットは扱えない"
        );
        assert_eq!(
            domain_error(&DomainError::LiveLessonQuotaExceeded),
            "今月のビデオ通話の枠を使い切っている"
        );
        assert_eq!(
            domain_error(&DomainError::NotASessionParticipant),
            "予約の参加者でない"
        );
        assert_eq!(
            domain_error(&DomainError::BookingNotConfirmed {
                status: BookingStatus::Pending
            }),
            "確定した予約でない（確定待ち）"
        );
        assert_eq!(
            domain_error(&DomainError::LiveLessonSessionAlreadyStarted),
            "この予約の通話セッションはすでにある"
        );
        assert_eq!(
            domain_error(&DomainError::LiveLessonSessionAlreadyEnded),
            "通話セッションは終了済み"
        );
        assert_eq!(
            domain_error(&DomainError::SessionEndsBeforeItStarts),
            "通話セッションの終了が開始より前"
        );
        assert_eq!(
            domain_error(&DomainError::InvalidPacketLossRate),
            "パケット損失率は 0〜100（%）"
        );
        assert_eq!(
            domain_error(&DomainError::NotAVideoParty),
            "動画の当事者でない"
        );
        assert_eq!(
            domain_error(&DomainError::NotTheVideoCoach),
            "動画の送り先のコーチでない"
        );
        assert_eq!(
            domain_error(&DomainError::VideoReviewAlreadyReturned),
            "動画の添削は返却済み"
        );
        assert_eq!(
            domain_error(&DomainError::VideoReviewNotAllowed { sport: Sport::Golf }),
            "スポーツ golf の動画は決済が確定するまで添削に着手できない"
        );
        assert_eq!(
            domain_error(&DomainError::ReviewReturnedBeforeSubmission),
            "添削の返却が動画の送付より前"
        );
        assert_eq!(
            domain_error(&DomainError::EmptyReviewComment),
            "添削コメントが空"
        );
        assert_eq!(
            domain_error(&DomainError::ReviewCommentTooLong { max: 4000 }),
            "添削コメントは 4000 文字まで"
        );
        assert_eq!(
            domain_error(&DomainError::LiveLessonSessionNotEnded),
            "通話セッションがまだ終了していない"
        );
        assert_eq!(
            domain_error(&DomainError::VideoReviewNotReturned),
            "動画の添削がまだ返されていない"
        );
        assert_eq!(
            domain_error(&DomainError::CompletionBeforeLessonEnded),
            "レッスンの完了の記録がレッスンの終わりより前"
        );
        assert_eq!(
            domain_error(&DomainError::LessonAlreadyCompleted),
            "このレッスンの完了は記録済み"
        );
        assert_eq!(
            domain_error(&DomainError::EmptyLessonMenuName),
            "レッスンメニューの名前が空"
        );
        assert_eq!(
            domain_error(&DomainError::LessonMenuNameTooLong { max: 40 }),
            "レッスンメニューの名前は 40 文字まで"
        );
        assert_eq!(
            domain_error(&DomainError::LessonMenuDescriptionTooLong { max: 400 }),
            "レッスンメニューの説明は 400 文字まで"
        );
        assert_eq!(
            domain_error(&DomainError::LessonDurationOutOfRange {
                min: 15,
                max: 120,
                step: 15
            }),
            "1 回の時間は 15〜120 分の 15 分刻み"
        );
        assert_eq!(
            domain_error(&DomainError::LessonDurationMismatchesType {
                lesson_type: LessonType::Live
            }),
            "同期レッスンには 1 回の時間が要る"
        );
        assert_eq!(
            domain_error(&DomainError::LessonDurationMismatchesType {
                lesson_type: LessonType::Async
            }),
            "非同期レッスンは 1 回の時間を持てない"
        );
        assert_eq!(
            domain_error(&DomainError::SportNotOfferedByCoach {
                sport: domain::catalog::Sport::Golf
            }),
            "コーチの提供スポーツでない（golf）"
        );
        assert_eq!(
            domain_error(&DomainError::NotTheLessonMenuCoach),
            "レッスンメニューのコーチでない"
        );
    }
    #[test]
    fn describes_each_message_content_as_a_body() {
        use domain::{
            async_lesson::{VideoId, VideoReviewId},
            messaging::MessageBody,
        };
        assert_eq!(
            message_content_text(&MessageContent::Text(
                MessageBody::new("こんにちは").unwrap()
            )),
            "こんにちは"
        );
        assert_eq!(
            message_content_text(&MessageContent::VideoReviewRequest {
                video_id: VideoId::from_u128(1)
            }),
            "動画の添削を依頼しました"
        );
        assert_eq!(
            message_content_text(&MessageContent::VideoReviewReturn {
                video_id: VideoId::from_u128(1),
                review_id: VideoReviewId::from_u128(2)
            }),
            "動画の添削を返しました"
        );
        assert_eq!(
            Stamp::ALL.map(|stamp| message_content_text(&MessageContent::Stamp(stamp))),
            [
                "スタンプ: ありがとう",
                "スタンプ: いいね",
                "スタンプ: 了解",
                "スタンプ: がんばろう",
                "スタンプ: ナイスショット",
                "スタンプ: おつかれさま",
                "スタンプ: 質問",
                "スタンプ: OK",
            ]
        );
    }
}

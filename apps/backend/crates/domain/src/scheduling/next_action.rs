use std::time::{Duration, SystemTime};

use super::{Booking, BookingId, BookingStatus, Slot};
use crate::{
    async_lesson::Video,
    identity::AccountId,
    lesson::{LessonCompletion, LessonSource},
    live_lesson::{LiveLessonSession, SessionStatus},
};

/// 生徒が次にできる操作の 1 件（glossary にない語。ADR 0010「生徒アプリの次の操作はサーバーの状態から出す」）。
///
/// 決め方は [`NextAction::decide`] の 1 か所だけに置く（student-app の契約 `getMyNextActions` の説明が正）。
/// アプリの中の段や端末の状態は使わず、予約の状態と時刻・通話セッションの終了・動画の添削の状態・完了したレッスンの
/// 記録の有無・受講権の有無だけから決める。完了したかは完了記録（[`LessonCompletion`]）だけで決め、セッションの終了や
/// 添削の返却から推し量らない（ADR 0006）。読むだけで何も変えない（ビデオ通話の枠も消費しない。ADR 0005）。
#[derive(Debug, Clone, PartialEq)]
pub enum NextAction {
    /// 確定待ちの予約を確定する（契約の `confirm_booking`）
    ConfirmBooking { booking: Booking, slot: Slot },
    /// 確定済みの同期レッスンの予約で、入室前チェックとビデオ通話に進む（契約の `join_live_lesson`）
    JoinLiveLesson { booking: Booking, slot: Slot },
    /// 終了した通話セッションに完了の記録がない（契約の同期の `record_lesson_completion`）。`slot` は生徒の予約した枠
    RecordLiveLessonCompletion {
        session: LiveLessonSession,
        slot: Slot,
    },
    /// 添削が返った動画に完了の記録がない（契約の非同期の `record_lesson_completion`）
    RecordAsyncLessonCompletion { video: Video },
    /// 送った動画をコーチが添削している（契約の `video_review_in_progress`）
    VideoReviewInProgress { video: Video },
    /// 完了したレッスンがあり、受講権がない（契約の `start_subscription`）
    StartSubscription,
}

impl NextAction {
    /// 返す次の操作の数の上限。
    pub const LIMIT: usize = 3;

    /// 入室前チェックを開けるのは、予約した枠の開始のこの時間前から（生徒アプリの `prejoinCheckLeadTime` と同じ）。
    pub const PREJOIN_CHECK_LEAD: Duration = Duration::from_secs(5 * 60);

    /// 生徒 `student_id` の次の操作を、優先の順に [`Self::LIMIT`] 件まで決める（`now` の時点）。
    ///
    /// - `bookings`: 生徒の予約（状態を問わない）と予約した枠
    /// - `sessions`: 生徒の予約が結び付いた通話セッション（終了の有無を問わない）
    /// - `videos`: 生徒が送った動画（添削の状態を問わない）
    /// - `completions`: 生徒の完了記録（同期・非同期の両方）
    /// - `has_entitlement`: 生徒が受講権を持つか
    ///
    /// どの入力も、生徒 `student_id` のものでなければ使わない（他の生徒の予約・動画・記録は返さない）。順は次のとおりで、
    /// 同じ種類が複数あれば、予約は開始時刻の近い順、動画は送った・返った時刻の古い順（同じ時刻は ID の順）。
    ///
    /// 1. 確定待ちの予約 → [`NextAction::ConfirmBooking`]
    /// 2. 確定済みで終わっていない予約のうち、開始の [`Self::PREJOIN_CHECK_LEAD`] 前を過ぎたもの → [`NextAction::JoinLiveLesson`]
    /// 3. 終了した通話セッション・添削が返った動画のうち、完了の記録がないもの（終わった・返った時刻の古い順） →
    ///    [`NextAction::RecordLiveLessonCompletion`]・[`NextAction::RecordAsyncLessonCompletion`]
    /// 4. 確定済みの予約のうち、まだ開始の [`Self::PREJOIN_CHECK_LEAD`] 前より前のいちばん近い 1 件 → [`NextAction::JoinLiveLesson`]
    /// 5. 添削が返っていない動画 → [`NextAction::VideoReviewInProgress`]
    /// 6. 完了の記録が 1 件以上あり、受講権がない → [`NextAction::StartSubscription`]
    ///
    /// 予約が「終わった」のは、結び付いた通話セッションが終了したとき、またはセッションが進行中でないまま枠の終了時刻を
    /// 過ぎたとき（進行中のセッションがあれば、枠の終了時刻を過ぎても入り直せる）。
    pub fn decide(
        student_id: AccountId,
        bookings: &[(Booking, Slot)],
        sessions: &[LiveLessonSession],
        videos: &[Video],
        completions: &[LessonCompletion],
        has_entitlement: bool,
        now: SystemTime,
    ) -> Vec<NextAction> {
        let mut bookings: Vec<&(Booking, Slot)> = bookings
            .iter()
            .filter(|(booking, slot)| {
                booking.student_id() == student_id && booking.slot_id() == slot.id()
            })
            .collect();
        bookings.sort_by_key(|(booking, slot)| (slot.starts_at(), booking.id()));
        let sessions: Vec<(BookingId, &LiveLessonSession)> = sessions
            .iter()
            .filter_map(|session| {
                session
                    .booking_of(student_id)
                    .map(|booking_id| (booking_id, session))
            })
            .collect();
        let session_of = |booking: &Booking| {
            sessions
                .iter()
                .find(|(booking_id, _)| *booking_id == booking.id())
                .map(|(_, session)| *session)
        };
        let completions: Vec<LessonSource> = completions
            .iter()
            .filter(|completion| completion.student_id() == student_id)
            .map(LessonCompletion::source)
            .collect();
        let mut videos: Vec<&Video> = videos
            .iter()
            .filter(|video| video.student_id() == student_id)
            .collect();
        videos.sort_by_key(|video| (video.submitted_at(), video.id()));

        let pending = bookings
            .iter()
            .filter(|(booking, _)| booking.status() == BookingStatus::Pending)
            .map(|(booking, slot)| NextAction::ConfirmBooking {
                booking: booking.clone(),
                slot: slot.clone(),
            });

        let (joinable, upcoming): (Vec<_>, Vec<_>) = bookings
            .iter()
            .filter(|(booking, slot)| {
                booking.status() == BookingStatus::Confirmed
                    && !is_over(slot, session_of(booking), now)
            })
            .partition(|(_, slot)| now + Self::PREJOIN_CHECK_LEAD >= slot.starts_at());
        let join = |(booking, slot): &&(Booking, Slot)| NextAction::JoinLiveLesson {
            booking: booking.clone(),
            slot: slot.clone(),
        };

        let mut unrecorded: Vec<(SystemTime, NextAction)> = Vec::new();
        for (booking_id, session) in &sessions {
            let ended_at = match (session.status(), session.ended_at()) {
                (SessionStatus::Ended, Some(ended_at)) => ended_at,
                _ => continue,
            };
            if completions.contains(&LessonSource::Live(session.id())) {
                continue;
            }
            // セッションに結び付いた生徒の予約の枠（開始・終了の時刻を載せる）。予約が読めなければ出さない
            let Some((_, slot)) = bookings
                .iter()
                .find(|(booking, _)| booking.id() == *booking_id)
            else {
                continue;
            };
            unrecorded.push((
                ended_at,
                NextAction::RecordLiveLessonCompletion {
                    session: (*session).clone(),
                    slot: slot.clone(),
                },
            ));
        }
        for video in &videos {
            let Some(review) = video.review() else {
                continue;
            };
            if completions.contains(&LessonSource::Async(video.id())) {
                continue;
            }
            unrecorded.push((
                review.returned_at(),
                NextAction::RecordAsyncLessonCompletion {
                    video: (*video).clone(),
                },
            ));
        }
        // 同じ時刻は同期（セッションは開始した予約の順）・非同期（動画は送った順）の順のまま（安定な並べ替え）
        unrecorded.sort_by_key(|(at, _)| *at);

        let in_review = videos
            .iter()
            .filter(|video| video.review().is_none())
            .map(|video| NextAction::VideoReviewInProgress {
                video: (*video).clone(),
            });

        let subscribe =
            (!completions.is_empty() && !has_entitlement).then_some(NextAction::StartSubscription);

        pending
            .chain(joinable.iter().map(join))
            .chain(unrecorded.into_iter().map(|(_, action)| action))
            .chain(upcoming.first().map(join))
            .chain(in_review)
            .chain(subscribe)
            .take(Self::LIMIT)
            .collect()
    }

    /// 次の操作に載るコーチ。[`NextAction::StartSubscription`] はコーチを持たない。
    pub fn coach_id(&self) -> Option<AccountId> {
        match self {
            NextAction::ConfirmBooking { booking, .. }
            | NextAction::JoinLiveLesson { booking, .. } => Some(booking.coach_id()),
            NextAction::RecordLiveLessonCompletion { session, .. } => Some(session.coach_id()),
            NextAction::RecordAsyncLessonCompletion { video }
            | NextAction::VideoReviewInProgress { video } => Some(video.coach_id()),
            NextAction::StartSubscription => None,
        }
    }
}

/// 予約した枠 `slot` の同期レッスンが `now` の時点で終わったか。結び付いたセッション `session` が終了していれば
/// 終わり、進行中なら終わっていない。セッションがなければ枠の終了時刻で決める。
fn is_over(slot: &Slot, session: Option<&LiveLessonSession>, now: SystemTime) -> bool {
    match session.map(LiveLessonSession::status) {
        Some(SessionStatus::Ended) => true,
        Some(SessionStatus::InProgress) => false,
        None => now >= slot.ends_at(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        async_lesson::{ReviewComment, VideoId, VideoReviewId},
        catalog::Sport,
        lesson::LessonCompletionId,
        live_lesson::{ConnectionQuality, SessionId},
        scheduling::{SlotCapacity, SlotId},
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(9);

    /// 基準の時刻から `minutes` 分後
    fn at(minutes: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + minutes * 60)
    }

    /// `student` が予約した、`starts`〜`starts + 60` 分の枠 `id`（`confirmed` なら確定済み）。予約 ID は枠と同じ数。
    fn booking(id: u128, student: AccountId, starts: u64, confirmed: bool) -> (Booking, Slot) {
        let mut slot = Slot::publish(
            SlotId::from_u128(id),
            COACH,
            Sport::Tennis,
            at(starts),
            at(starts + 60),
            SlotCapacity::new(1).unwrap(),
        )
        .unwrap();
        let mut booking = Booking::reserve(
            BookingId::from_u128(id),
            student,
            false,
            &mut slot,
            &[],
            at(0),
        )
        .unwrap();
        if confirmed {
            booking.confirm(student, &mut slot, at(0)).unwrap();
        }
        (booking, slot)
    }

    /// 予約 `booked` の通話セッション `id`（`started` 分に開始し、`ended` があればその分に終了）。
    fn session(
        id: u128,
        booked: &(Booking, Slot),
        started: u64,
        ended: Option<u64>,
    ) -> LiveLessonSession {
        let mut session = LiveLessonSession::start(
            SessionId::from_u128(id),
            COACH,
            &booked.0,
            &[],
            None,
            at(started),
        )
        .unwrap();
        if let Some(ended) = ended {
            session
                .end(
                    COACH,
                    at(ended),
                    ConnectionQuality::new(0.0, 0, 50, 0).unwrap(),
                )
                .unwrap();
        }
        session
    }

    /// `student` が `submitted` 分に送った動画 `id`（`returned` があればその分に添削が返った）。
    fn video(id: u128, student: AccountId, submitted: u64, returned: Option<u64>) -> Video {
        let mut video = Video::submit(
            VideoId::from_u128(id),
            student,
            COACH,
            Sport::Tennis,
            at(submitted),
        );
        if let Some(returned) = returned {
            video
                .return_review(
                    VideoReviewId::from_u128(id),
                    COACH,
                    ReviewComment::new("よい").unwrap(),
                    true,
                    at(returned),
                )
                .unwrap();
        }
        video
    }

    fn completion(id: u128, student: AccountId, source: LessonSource) -> LessonCompletion {
        LessonCompletion::restore(
            LessonCompletionId::from_u128(id),
            student,
            COACH,
            source,
            at(0),
            id == 1,
        )
    }

    /// 次の操作を `(種類, ID の数)` に短くする（予約は予約 ID、セッションはセッション ID、動画は動画 ID）。
    fn kinds(actions: &[NextAction]) -> Vec<(&'static str, u128)> {
        actions
            .iter()
            .map(|action| match action {
                NextAction::ConfirmBooking { booking, .. } => {
                    ("confirm_booking", booking.id().as_u128())
                }
                NextAction::JoinLiveLesson { booking, .. } => {
                    ("join_live_lesson", booking.id().as_u128())
                }
                NextAction::RecordLiveLessonCompletion { session, .. } => {
                    ("record_live", session.id().as_u128())
                }
                NextAction::RecordAsyncLessonCompletion { video } => {
                    ("record_async", video.id().as_u128())
                }
                NextAction::VideoReviewInProgress { video } => {
                    ("video_review_in_progress", video.id().as_u128())
                }
                NextAction::StartSubscription => ("start_subscription", 0),
            })
            .collect()
    }

    fn decide(
        bookings: &[(Booking, Slot)],
        sessions: &[LiveLessonSession],
        videos: &[Video],
        completions: &[LessonCompletion],
        has_entitlement: bool,
        now: u64,
    ) -> Vec<(&'static str, u128)> {
        kinds(&NextAction::decide(
            STUDENT,
            bookings,
            sessions,
            videos,
            completions,
            has_entitlement,
            at(now),
        ))
    }

    #[test]
    fn a_student_without_records_has_no_next_action() {
        assert!(decide(&[], &[], &[], &[], false, 0).is_empty());
    }

    #[test]
    fn pending_bookings_are_confirmed_nearest_start_first() {
        let bookings = [
            booking(2, STUDENT, 300, false),
            booking(1, STUDENT, 200, false),
        ];
        assert_eq!(
            decide(&bookings, &[], &[], &[], false, 10),
            [("confirm_booking", 1), ("confirm_booking", 2)]
        );
    }

    #[test]
    fn a_confirmed_booking_is_joinable_from_five_minutes_before_its_start() {
        let bookings = [booking(1, STUDENT, 100, true)];
        for now in [0, 94, 95, 100, 159] {
            assert_eq!(
                decide(&bookings, &[], &[], &[], false, now),
                [("join_live_lesson", 1)],
                "{now} 分"
            );
        }
        assert!(
            decide(&bookings, &[], &[], &[], false, 160).is_empty(),
            "セッションがないまま枠が終わったら出さない"
        );
    }

    #[test]
    fn only_the_nearest_booking_before_its_prejoin_check_is_listed() {
        let bookings = [
            booking(1, STUDENT, 100, true),
            booking(2, STUDENT, 200, true),
            booking(3, STUDENT, 300, true),
        ];
        assert_eq!(
            decide(&bookings, &[], &[], &[], false, 0),
            [("join_live_lesson", 1)]
        );
        assert_eq!(
            decide(&bookings, &[], &[], &[], false, 96),
            [("join_live_lesson", 1), ("join_live_lesson", 2)],
            "入室前チェックを開ける予約のあとに、次の予約を 1 件"
        );
    }

    #[test]
    fn an_in_progress_session_keeps_the_lesson_joinable_and_an_ended_one_asks_for_completion() {
        let booked = booking(1, STUDENT, 100, true);
        let in_progress = session(50, &booked, 100, None);
        assert_eq!(
            decide(
                std::slice::from_ref(&booked),
                &[in_progress],
                &[],
                &[],
                false,
                170
            ),
            [("join_live_lesson", 1)],
            "進行中なら枠の終了時刻を過ぎても入り直せる"
        );

        let ended = session(50, &booked, 100, Some(150));
        assert_eq!(
            decide(
                std::slice::from_ref(&booked),
                std::slice::from_ref(&ended),
                &[],
                &[],
                true,
                155
            ),
            [("record_live", 50)],
            "終了したら入室ではなく完了の記録"
        );
        assert!(
            decide(
                &[booked],
                &[ended],
                &[],
                &[completion(
                    1,
                    STUDENT,
                    LessonSource::Live(SessionId::from_u128(50))
                )],
                true,
                155,
            )
            .is_empty(),
            "完了の記録があれば出さない"
        );
    }

    #[test]
    fn returned_videos_without_completion_ask_for_it_and_others_wait_for_review() {
        let videos = [
            video(62, STUDENT, 30, None),
            video(61, STUDENT, 20, Some(40)),
            video(60, STUDENT, 10, None),
        ];
        assert_eq!(
            decide(&[], &[], &videos, &[], true, 50),
            [
                ("record_async", 61),
                ("video_review_in_progress", 60),
                ("video_review_in_progress", 62),
            ]
        );
        let recorded = [completion(
            1,
            STUDENT,
            LessonSource::Async(VideoId::from_u128(61)),
        )];
        assert_eq!(
            decide(&[], &[], &videos, &recorded, true, 50),
            [
                ("video_review_in_progress", 60),
                ("video_review_in_progress", 62),
            ],
            "完了の記録の有無はサーバーの記録だけで決める"
        );
    }

    #[test]
    fn unrecorded_lessons_are_listed_oldest_end_first_whatever_their_type() {
        let booked = booking(1, STUDENT, 100, true);
        let ended = session(50, &booked, 100, Some(150));
        let videos = [
            video(60, STUDENT, 0, Some(120)),
            video(61, STUDENT, 0, Some(200)),
        ];
        assert_eq!(
            decide(&[booked], &[ended], &videos, &[], true, 210),
            [
                ("record_async", 60),
                ("record_live", 50),
                ("record_async", 61)
            ]
        );
    }

    #[test]
    fn a_student_with_completions_but_no_entitlement_is_asked_to_subscribe() {
        let completions = [completion(
            1,
            STUDENT,
            LessonSource::Async(VideoId::from_u128(60)),
        )];
        assert_eq!(
            decide(&[], &[], &[], &completions, false, 0),
            [("start_subscription", 0)]
        );
        assert!(
            decide(&[], &[], &[], &completions, true, 0).is_empty(),
            "受講権があれば出さない"
        );
        assert!(
            decide(&[], &[], &[], &[], false, 0).is_empty(),
            "完了したレッスンがなければ出さない"
        );
    }

    #[test]
    fn actions_follow_the_contract_order_and_are_capped_at_three() {
        // 確定待ち・入室できる予約・完了の記録なし・次の予約・添削待ち・月額契約なしが全部ある
        let joinable = booking(2, STUDENT, 100, true);
        let finished = booking(3, STUDENT, 0, true);
        let bookings = [
            booking(1, STUDENT, 500, false),
            joinable,
            finished.clone(),
            booking(4, STUDENT, 400, true),
        ];
        let sessions = [session(50, &finished, 0, Some(60))];
        let videos = [video(60, STUDENT, 0, None)];
        let completions = [completion(
            1,
            STUDENT,
            LessonSource::Async(VideoId::from_u128(99)),
        )];
        let all = NextAction::decide(
            STUDENT,
            &bookings,
            &sessions,
            &videos,
            &completions,
            false,
            at(96),
        );
        assert_eq!(
            kinds(&all),
            [
                ("confirm_booking", 1),
                ("join_live_lesson", 2),
                ("record_live", 50),
            ]
        );

        // 上の 3 つが片づくと、次の予約・添削待ち・月額契約なしが出る
        let rest = decide(&bookings[3..], &[], &videos, &completions, false, 96);
        assert_eq!(
            rest,
            [
                ("join_live_lesson", 4),
                ("video_review_in_progress", 60),
                ("start_subscription", 0),
            ]
        );
    }

    #[test]
    fn cancelled_bookings_are_not_listed() {
        let (mut booked, mut slot) = booking(1, STUDENT, 100, false);
        booked.cancel(STUDENT, &mut slot, at(1)).unwrap();
        assert!(decide(&[(booked, slot)], &[], &[], &[], false, 10).is_empty());
    }

    #[test]
    fn records_of_other_students_are_never_used() {
        let others = booking(1, OTHER_STUDENT, 100, true);
        let others_session = session(50, &others, 100, Some(150));
        let actions = decide(
            &[booking(2, OTHER_STUDENT, 300, false), others],
            &[others_session],
            &[
                video(60, OTHER_STUDENT, 0, None),
                video(61, OTHER_STUDENT, 0, Some(10)),
            ],
            &[completion(
                1,
                OTHER_STUDENT,
                LessonSource::Async(VideoId::from_u128(61)),
            )],
            false,
            155,
        );
        assert!(actions.is_empty(), "{actions:?}");
    }

    #[test]
    fn coach_of_each_action() {
        let booked = booking(1, STUDENT, 100, true);
        let actions = NextAction::decide(
            STUDENT,
            &[booked],
            &[],
            &[video(60, STUDENT, 0, None)],
            &[completion(
                1,
                STUDENT,
                LessonSource::Async(VideoId::from_u128(99)),
            )],
            false,
            at(0),
        );
        assert_eq!(
            actions.iter().map(NextAction::coach_id).collect::<Vec<_>>(),
            [Some(COACH), Some(COACH), None]
        );
    }
}

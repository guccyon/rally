//! Postgres の保存先（完了記録・通話セッション・予約・動画）で組み立てた `LessonCompletionMenuLookup`（review の
//! `ReviewedLessonMenuLookup`）の検証。評価した完了したレッスンのメニューが、元の予約（グループレッスンでは生徒ごとの
//! 予約）・動画に保存した写しから読めることと、元にメニューがなければ `None` になること。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::{
    async_lesson::VideoRepository,
    lesson::{LessonCompletionMenuLookup, LessonCompletionRepository},
    live_lesson::LiveLessonSessionRepository,
    review::ReviewedLessonMenuLookup,
    scheduling::{BookingRepository, SchedulingBookingLookup, SlotRepository},
};
use domain::{
    async_lesson::{ReviewComment, Video},
    catalog::Sport,
    catalog::{
        LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
        LessonMenuId, LessonMenuName,
    },
    identity::AccountId,
    lesson::LessonCompletion,
    live_lesson::LiveLessonSession,
    review::ReviewLessonMenu,
    scheduling::{Booking, Slot, SlotCapacity},
};
use infra::{
    PgBookingRepository, PgLessonCompletionRepository, PgLiveLessonSessionRepository,
    PgSlotRepository, PgVideoRepository,
};
use sqlx::PgPool;
use support::at;

type Lookup = LessonCompletionMenuLookup<
    PgLessonCompletionRepository,
    PgLiveLessonSessionRepository,
    SchedulingBookingLookup<PgBookingRepository>,
    PgVideoRepository,
>;

fn lookup(pool: &PgPool) -> Lookup {
    LessonCompletionMenuLookup::new(
        PgLessonCompletionRepository::new(pool.clone()),
        PgLiveLessonSessionRepository::new(pool.clone()),
        SchedulingBookingLookup::new(PgBookingRepository::new(pool.clone())),
        PgVideoRepository::new(pool.clone()),
    )
}

/// コーチ `coach` の公開中のテニスのメニュー（メニューは catalog の持ち物で、写しは catalog の行がなくても保存できる）。
fn menu(id: u128, coach: AccountId, name: &str, format: LessonFormat) -> LessonMenu {
    LessonMenu::restore(
        LessonMenuId::from_u128(id),
        coach,
        LessonMenuContent {
            sport: Sport::Tennis,
            format,
            name: LessonMenuName::new(name).unwrap(),
            description: LessonMenuDescription::default(),
        },
        true,
        at(0),
    )
}

fn review_menu(menu: &LessonMenu) -> ReviewLessonMenu {
    ReviewLessonMenu::new(menu.id(), menu.name().clone())
}

/// `student` が `slot` を予約して（メニュー `menu` があれば写して）確定した予約。
async fn confirmed_booking(
    pool: &PgPool,
    student: AccountId,
    slot: &Slot,
    menu: Option<&LessonMenu>,
) -> Booking {
    let bookings = PgBookingRepository::new(pool.clone());
    let id = bookings.next_id();
    let booking = bookings
        .reserve(slot.id(), student, |held, existing| {
            let booking = Booking::reserve(id, student, false, held, existing, at(0))?;
            Ok(booking.with_lesson_menu(slot, menu))
        })
        .await
        .unwrap();
    bookings
        .update(booking.id(), |booking, slot| {
            booking.confirm(student, slot, at(0))
        })
        .await
        .unwrap()
}

/// 生徒の完了記録を保存する。
async fn record(
    pool: &PgPool,
    student: AccountId,
    complete: impl FnOnce(
        domain::lesson::LessonCompletionId,
        &[LessonCompletion],
    ) -> Result<LessonCompletion, domain::DomainError>
    + Send,
) -> LessonCompletion {
    let completions = PgLessonCompletionRepository::new(pool.clone());
    let id = completions.next_id();
    completions
        .record(student, |history| complete(id, history))
        .await
        .unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_live_lesson_has_the_menu_copied_to_the_booking_of_its_student(pool: PgPool) {
    let coach = support::coach(&pool).await;
    let sixty = LessonFormat::Live(LessonDuration::from_minutes(60).unwrap());
    let group_menu = menu(0x50, coach, "グループ練習", sixty);
    let slots = PgSlotRepository::new(pool.clone());
    let slot = Slot::publish(
        slots.next_id(),
        coach,
        Sport::Tennis,
        at(10),
        at(11),
        SlotCapacity::new(2).unwrap(),
    )
    .unwrap()
    .with_lesson_menu(&group_menu)
    .unwrap();
    slots
        .add_to_calendar(slot.clone(), |calendar, slot| calendar.publish(slot))
        .await
        .unwrap();
    // 1 人目は予約した時点のメニューを写し、2 人目はメニューを引けなかった予約（写しがない）
    let (first, second) = (support::student(&pool).await, support::student(&pool).await);
    let bookings = [
        confirmed_booking(&pool, first, &slot, Some(&group_menu)).await,
        confirmed_booking(&pool, second, &slot, None).await,
    ];
    let sessions = PgLiveLessonSessionRepository::new(pool.clone());
    let session = LiveLessonSession::start(
        sessions.next_id(),
        coach,
        &bookings[0],
        &bookings,
        None,
        at(10),
    )
    .unwrap();
    sessions.insert(session.clone()).await.unwrap();
    let session = sessions
        .update(session.id(), |s| s.end(coach, at(11), support::quality()))
        .await
        .unwrap();

    let lookup = lookup(&pool);
    let with_menu = record(&pool, first, |id, history| {
        session.complete_for(id, coach, first, history, at(12))
    })
    .await;
    assert_eq!(
        lookup.find_lesson_menu(with_menu.id()).await.unwrap(),
        Some(review_menu(&group_menu))
    );
    let without_menu = record(&pool, second, |id, history| {
        session.complete_for(id, coach, second, history, at(12))
    })
    .await;
    assert_eq!(
        lookup.find_lesson_menu(without_menu.id()).await.unwrap(),
        None,
        "グループレッスンでも、開始した予約ではなくその生徒の予約の写し"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn an_async_lesson_has_the_menu_copied_to_its_video(pool: PgPool) {
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let async_menu = menu(0x51, coach, "フォーム添削", LessonFormat::Async);
    let videos = PgVideoRepository::new(pool.clone());
    let lookup = lookup(&pool);
    for menu in [Some(&async_menu), None] {
        let mut video = Video::submit(videos.next_video_id(), student, coach, Sport::Tennis, at(1));
        if let Some(menu) = menu {
            video = video.with_lesson_menu(menu).unwrap();
        }
        videos.insert(video.clone()).await.unwrap();
        let review_id = videos.next_review_id();
        let video = videos
            .update(video.id(), |v| {
                v.return_review(
                    review_id,
                    coach,
                    ReviewComment::new("よい").unwrap(),
                    true,
                    at(2),
                )
            })
            .await
            .unwrap();
        let completion = record(&pool, student, |id, history| {
            video.complete(id, student, history, at(3))
        })
        .await;

        assert_eq!(
            lookup.find_lesson_menu(completion.id()).await.unwrap(),
            menu.map(review_menu)
        );
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn an_unknown_lesson_has_no_menu(pool: PgPool) {
    let completions = PgLessonCompletionRepository::new(pool.clone());
    assert_eq!(
        lookup(&pool)
            .find_lesson_menu(completions.next_id())
            .await
            .unwrap(),
        None
    );
}

//! スキーマの検証。空の DB にマイグレーションを当て、DB で守る不変条件を確かめる。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`apps/backend/README.md` の手順で
//! Postgres を起動し、`DATABASE_URL` を設定して `cargo test -p infra -- --include-ignored` で走らせる。
//! `#[sqlx::test]` はテストごとに新しい DB を作って `infra::MIGRATOR` を当てる。

use sqlx::{PgPool, types::Uuid};

/// 制約違反の SQLSTATE と制約名を取り出す。
fn violation<T: std::fmt::Debug>(result: Result<T, sqlx::Error>) -> (String, String) {
    match result {
        Err(sqlx::Error::Database(e)) => (
            e.code().map(|c| c.into_owned()).unwrap_or_default(),
            e.constraint().unwrap_or_default().to_owned(),
        ),
        other => panic!("制約違反になるはずが {other:?}"),
    }
}

const UNIQUE_VIOLATION: &str = "23505";
const FOREIGN_KEY_VIOLATION: &str = "23503";
const CHECK_VIOLATION: &str = "23514";

async fn account(pool: &PgPool, role: &str, sport: Option<&str>) -> String {
    sqlx::query_scalar(
        "INSERT INTO identity.accounts (role, email, display_name, sport)
         VALUES ($1, gen_random_uuid()::text || '@example.com', 'テスト', $2)
         RETURNING account_id::text",
    )
    .bind(role)
    .bind(sport)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn student(pool: &PgPool) -> String {
    account(pool, "student", Some("tennis")).await
}

async fn coach(pool: &PgPool) -> String {
    let id = account(pool, "coach", None).await;
    sqlx::query("INSERT INTO identity.coaches (coach_id) VALUES ($1::uuid)")
        .bind(&id)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn slot(pool: &PgPool) -> String {
    let coach_id = coach(pool).await;
    sqlx::query_scalar(
        "INSERT INTO scheduling.slots (coach_id, sport, starts_at, ends_at)
         VALUES ($1::uuid, 'tennis', now() + interval '2 days', now() + interval '2 days 1 hour')
         RETURNING slot_id::text",
    )
    .bind(coach_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn book(pool: &PgPool, slot_id: &str, student_id: &str) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO scheduling.bookings (slot_id, student_id, is_trial)
         VALUES ($1::uuid, $2::uuid, true)
         RETURNING booking_id::text",
    )
    .bind(slot_id)
    .bind(student_id)
    .fetch_one(pool)
    .await
}

/// 月額契約とその受講権を作り、受講権の id を返す。
async fn subscription_entitlement(pool: &PgPool, student_id: &str) -> String {
    let subscription_id: String = sqlx::query_scalar(
        "INSERT INTO entitlement.subscriptions (student_id, sport)
         VALUES ($1::uuid, 'tennis') RETURNING subscription_id::text",
    )
    .bind(student_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO entitlement.entitlements
             (student_id, contract_type, sport, subscription_id, unlimited_video_and_chat)
         VALUES ($1::uuid, 'subscription', 'tennis', $2::uuid, true)
         RETURNING entitlement_id::text",
    )
    .bind(student_id)
    .bind(subscription_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn migrations_create_core_tables(pool: PgPool) {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_schema || '.' || table_name FROM information_schema.tables
         WHERE table_schema IN ('identity', 'scheduling', 'entitlement', 'live_lesson', 'async_lesson', 'matching', 'messaging', 'notification')
         ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        tables,
        [
            "async_lesson.lesson_completions",
            "async_lesson.video_reviews",
            "async_lesson.videos",
            "entitlement.entitlements",
            "entitlement.live_lesson_quota_bookings",
            "entitlement.live_lesson_quotas",
            "entitlement.subscriptions",
            "identity.accounts",
            "identity.coach_sports",
            "identity.coaches",
            "live_lesson.lesson_completions",
            "live_lesson.session_bookings",
            "live_lesson.sessions",
            "matching.favorite_coaches",
            "messaging.messages",
            "messaging.threads",
            "notification.announcement_drafts",
            "notification.announcements",
            "notification.notification_settings",
            "notification.notifications",
            "scheduling.bookings",
            "scheduling.coach_acceptance_changes",
            "scheduling.coach_acceptances",
            "scheduling.slots",
        ]
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn accounts_distinguish_student_coach_and_headquarters(pool: PgPool) {
    student(&pool).await;
    account(&pool, "coach", None).await;
    account(&pool, "headquarters", None).await;

    let (code, _) = violation(
        sqlx::query(
            "INSERT INTO identity.accounts (role, email, display_name) VALUES ('guest', 'g@example.com', 'g')",
        )
        .execute(&pool)
        .await,
    );
    assert_eq!(code, CHECK_VIOLATION);

    // メールアドレスは大文字小文字を区別せず一意
    sqlx::query("INSERT INTO identity.accounts (role, email, display_name) VALUES ('coach', 'Same@example.com', 'a')")
        .execute(&pool)
        .await
        .unwrap();
    let (code, constraint) = violation(
        sqlx::query("INSERT INTO identity.accounts (role, email, display_name) VALUES ('coach', 'same@example.com', 'b')")
            .execute(&pool)
            .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "accounts_email_key")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn only_coach_accounts_can_be_coaches(pool: PgPool) {
    let student_id = student(&pool).await;
    let (code, constraint) = violation(
        sqlx::query("INSERT INTO identity.coaches (coach_id) VALUES ($1::uuid)")
            .bind(student_id)
            .execute(&pool)
            .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "coaches_account_fkey")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn coach_bio_defaults_to_empty_and_is_at_most_400_chars(pool: PgPool) {
    let coach_id = coach(&pool).await;
    let bio: String =
        sqlx::query_scalar("SELECT bio FROM identity.coaches WHERE coach_id = $1::uuid")
            .bind(&coach_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(bio, "");

    let set_bio = |bio: String| {
        let pool = pool.clone();
        let coach_id = coach_id.clone();
        async move {
            sqlx::query("UPDATE identity.coaches SET bio = $2 WHERE coach_id = $1::uuid")
                .bind(coach_id)
                .bind(bio)
                .execute(&pool)
                .await
        }
    };
    // 文字数で数える（「あ」は UTF-8 で 3 バイト）
    set_bio("あ".repeat(400)).await.unwrap();
    let (code, constraint) = violation(set_bio("あ".repeat(401)).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "coaches_bio_length")
    );
    let (code, _) = violation(
        sqlx::query("UPDATE identity.coaches SET bio = NULL WHERE coach_id = $1::uuid")
            .bind(&coach_id)
            .execute(&pool)
            .await,
    );
    assert_eq!(code, "23502", "NULL は入らない（NOT NULL）");
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn slot_must_end_after_it_starts(pool: PgPool) {
    let coach_id = coach(&pool).await;
    let (code, constraint) = violation(
        sqlx::query(
            "INSERT INTO scheduling.slots (coach_id, sport, starts_at, ends_at)
             VALUES ($1::uuid, 'tennis', now(), now())",
        )
        .bind(coach_id)
        .execute(&pool)
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "slots_ends_after_starts")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn slot_takes_several_bookings_but_one_active_per_student(pool: PgPool) {
    let slot_id = slot(&pool).await;
    let student_id = student(&pool).await;
    let first = book(&pool, &slot_id, &student_id).await.unwrap();

    // 1 つの枠に複数の生徒の予約を持てる（定員を超えないことは domain とスロットの行のロックが守る。D40）
    book(&pool, &slot_id, &student(&pool).await).await.unwrap();

    // 同じ生徒の同じ枠への有効な予約は 1 件まで
    let (code, constraint) = violation(book(&pool, &slot_id, &student_id).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "bookings_one_active_per_slot_student")
    );

    // キャンセルされた予約は数えない
    sqlx::query(
        "UPDATE scheduling.bookings SET status = 'cancelled', cancelled_at = now()
         WHERE booking_id = $1::uuid",
    )
    .bind(first)
    .execute(&pool)
    .await
    .unwrap();
    book(&pool, &slot_id, &student_id).await.unwrap();
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn slot_capacity_defaults_to_one_and_is_one_to_six(pool: PgPool) {
    let slot_id = slot(&pool).await;
    let capacity: i16 =
        sqlx::query_scalar("SELECT capacity FROM scheduling.slots WHERE slot_id = $1::uuid")
            .bind(&slot_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        capacity,
        i16::from(domain::scheduling::SlotCapacity::default().value()),
        "既存の枠と定員を省いた枠はマンツーマン"
    );

    let set_capacity = |capacity: i16| {
        sqlx::query("UPDATE scheduling.slots SET capacity = $1 WHERE slot_id = $2::uuid")
            .bind(capacity)
            .bind(&slot_id)
            .execute(&pool)
    };
    for capacity in [
        domain::scheduling::SlotCapacity::MIN,
        domain::scheduling::SlotCapacity::MAX,
    ] {
        set_capacity(i16::from(capacity)).await.unwrap();
    }
    for capacity in [0, 7] {
        let (code, constraint) = violation(set_capacity(capacity).await);
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (CHECK_VIOLATION, "slots_capacity_check"),
            "{capacity}"
        );
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn slot_lesson_menu_is_null_unless_given_and_has_no_foreign_key(pool: PgPool) {
    // 列を知らない（古い版の）INSERT の枠は NULL（メニューのない枠）
    let slot_id = slot(&pool).await;
    let lesson_menu_id: Option<sqlx::types::Uuid> =
        sqlx::query_scalar("SELECT lesson_menu_id FROM scheduling.slots WHERE slot_id = $1::uuid")
            .bind(&slot_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(lesson_menu_id, None);

    // メニューは catalog の持ち物なので外部キーは張らない（結びつけられるかの判定は domain の Slot::with_lesson_menu）
    sqlx::query(
        "UPDATE scheduling.slots SET lesson_menu_id = gen_random_uuid() WHERE slot_id = $1::uuid",
    )
    .bind(&slot_id)
    .execute(&pool)
    .await
    .unwrap();
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn booking_lesson_menu_is_null_unless_copied_and_has_the_shape_of_a_copy(pool: PgPool) {
    // 列を知らない（古い版の）INSERT の予約は NULL（メニューなしの予約）
    let booking_id = book(&pool, &slot(&pool).await, &student(&pool).await)
        .await
        .unwrap();
    let copy: (Option<Uuid>, Option<String>, Option<i16>) = sqlx::query_as(
        "SELECT lesson_menu_id, lesson_menu_name, lesson_menu_duration_minutes
         FROM scheduling.bookings WHERE booking_id = $1::uuid",
    )
    .bind(&booking_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(copy, (None, None, None));

    let set_copy = |id: Option<Uuid>, name: Option<&'static str>, minutes: Option<i16>| {
        sqlx::query(
            "UPDATE scheduling.bookings
             SET lesson_menu_id = $2, lesson_menu_name = $3, lesson_menu_duration_minutes = $4
             WHERE booking_id = $1::uuid",
        )
        .bind(&booking_id)
        .bind(id)
        .bind(name)
        .bind(minutes)
        .execute(&pool)
    };
    // メニューは catalog の持ち物なので外部キーは張らない（ない ID も入る）。時間は非同期だったメニューなら NULL
    let id = Some(Uuid::new_v4());
    for (name, minutes) in [
        ("フォア改善", Some(15)),
        ("フォア改善", Some(120)),
        ("フォア改善", None),
        (
            "あいうえおかきくけこあいうえおかきくけこあいうえおかきくけこあいうえおかきくけこ",
            Some(60),
        ),
    ] {
        set_copy(id, Some(name), minutes).await.unwrap();
    }
    set_copy(None, None, None).await.unwrap();

    for (id, name, minutes, expected) in [
        (id, None, Some(60), "bookings_lesson_menu_shape"),
        (
            None,
            Some("フォア改善"),
            Some(60),
            "bookings_lesson_menu_shape",
        ),
        (None, None, Some(60), "bookings_lesson_menu_shape"),
        (id, Some(" "), Some(60), "bookings_lesson_menu_name_check"),
        (
            id,
            Some(
                "あいうえおかきくけこあいうえおかきくけこあいうえおかきくけこあいうえおかきくけこあ",
            ),
            Some(60),
            "bookings_lesson_menu_name_check",
        ),
        (
            id,
            Some("フォア改善"),
            Some(0),
            "bookings_lesson_menu_duration_check",
        ),
        (
            id,
            Some("フォア改善"),
            Some(50),
            "bookings_lesson_menu_duration_check",
        ),
        (
            id,
            Some("フォア改善"),
            Some(135),
            "bookings_lesson_menu_duration_check",
        ),
    ] {
        let (code, constraint) = violation(set_copy(id, name, minutes).await);
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (CHECK_VIOLATION, expected),
            "{name:?} {minutes:?}"
        );
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn only_students_can_book(pool: PgPool) {
    let slot_id = slot(&pool).await;
    let coach_id = coach(&pool).await;
    let (code, constraint) = violation(book(&pool, &slot_id, &coach_id).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "bookings_student_fkey")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn booking_status_matches_timestamps(pool: PgPool) {
    let slot_id = slot(&pool).await;
    let booking_id = book(&pool, &slot_id, &student(&pool).await).await.unwrap();
    let (code, constraint) = violation(
        sqlx::query(
            "UPDATE scheduling.bookings SET status = 'confirmed' WHERE booking_id = $1::uuid",
        )
        .bind(&booking_id)
        .execute(&pool)
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "bookings_status_timestamps")
    );
    sqlx::query(
        "UPDATE scheduling.bookings SET status = 'confirmed', confirmed_at = now()
         WHERE booking_id = $1::uuid",
    )
    .bind(&booking_id)
    .execute(&pool)
    .await
    .unwrap();
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn subscription_is_not_for_golf(pool: PgPool) {
    let student_id = student(&pool).await;
    let (code, _) = violation(
        sqlx::query(
            "INSERT INTO entitlement.subscriptions (student_id, sport) VALUES ($1::uuid, 'golf')",
        )
        .bind(student_id)
        .execute(&pool)
        .await,
    );
    assert_eq!(code, CHECK_VIOLATION);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn subscription_sports_are_the_sports_monthly_covers(pool: PgPool) {
    use domain::{catalog::Sport, entitlement::ContractType};
    use infra::identity_values::sport_value;

    let monthly: Vec<&str> = Sport::ALL
        .into_iter()
        .filter(|sport| ContractType::Monthly.covers(*sport))
        .map(sport_value)
        .collect();
    assert_eq!(
        check_values(
            &pool,
            "entitlement.subscriptions",
            "subscriptions_sport_check"
        )
        .await,
        monthly
    );
    assert_eq!(
        check_values(
            &pool,
            "entitlement.entitlements",
            "entitlements_sport_check"
        )
        .await,
        Sport::ALL.into_iter().map(sport_value).collect::<Vec<_>>()
    );
    assert_eq!(
        check_values(&pool, "scheduling.slots", "slots_sport_check").await,
        Sport::ALL.into_iter().map(sport_value).collect::<Vec<_>>()
    );
}

/// 変更シナリオ cs-001 で足した水泳は、コーチ・枠・月額の契約と受講権に入り、ワンショットには入らない。
#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn swimming_goes_through_coaches_slots_and_monthly_entitlements(pool: PgPool) {
    let coach_id = coach(&pool).await;
    sqlx::query(
        "INSERT INTO identity.coach_sports (coach_id, sport) VALUES ($1::uuid, 'swimming')",
    )
    .bind(&coach_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO scheduling.slots (coach_id, sport, starts_at, ends_at)
         VALUES ($1::uuid, 'swimming', '2026-12-01T01:00:00Z', '2026-12-01T02:00:00Z')",
    )
    .bind(&coach_id)
    .execute(&pool)
    .await
    .unwrap();

    let student_id = account(&pool, "student", Some("swimming")).await;
    let subscription_id: String = sqlx::query_scalar(
        "INSERT INTO entitlement.subscriptions (student_id, sport)
         VALUES ($1::uuid, 'swimming') RETURNING subscription_id::text",
    )
    .bind(&student_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO entitlement.entitlements
             (student_id, contract_type, sport, subscription_id, unlimited_video_and_chat)
         VALUES ($1::uuid, 'subscription', 'swimming', $2::uuid, true)",
    )
    .bind(&student_id)
    .bind(&subscription_id)
    .execute(&pool)
    .await
    .unwrap();

    let (code, constraint) = violation(
        sqlx::query(
            "INSERT INTO entitlement.entitlements
                 (student_id, contract_type, sport, unlimited_video_and_chat, remaining_video_reviews)
             VALUES ($1::uuid, 'one_shot', 'swimming', false, 1)",
        )
        .bind(&student_id)
        .execute(&pool)
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "entitlements_one_shot_shape")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn entitlement_shape_follows_contract_type(pool: PgPool) {
    let student_id = student(&pool).await;

    // ワンショットはゴルフだけで、残りの動画添削回数を持つ
    sqlx::query(
        "INSERT INTO entitlement.entitlements
             (student_id, contract_type, sport, unlimited_video_and_chat, remaining_video_reviews)
         VALUES ($1::uuid, 'one_shot', 'golf', false, 1)",
    )
    .bind(&student_id)
    .execute(&pool)
    .await
    .unwrap();
    let (code, constraint) = violation(
        sqlx::query(
            "INSERT INTO entitlement.entitlements
                 (student_id, contract_type, sport, unlimited_video_and_chat, remaining_video_reviews)
             VALUES ($1::uuid, 'one_shot', 'tennis', false, 1)",
        )
        .bind(&student_id)
        .execute(&pool)
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "entitlements_one_shot_shape")
    );

    // 月額は元の契約を持ち、動画とチャットは送り放題
    let (code, constraint) = violation(
        sqlx::query(
            "INSERT INTO entitlement.entitlements
                 (student_id, contract_type, sport, unlimited_video_and_chat)
             VALUES ($1::uuid, 'subscription', 'tennis', true)",
        )
        .bind(&student_id)
        .execute(&pool)
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "entitlements_subscription_shape")
    );
    subscription_entitlement(&pool, &student_id).await;
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn subscription_entitlement_matches_its_subscription(pool: PgPool) {
    let student_id = student(&pool).await;
    let subscription_id: String = sqlx::query_scalar(
        "INSERT INTO entitlement.subscriptions (student_id, sport)
         VALUES ($1::uuid, 'tennis') RETURNING subscription_id::text",
    )
    .bind(&student_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 別の生徒・別のスポーツの受講権にはできない
    for (owner, sport) in [(student(&pool).await, "tennis"), (student_id, "soccer")] {
        let (code, constraint) = violation(
            sqlx::query(
                "INSERT INTO entitlement.entitlements
                     (student_id, contract_type, sport, subscription_id, unlimited_video_and_chat)
                 VALUES ($1::uuid, 'subscription', $2, $3::uuid, true)",
            )
            .bind(owner)
            .bind(sport)
            .bind(&subscription_id)
            .execute(&pool)
            .await,
        );
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (FOREIGN_KEY_VIOLATION, "entitlements_subscription_fkey")
        );
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn live_lesson_quota_cannot_be_used_beyond_its_remaining(pool: PgPool) {
    // 残回数はプランの上限（domain の SubscriptionPlan）から数えて書く。DB は 0 以上だけを守り、上限の数は持たない
    let student_id = student(&pool).await;
    let entitlement_id = subscription_entitlement(&pool, &student_id).await;

    sqlx::query(
        "INSERT INTO entitlement.live_lesson_quotas (entitlement_id, month, remaining_live_lessons)
         VALUES ($1::uuid, '2026-11-01', 2)",
    )
    .bind(&entitlement_id)
    .execute(&pool)
    .await
    .unwrap();

    let consume = || {
        sqlx::query(
            "UPDATE entitlement.live_lesson_quotas
             SET remaining_live_lessons = remaining_live_lessons - 1
             WHERE entitlement_id = $1::uuid AND month = '2026-11-01'",
        )
        .bind(&entitlement_id)
        .execute(&pool)
    };
    consume().await.unwrap();
    consume().await.unwrap();
    // 残回数を使い切った後は入らない（standard の 3 回目。不変条件 1）
    let (code, constraint) = violation(consume().await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (
            CHECK_VIOLATION,
            "live_lesson_quotas_remaining_live_lessons_check"
        )
    );

    // 枠は暦月の初日で持つ
    let (code, _) = violation(
        sqlx::query(
            "INSERT INTO entitlement.live_lesson_quotas (entitlement_id, month, remaining_live_lessons)
             VALUES ($1::uuid, '2026-12-15', 2)",
        )
        .bind(&entitlement_id)
        .execute(&pool)
        .await,
    );
    assert_eq!(code, CHECK_VIOLATION);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn only_subscription_entitlements_have_live_lesson_quota(pool: PgPool) {
    let student_id = student(&pool).await;
    let one_shot: String = sqlx::query_scalar(
        "INSERT INTO entitlement.entitlements
             (student_id, contract_type, sport, unlimited_video_and_chat, remaining_video_reviews)
         VALUES ($1::uuid, 'one_shot', 'golf', false, 1)
         RETURNING entitlement_id::text",
    )
    .bind(student_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (code, constraint) = violation(
        sqlx::query(
            "INSERT INTO entitlement.live_lesson_quotas (entitlement_id, month, remaining_live_lessons)
         VALUES ($1::uuid, '2026-11-01', 2)",
        )
        .bind(one_shot)
        .execute(&pool)
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "live_lesson_quotas_entitlement_fkey")
    );
}

/// CHECK 制約の定義に並ぶ文字列リテラル（`'student'::text` の `student`）を出現順に返す。
async fn check_values(pool: &PgPool, table: &str, constraint: &str) -> Vec<String> {
    let definition: String = sqlx::query_scalar(
        "SELECT pg_get_constraintdef(oid) FROM pg_constraint
         WHERE conrelid = $1::regclass AND conname = $2 AND contype = 'c'",
    )
    .bind(table)
    .bind(constraint)
    .fetch_one(pool)
    .await
    .unwrap();
    definition
        .split('\'')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn identity_values_match_the_accounts_checks(pool: PgPool) {
    use domain::{catalog::Sport, identity::Role};
    use infra::identity_values::{role_value, sport_value};

    let roles: Vec<&str> = Role::ALL.into_iter().map(role_value).collect();
    assert_eq!(
        check_values(&pool, "identity.accounts", "accounts_role_check").await,
        roles
    );
    let sports: Vec<&str> = Sport::ALL.into_iter().map(sport_value).collect();
    assert_eq!(
        check_values(&pool, "identity.accounts", "accounts_sport_check").await,
        sports
    );

    // 対応表の値で実際に保存できる
    for role in Role::ALL {
        let sport = (role == Role::Student).then(|| sport_value(Sport::Tennis));
        account(&pool, role_value(role), sport).await;
    }
    for sport in Sport::ALL {
        account(&pool, role_value(Role::Student), Some(sport_value(sport))).await;
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn scheduling_values_match_the_slots_checks(pool: PgPool) {
    use domain::scheduling::SlotStatus;
    use infra::scheduling_values::slot_status_value;

    let statuses: Vec<&str> = SlotStatus::ALL.into_iter().map(slot_status_value).collect();
    assert_eq!(
        check_values(&pool, "scheduling.slots", "slots_status_check").await,
        statuses
    );

    // 既定は公開中で、対応表にない値は入らない
    let slot_id = slot(&pool).await;
    let status: String =
        sqlx::query_scalar("SELECT status FROM scheduling.slots WHERE slot_id = $1::uuid")
            .bind(&slot_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, slot_status_value(SlotStatus::Open));
    let (code, constraint) = violation(
        sqlx::query("UPDATE scheduling.slots SET status = 'cancelled' WHERE slot_id = $1::uuid")
            .bind(&slot_id)
            .execute(&pool)
            .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "slots_status_check")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn coach_acceptances_are_for_coaches_and_match_the_scheduling_values(pool: PgPool) {
    use domain::scheduling::CoachAcceptanceStatus;
    use infra::scheduling_values::coach_acceptance_status_value;

    let statuses: Vec<&str> = CoachAcceptanceStatus::ALL
        .into_iter()
        .map(coach_acceptance_status_value)
        .collect();
    assert_eq!(
        check_values(
            &pool,
            "scheduling.coach_acceptances",
            "coach_acceptances_status_check"
        )
        .await,
        statuses
    );

    let coach_id = coach(&pool).await;
    sqlx::query(
        "INSERT INTO scheduling.coach_acceptances (coach_id, status) VALUES ($1::uuid, 'suspended')",
    )
    .bind(&coach_id)
    .execute(&pool)
    .await
    .unwrap();
    let (code, constraint) = violation(
        sqlx::query(
            "UPDATE scheduling.coach_acceptances SET status = 'closed' WHERE coach_id = $1::uuid",
        )
        .bind(&coach_id)
        .execute(&pool)
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "coach_acceptances_status_check")
    );

    // コーチでないアカウントの受付は入らない
    let student_id = student(&pool).await;
    let (code, constraint) = violation(
        sqlx::query(
            "INSERT INTO scheduling.coach_acceptances (coach_id, status) VALUES ($1::uuid, 'suspended')",
        )
        .bind(&student_id)
        .execute(&pool)
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        ("23503", "coach_acceptances_coach_id_fkey")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn coach_acceptance_changes_are_by_headquarters_for_coaches_and_match_the_scheduling_values(
    pool: PgPool,
) {
    use domain::scheduling::CoachAcceptanceStatus;
    use infra::scheduling_values::coach_acceptance_status_value;

    let statuses: Vec<&str> = CoachAcceptanceStatus::ALL
        .into_iter()
        .map(coach_acceptance_status_value)
        .collect();
    assert_eq!(
        check_values(
            &pool,
            "scheduling.coach_acceptance_changes",
            "coach_acceptance_changes_status_check"
        )
        .await,
        statuses
    );

    let insert =
        "INSERT INTO scheduling.coach_acceptance_changes (coach_id, status, changed_by, changed_at)
                  VALUES ($1::uuid, $2, $3::uuid, now())";
    let coach_id = coach(&pool).await;
    let headquarters_id = account(&pool, "headquarters", None).await;
    sqlx::query(insert)
        .bind(&coach_id)
        .bind("suspended")
        .bind(&headquarters_id)
        .execute(&pool)
        .await
        .unwrap();

    let (code, constraint) = violation(
        sqlx::query(insert)
            .bind(&coach_id)
            .bind("closed")
            .bind(&headquarters_id)
            .execute(&pool)
            .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "coach_acceptance_changes_status_check")
    );

    // 操作したのは本部のアカウントだけ
    let other_coach = coach(&pool).await;
    let (code, constraint) = violation(
        sqlx::query(insert)
            .bind(&coach_id)
            .bind("accepting")
            .bind(&other_coach)
            .execute(&pool)
            .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (
            FOREIGN_KEY_VIOLATION,
            "coach_acceptance_changes_changed_by_fkey"
        )
    );

    // コーチでないアカウントの記録は入らない
    let student_id = student(&pool).await;
    let (code, constraint) = violation(
        sqlx::query(insert)
            .bind(&student_id)
            .bind("suspended")
            .bind(&headquarters_id)
            .execute(&pool)
            .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (
            FOREIGN_KEY_VIOLATION,
            "coach_acceptance_changes_coach_id_fkey"
        )
    );
}

/// 通話セッションを `started_at` に始めた行を入れ、session_id を返す。
async fn start_session(
    pool: &PgPool,
    booking_id: &str,
    student_id: &str,
    coach_id: &str,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO live_lesson.sessions (booking_id, student_id, coach_id, started_at)
         VALUES ($1::uuid, $2::uuid, $3::uuid, '2026-11-01T10:00:00Z')
         RETURNING session_id::text",
    )
    .bind(booking_id)
    .bind(student_id)
    .bind(coach_id)
    .fetch_one(pool)
    .await
}

/// 予約 1 件分の生徒・コーチ・予約の id（予約は scheduling の行で、live_lesson からは外部キーで参照しない）。
async fn session_parties(pool: &PgPool) -> (String, String, String) {
    let slot_id = slot(pool).await;
    let student_id = student(pool).await;
    let booking_id = book(pool, &slot_id, &student_id).await.unwrap();
    let coach_id: String =
        sqlx::query_scalar("SELECT coach_id::text FROM scheduling.slots WHERE slot_id = $1::uuid")
            .bind(&slot_id)
            .fetch_one(pool)
            .await
            .unwrap();
    (booking_id, student_id, coach_id)
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_booking_has_at_most_one_live_lesson_session(pool: PgPool) {
    let (booking_id, student_id, coach_id) = session_parties(&pool).await;
    start_session(&pool, &booking_id, &student_id, &coach_id)
        .await
        .unwrap();
    let (code, constraint) =
        violation(start_session(&pool, &booking_id, &student_id, &coach_id).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "sessions_booking_id_key")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn live_lesson_session_parties_have_the_right_roles(pool: PgPool) {
    let (booking_id, student_id, coach_id) = session_parties(&pool).await;
    // 生徒の欄にコーチ、コーチの欄に生徒は入らない
    let (code, constraint) =
        violation(start_session(&pool, &booking_id, &coach_id, &coach_id).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "sessions_student_fkey")
    );
    let (code, _) = violation(start_session(&pool, &booking_id, &student_id, &student_id).await);
    assert_eq!(code, FOREIGN_KEY_VIOLATION);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn live_lesson_session_records_quality_only_when_ended(pool: PgPool) {
    let (booking_id, student_id, coach_id) = session_parties(&pool).await;
    let session_id = start_session(&pool, &booking_id, &student_id, &coach_id)
        .await
        .unwrap();
    let end = |ended_at: &'static str, packet_loss_rate: Option<f64>| {
        sqlx::query(
            "UPDATE live_lesson.sessions
             SET ended_at = $2::timestamptz, packet_loss_rate = $3,
                 jitter_ms = 20, round_trip_ms = 120, disconnect_count = 0
             WHERE session_id = $1::uuid",
        )
        .bind(&session_id)
        .bind(ended_at)
        .bind(packet_loss_rate)
        .execute(&pool)
    };

    // 終了は開始の後
    let (code, constraint) = violation(end("2026-11-01T09:59:59Z", Some(0.5)).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "sessions_ends_after_starts")
    );
    // 指標は 4 つ揃える
    let (code, constraint) = violation(end("2026-11-01T10:45:00Z", None).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "sessions_quality_when_ended")
    );
    // 損失率は百分率
    let (code, _) = violation(end("2026-11-01T10:45:00Z", Some(100.5)).await);
    assert_eq!(code, CHECK_VIOLATION);
    end("2026-11-01T10:45:00Z", Some(0.5)).await.unwrap();

    // 終了していないセッションは指標を持たない
    let (booking_id, student_id, coach_id) = session_parties(&pool).await;
    let (code, constraint) = violation(
        sqlx::query(
            "INSERT INTO live_lesson.sessions (booking_id, student_id, coach_id, started_at, jitter_ms)
             VALUES ($1::uuid, $2::uuid, $3::uuid, now(), 20)",
        )
        .bind(booking_id)
        .bind(student_id)
        .bind(coach_id)
        .execute(&pool)
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "sessions_quality_when_ended")
    );
}

/// 生徒 `student_id` がコーチ `coach_id` に送った動画の行を入れ、video_id を返す。
async fn submit_video(
    pool: &PgPool,
    student_id: &str,
    coach_id: &str,
    sport: &str,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO async_lesson.videos (student_id, coach_id, sport, submitted_at)
         VALUES ($1::uuid, $2::uuid, $3, '2026-10-26T10:00:00Z')
         RETURNING video_id::text",
    )
    .bind(student_id)
    .bind(coach_id)
    .bind(sport)
    .fetch_one(pool)
    .await
}

/// 動画 `video_id` に添削 `comment` を返した行を入れる。
async fn return_review(
    pool: &PgPool,
    video_id: &str,
    comment: &str,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO async_lesson.video_reviews (video_id, comment, returned_at)
         VALUES ($1::uuid, $2, '2026-10-27T16:00:00Z')
         RETURNING review_id::text",
    )
    .bind(video_id)
    .bind(comment)
    .fetch_one(pool)
    .await
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn video_parties_have_the_right_roles(pool: PgPool) {
    let student_id = student(&pool).await;
    let coach_id = coach(&pool).await;
    submit_video(&pool, &student_id, &coach_id, "tennis")
        .await
        .unwrap();
    // 生徒の欄にコーチ、コーチの欄に生徒は入らない
    let (code, constraint) = violation(submit_video(&pool, &coach_id, &coach_id, "tennis").await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "videos_student_fkey")
    );
    let (code, _) = violation(submit_video(&pool, &student_id, &student_id, "tennis").await);
    assert_eq!(code, FOREIGN_KEY_VIOLATION);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn video_lesson_menu_is_null_unless_copied_and_has_the_shape_of_a_copy(pool: PgPool) {
    // 列を知らない（古い版の）INSERT の動画は NULL（メニューなしの動画）
    let video_id = submit_video(&pool, &student(&pool).await, &coach(&pool).await, "tennis")
        .await
        .unwrap();
    let copy: (Option<Uuid>, Option<String>) = sqlx::query_as(
        "SELECT lesson_menu_id, lesson_menu_name FROM async_lesson.videos WHERE video_id = $1::uuid",
    )
    .bind(&video_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(copy, (None, None));

    let set_copy = |id: Option<Uuid>, name: Option<&'static str>| {
        sqlx::query(
            "UPDATE async_lesson.videos SET lesson_menu_id = $2, lesson_menu_name = $3
             WHERE video_id = $1::uuid",
        )
        .bind(&video_id)
        .bind(id)
        .bind(name)
        .execute(&pool)
    };
    // メニューは catalog の持ち物なので外部キーは張らない（ない ID も入る）
    let id = Some(Uuid::new_v4());
    for name in [
        "フォーム添削",
        "あいうえおかきくけこあいうえおかきくけこあいうえおかきくけこあいうえおかきくけこ",
    ] {
        set_copy(id, Some(name)).await.unwrap();
    }
    set_copy(None, None).await.unwrap();

    for (id, name, expected) in [
        (id, None, "videos_lesson_menu_shape"),
        (None, Some("フォーム添削"), "videos_lesson_menu_shape"),
        (id, Some(""), "videos_lesson_menu_name_check"),
        (id, Some(" "), "videos_lesson_menu_name_check"),
        (
            id,
            Some(
                "あいうえおかきくけこあいうえおかきくけこあいうえおかきくけこあいうえおかきくけこあ",
            ),
            "videos_lesson_menu_name_check",
        ),
    ] {
        let (code, constraint) = violation(set_copy(id, name).await);
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (CHECK_VIOLATION, expected),
            "{name:?}"
        );
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn video_sports_match_the_identity_values(pool: PgPool) {
    use domain::catalog::Sport;
    use infra::identity_values::sport_value;

    let sports: Vec<&str> = Sport::ALL.into_iter().map(sport_value).collect();
    assert_eq!(
        check_values(&pool, "async_lesson.videos", "videos_sport_check").await,
        sports
    );
    let student_id = student(&pool).await;
    let coach_id = coach(&pool).await;
    let (code, constraint) =
        violation(submit_video(&pool, &student_id, &coach_id, "cricket").await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "videos_sport_check")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_video_has_at_most_one_review(pool: PgPool) {
    let student_id = student(&pool).await;
    let coach_id = coach(&pool).await;
    let video_id = submit_video(&pool, &student_id, &coach_id, "tennis")
        .await
        .unwrap();

    // 空のコメントは入らない
    for comment in ["", "   "] {
        let (code, constraint) = violation(return_review(&pool, &video_id, comment).await);
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (CHECK_VIOLATION, "video_reviews_comment_check"),
            "{comment:?}"
        );
    }
    return_review(&pool, &video_id, "テイクバックが早い")
        .await
        .unwrap();
    // 返却は 1 回
    let (code, constraint) = violation(return_review(&pool, &video_id, "もう一度").await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "video_reviews_video_id_key")
    );
    // ない動画の添削は入らない
    let (code, constraint) =
        violation(return_review(&pool, "00000000-0000-4000-8000-000000000000", "コメント").await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "video_reviews_video_id_fkey")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn ai_feedback_has_a_non_empty_comment_and_a_time_after_submission(pool: PgPool) {
    let student_id = student(&pool).await;
    let coach_id = coach(&pool).await;
    let video_id = submit_video(&pool, &student_id, &coach_id, "tennis")
        .await
        .unwrap();
    // 送付は '2026-10-26T10:00:00Z'
    let set = |comment: Option<&'static str>, created_at: Option<&'static str>| {
        sqlx::query(
            "UPDATE async_lesson.videos
             SET ai_feedback_comment = $2, ai_feedback_created_at = $3::timestamptz
             WHERE video_id = $1::uuid",
        )
        .bind(video_id.clone())
        .bind(comment)
        .bind(created_at)
        .execute(&pool)
    };
    for (comment, created_at, expected) in [
        (
            Some("  "),
            Some("2026-10-26T10:00:00Z"),
            "videos_ai_feedback_comment_check",
        ),
        (Some("文"), None, "videos_ai_feedback_shape"),
        (
            None,
            Some("2026-10-26T10:00:00Z"),
            "videos_ai_feedback_shape",
        ),
        (
            Some("文"),
            Some("2026-10-26T09:59:59Z"),
            "videos_ai_feedback_after_submission",
        ),
    ] {
        let (code, constraint) = violation(set(comment, created_at).await);
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (CHECK_VIOLATION, expected),
            "{comment:?} {created_at:?}"
        );
    }
    // 送付と同じ時刻の一次フィードバックは入り、揃って NULL（なし）にも戻せる
    set(Some("フォームを確認しました"), Some("2026-10-26T10:00:00Z"))
        .await
        .unwrap();
    set(None, None).await.unwrap();
}

/// 完了記録の行を `table`（`live_lesson.lesson_completions` / `async_lesson.lesson_completions`）に入れる。
/// `source_column` は元の集約の列（`session_id` / `video_id`）。
async fn complete_lesson(
    pool: &PgPool,
    table: &'static str,
    source_column: &'static str,
    source_id: &str,
    (student_id, coach_id): (&str, &str),
    is_first_lesson: bool,
) -> Result<String, sqlx::Error> {
    // テーブル名・列名はテストの定数だけ（利用者の入力ではない）
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {table} ({source_column}, student_id, coach_id, is_first_lesson, completed_at)
         VALUES ($1::uuid, $2::uuid, $3::uuid, $4, '2026-11-01T11:00:00Z')
         RETURNING lesson_id::text"
    )))
    .bind(source_id)
    .bind(student_id)
    .bind(coach_id)
    .bind(is_first_lesson)
    .fetch_one(pool)
    .await
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_live_lesson_session_is_completed_once_per_student_by_its_coach(pool: PgPool) {
    const TABLE: &str = "live_lesson.lesson_completions";
    let (booking_id, student_id, coach_id) = session_parties(&pool).await;
    let session_id = start_session(&pool, &booking_id, &student_id, &coach_id)
        .await
        .unwrap();
    let other_coach = coach(&pool).await;

    // コーチはセッションのコーチ
    let (code, constraint) = violation(
        complete_lesson(
            &pool,
            TABLE,
            "session_id",
            &session_id,
            (&student_id, &other_coach),
            true,
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (
            FOREIGN_KEY_VIOLATION,
            "lesson_completions_session_coach_fkey"
        )
    );
    complete_lesson(
        &pool,
        TABLE,
        "session_id",
        &session_id,
        (&student_id, &coach_id),
        true,
    )
    .await
    .unwrap();
    // 同じセッションの同じ生徒の完了は 1 回
    let (code, constraint) = violation(
        complete_lesson(
            &pool,
            TABLE,
            "session_id",
            &session_id,
            (&student_id, &coach_id),
            false,
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "lesson_completions_session_student_key")
    );
    // グループレッスンの別の生徒の完了は同じセッションにも入る（生徒が結び付いた予約の生徒かは domain が判定する。D41）
    complete_lesson(
        &pool,
        TABLE,
        "session_id",
        &session_id,
        (&student(&pool).await, &coach_id),
        true,
    )
    .await
    .unwrap();
}

/// 通話セッション `session_id` に予約 `booking_id`（生徒 `student_id`）を `position` 番目に結び付ける。
async fn link_booking(
    pool: &PgPool,
    session_id: &str,
    booking_id: &str,
    student_id: &str,
    position: i16,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO live_lesson.session_bookings (session_id, booking_id, student_id, position)
         VALUES ($1::uuid, $2::uuid, $3::uuid, $4)",
    )
    .bind(session_id)
    .bind(booking_id)
    .bind(student_id)
    .bind(position)
    .execute(pool)
    .await
    .map(|_| ())
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_booking_is_linked_to_at_most_one_live_lesson_session(pool: PgPool) {
    let (booking_id, student_id, coach_id) = session_parties(&pool).await;
    let session_id = start_session(&pool, &booking_id, &student_id, &coach_id)
        .await
        .unwrap();
    link_booking(&pool, &session_id, &booking_id, &student_id, 0)
        .await
        .unwrap();

    // 同じ予約を別のセッションに結び付けられない
    let (other_booking, other_student, _) = session_parties(&pool).await;
    let other_session = start_session(&pool, &other_booking, &other_student, &coach_id)
        .await
        .unwrap();
    let (code, constraint) =
        violation(link_booking(&pool, &other_session, &booking_id, &student_id, 1).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "session_bookings_booking_id_key")
    );
    // 同じセッションに同じ生徒・同じ並びは 1 つ。生徒の欄は生徒だけ
    let third_booking = Uuid::new_v4().to_string();
    for (student, position, expected) in [
        (
            &student_id,
            1,
            (UNIQUE_VIOLATION, "session_bookings_session_student_key"),
        ),
        (
            &other_student,
            0,
            (UNIQUE_VIOLATION, "session_bookings_session_position_key"),
        ),
        (
            &coach_id,
            1,
            (FOREIGN_KEY_VIOLATION, "session_bookings_student_fkey"),
        ),
    ] {
        let (code, constraint) =
            violation(link_booking(&pool, &session_id, &third_booking, student, position).await);
        assert_eq!((code.as_str(), constraint.as_str()), expected);
    }
    link_booking(&pool, &session_id, &third_booking, &other_student, 1)
        .await
        .unwrap();
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn only_reviewed_videos_are_completed_once_by_their_parties(pool: PgPool) {
    const TABLE: &str = "async_lesson.lesson_completions";
    let student_id = student(&pool).await;
    let coach_id = coach(&pool).await;
    let other_coach = coach(&pool).await;
    let video_id = submit_video(&pool, &student_id, &coach_id, "tennis")
        .await
        .unwrap();
    let parties = (student_id.as_str(), coach_id.as_str());

    // 添削を返していない動画は完了にできない
    let (code, constraint) =
        violation(complete_lesson(&pool, TABLE, "video_id", &video_id, parties, true).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "lesson_completions_video_id_fkey")
    );
    return_review(&pool, &video_id, "テイクバックが早い")
        .await
        .unwrap();
    // 生徒・コーチは動画の当事者
    let (code, constraint) = violation(
        complete_lesson(
            &pool,
            TABLE,
            "video_id",
            &video_id,
            (&student_id, &other_coach),
            true,
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "lesson_completions_video_fkey")
    );
    complete_lesson(&pool, TABLE, "video_id", &video_id, parties, true)
        .await
        .unwrap();
    // 同じ動画の完了は 1 回
    let (code, constraint) =
        violation(complete_lesson(&pool, TABLE, "video_id", &video_id, parties, false).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "lesson_completions_video_id_key")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_student_has_at_most_one_first_lesson_per_table(pool: PgPool) {
    // 同期: 同じ生徒の 2 つのセッション
    let (booking_id, student_id, coach_id) = session_parties(&pool).await;
    let first = start_session(&pool, &booking_id, &student_id, &coach_id)
        .await
        .unwrap();
    let slot_id: String = sqlx::query_scalar(
        "INSERT INTO scheduling.slots (coach_id, sport, starts_at, ends_at)
         VALUES ($1::uuid, 'tennis', now() + interval '3 days', now() + interval '3 days 1 hour')
         RETURNING slot_id::text",
    )
    .bind(&coach_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let second_booking = book(&pool, &slot_id, &student_id).await.unwrap();
    let second = start_session(&pool, &second_booking, &student_id, &coach_id)
        .await
        .unwrap();
    let parties = (student_id.as_str(), coach_id.as_str());
    const LIVE: &str = "live_lesson.lesson_completions";
    complete_lesson(&pool, LIVE, "session_id", &first, parties, true)
        .await
        .unwrap();
    let (code, constraint) =
        violation(complete_lesson(&pool, LIVE, "session_id", &second, parties, true).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "lesson_completions_one_first_per_student")
    );
    complete_lesson(&pool, LIVE, "session_id", &second, parties, false)
        .await
        .unwrap();

    // 非同期: 同じ生徒の 2 本の動画
    const ASYNC: &str = "async_lesson.lesson_completions";
    let mut videos = Vec::new();
    for _ in 0..2 {
        let video_id = submit_video(&pool, &student_id, &coach_id, "tennis")
            .await
            .unwrap();
        return_review(&pool, &video_id, "テイクバックが早い")
            .await
            .unwrap();
        videos.push(video_id);
    }
    complete_lesson(&pool, ASYNC, "video_id", &videos[0], parties, true)
        .await
        .unwrap();
    let (code, constraint) =
        violation(complete_lesson(&pool, ASYNC, "video_id", &videos[1], parties, true).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "lesson_completions_one_first_per_student")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn booking_and_entitlement_values_match_their_checks(pool: PgPool) {
    use domain::scheduling::BookingStatus;
    use infra::{
        entitlement_values::{
            CONTRACT_TYPES, SUBSCRIPTION_PLANS, SUBSCRIPTION_STATUSES, contract_type_value,
            subscription_plan_value, subscription_status_value,
        },
        scheduling_values::booking_status_value,
    };

    let statuses: Vec<&str> = BookingStatus::ALL
        .into_iter()
        .map(booking_status_value)
        .collect();
    assert_eq!(
        check_values(&pool, "scheduling.bookings", "bookings_status_check").await,
        statuses
    );
    let contract_types: Vec<&str> = CONTRACT_TYPES
        .into_iter()
        .map(contract_type_value)
        .collect();
    assert_eq!(
        check_values(
            &pool,
            "entitlement.entitlements",
            "entitlements_contract_type_check"
        )
        .await,
        contract_types
    );
    let subscription_statuses: Vec<&str> = SUBSCRIPTION_STATUSES
        .into_iter()
        .map(subscription_status_value)
        .collect();
    assert_eq!(
        check_values(
            &pool,
            "entitlement.subscriptions",
            "subscriptions_status_check"
        )
        .await,
        subscription_statuses
    );
    let subscription_plans: Vec<&str> = SUBSCRIPTION_PLANS
        .into_iter()
        .map(subscription_plan_value)
        .collect();
    assert_eq!(
        check_values(
            &pool,
            "entitlement.subscriptions",
            "subscriptions_plan_check"
        )
        .await,
        subscription_plans
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn subscriptions_default_to_the_standard_plan_and_reject_unknown_plans(pool: PgPool) {
    // 既存の契約（列を足す前の行）は standard として読む（変更シナリオ cs-003）
    let student_id = student(&pool).await;
    subscription_entitlement(&pool, &student_id).await;
    let plan: String = sqlx::query_scalar("SELECT plan FROM entitlement.subscriptions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(plan, "standard");

    let (code, constraint) = violation(
        sqlx::query(
            "INSERT INTO entitlement.subscriptions (student_id, sport, plan)
             VALUES ($1::uuid, 'tennis', 'premium')",
        )
        .bind(student(&pool).await)
        .execute(&pool)
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "subscriptions_plan_check")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn only_coaches_have_coach_sports(pool: PgPool) {
    use domain::catalog::Sport;
    use infra::identity_values::sport_value;

    let sports: Vec<&str> = Sport::ALL.into_iter().map(sport_value).collect();
    assert_eq!(
        check_values(&pool, "identity.coach_sports", "coach_sports_sport_check").await,
        sports
    );
    let coach_id = coach(&pool).await;
    let insert = |id: String, sport: &'static str| {
        sqlx::query("INSERT INTO identity.coach_sports (coach_id, sport) VALUES ($1::uuid, $2)")
            .bind(id)
            .bind(sport)
            .execute(&pool)
    };
    insert(coach_id.clone(), "tennis").await.unwrap();
    insert(coach_id.clone(), "golf").await.unwrap();
    let (code, _) = violation(insert(coach_id.clone(), "tennis").await);
    assert_eq!(code, UNIQUE_VIOLATION);
    let (code, constraint) = violation(insert(student(&pool).await, "tennis").await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "coach_sports_coach_id_fkey")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn quota_bookings_belong_to_a_quota_month(pool: PgPool) {
    let student_id = student(&pool).await;
    let entitlement_id = subscription_entitlement(&pool, &student_id).await;
    sqlx::query(
        "INSERT INTO entitlement.live_lesson_quotas (entitlement_id, month, remaining_live_lessons)
         VALUES ($1::uuid, '2026-11-01', 2)",
    )
    .bind(&entitlement_id)
    .execute(&pool)
    .await
    .unwrap();
    let insert = |month: &'static str| {
        sqlx::query(
            "INSERT INTO entitlement.live_lesson_quota_bookings (entitlement_id, month, booking_id)
             VALUES ($1::uuid, $2::date, gen_random_uuid())",
        )
        .bind(entitlement_id.clone())
        .bind(month)
        .execute(&pool)
    };
    insert("2026-11-01").await.unwrap();
    // 枠の行がない月には入らない
    let (code, constraint) = violation(insert("2026-12-01").await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (
            FOREIGN_KEY_VIOLATION,
            "live_lesson_quota_bookings_quota_fkey"
        )
    );
}

/// 生徒 `student_id` と担当コーチ `coach_id` のスレッドの行を入れ、thread_id を返す。
async fn open_thread(
    pool: &PgPool,
    student_id: &str,
    coach_id: &str,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO messaging.threads (student_id, coach_id, opened_at)
         VALUES ($1::uuid, $2::uuid, '2026-10-26T10:00:00Z')
         RETURNING thread_id::text",
    )
    .bind(student_id)
    .bind(coach_id)
    .fetch_one(pool)
    .await
}

/// スレッド `thread_id` に `sender_role` が `body` を送った行を入れる（送信 10:00、既読は `read_at`）。
async fn send_message(
    pool: &PgPool,
    thread_id: &str,
    sender_role: &str,
    body: &str,
    read_at: Option<&str>,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO messaging.messages (thread_id, sender_role, body, sent_at, read_at)
         VALUES ($1::uuid, $2, $3, '2026-10-26T10:00:00Z', $4::timestamptz)
         RETURNING message_id::text",
    )
    .bind(thread_id)
    .bind(sender_role)
    .bind(body)
    .bind(read_at)
    .fetch_one(pool)
    .await
}

/// レッスン `lesson_id` に、生徒 `student_id` がコーチ `coach_id` を評価した行を入れる。
async fn review_lesson(
    pool: &PgPool,
    lesson_id: &str,
    student_id: &str,
    coach_id: &str,
    rating: i16,
    comment: &str,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO review.lesson_reviews (lesson_id, student_id, coach_id, rating, comment, submitted_at)
         VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, '2026-10-27T16:00:00Z')
         RETURNING review_id::text",
    )
    .bind(lesson_id)
    .bind(student_id)
    .bind(coach_id)
    .bind(rating)
    .bind(comment)
    .fetch_one(pool)
    .await
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_student_and_a_coach_have_at_most_one_thread_with_the_right_roles(pool: PgPool) {
    let student_id = student(&pool).await;
    let coach_id = coach(&pool).await;
    open_thread(&pool, &student_id, &coach_id).await.unwrap();
    // 同じ生徒とコーチのスレッドは 1 つ
    let (code, constraint) = violation(open_thread(&pool, &student_id, &coach_id).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "threads_student_id_coach_id_key")
    );
    // 生徒の欄にコーチ、コーチの欄に生徒は入らない
    let (code, constraint) = violation(open_thread(&pool, &coach_id, &coach_id).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "threads_student_fkey")
    );
    let (code, constraint) = violation(open_thread(&pool, &student_id, &student_id).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "threads_coach_id_fkey")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn messages_have_a_body_a_known_sender_and_are_read_after_sent(pool: PgPool) {
    use domain::messaging::MessageAuthor;
    use infra::messaging_values::message_author_value;

    let senders: Vec<&str> = MessageAuthor::ALL
        .into_iter()
        .map(message_author_value)
        .collect();
    assert_eq!(
        check_values(&pool, "messaging.messages", "messages_sender_role_check").await,
        senders
    );

    let student_id = student(&pool).await;
    let coach_id = coach(&pool).await;
    let thread_id = open_thread(&pool, &student_id, &coach_id).await.unwrap();
    for sender in senders {
        send_message(&pool, &thread_id, sender, "こんにちは", None)
            .await
            .unwrap();
    }
    send_message(
        &pool,
        &thread_id,
        "student",
        "読んだ",
        Some("2026-10-26T10:00:00Z"),
    )
    .await
    .unwrap();

    for (sender, body, read_at, expected) in [
        (
            "headquarters",
            "x",
            None,
            (CHECK_VIOLATION, "messages_sender_role_check"),
        ),
        (
            "student",
            "",
            None,
            (CHECK_VIOLATION, "messages_body_check"),
        ),
        (
            "student",
            "   ",
            None,
            (CHECK_VIOLATION, "messages_body_check"),
        ),
        (
            "coach",
            "x",
            Some("2026-10-26T09:59:59Z"),
            (CHECK_VIOLATION, "messages_read_after_sent"),
        ),
    ] {
        let (code, constraint) =
            violation(send_message(&pool, &thread_id, sender, body, read_at).await);
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            expected,
            "{sender} {body:?} {read_at:?}"
        );
    }
    // ないスレッドのメッセージは入らない
    let (code, constraint) = violation(
        send_message(
            &pool,
            "00000000-0000-4000-8000-000000000000",
            "student",
            "x",
            None,
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "messages_thread_id_fkey")
    );
}

/// スレッド `thread_id` に、種類 `kind` のメッセージの行を入れる（送信 10:00）。
async fn post_message(
    pool: &PgPool,
    thread_id: &str,
    sender_role: &str,
    kind: &str,
    body: Option<&str>,
    video_id: Option<&str>,
    review_id: Option<&str>,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO messaging.messages (thread_id, sender_role, kind, body, video_id, review_id, sent_at)
         VALUES ($1::uuid, $2, $3, $4, $5::uuid, $6::uuid, '2026-10-26T10:00:00Z')
         RETURNING message_id::text",
    )
    .bind(thread_id)
    .bind(sender_role)
    .bind(kind)
    .bind(body)
    .bind(video_id)
    .bind(review_id)
    .fetch_one(pool)
    .await
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn messages_have_the_shape_and_the_sender_of_their_kind(pool: PgPool) {
    use domain::messaging::MessageKind;
    use infra::messaging_values::message_kind_value;

    const VIDEO: &str = "00000000-0000-4000-8000-000000000001";
    const OTHER_VIDEO: &str = "00000000-0000-4000-8000-000000000002";
    const REVIEW: &str = "00000000-0000-4000-8000-000000000003";

    let kinds: Vec<&str> = MessageKind::ALL
        .into_iter()
        .map(message_kind_value)
        .collect();
    assert_eq!(
        check_values(&pool, "messaging.messages", "messages_kind_check").await,
        kinds
    );

    let student_id = student(&pool).await;
    let coach_id = coach(&pool).await;
    let thread_id = open_thread(&pool, &student_id, &coach_id).await.unwrap();
    // 種類を書かない行（古い版のアプリ）は本文の種類
    let old = send_message(&pool, &thread_id, "coach", "こんにちは", None)
        .await
        .unwrap();
    let kind: String =
        sqlx::query_scalar("SELECT kind FROM messaging.messages WHERE message_id = $1::uuid")
            .bind(&old)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(kind, "text");
    post_message(
        &pool,
        &thread_id,
        "system",
        "text",
        Some("お知らせ"),
        None,
        None,
    )
    .await
    .unwrap();
    post_message(
        &pool,
        &thread_id,
        "student",
        "video_review_request",
        None,
        Some(VIDEO),
        None,
    )
    .await
    .unwrap();
    post_message(
        &pool,
        &thread_id,
        "coach",
        "video_review_return",
        None,
        Some(VIDEO),
        Some(REVIEW),
    )
    .await
    .unwrap();
    // 別の動画の依頼は入る
    post_message(
        &pool,
        &thread_id,
        "student",
        "video_review_request",
        None,
        Some(OTHER_VIDEO),
        None,
    )
    .await
    .unwrap();

    for (sender, kind, body, video, review, expected) in [
        (
            "student",
            "concierge",
            Some("x"),
            None,
            None,
            (CHECK_VIOLATION, "messages_kind_check"),
        ),
        (
            "student",
            "text",
            None,
            None,
            None,
            (CHECK_VIOLATION, "messages_kind_shape"),
        ),
        (
            "student",
            "text",
            Some("x"),
            Some(VIDEO),
            None,
            (CHECK_VIOLATION, "messages_kind_shape"),
        ),
        (
            "student",
            "video_review_request",
            Some("x"),
            Some(VIDEO),
            None,
            (CHECK_VIOLATION, "messages_kind_shape"),
        ),
        (
            "student",
            "video_review_request",
            None,
            None,
            None,
            (CHECK_VIOLATION, "messages_kind_shape"),
        ),
        (
            "coach",
            "video_review_return",
            None,
            Some(VIDEO),
            None,
            (CHECK_VIOLATION, "messages_kind_shape"),
        ),
        (
            "coach",
            "video_review_request",
            None,
            Some(OTHER_VIDEO),
            None,
            (CHECK_VIOLATION, "messages_kind_sender"),
        ),
        (
            "student",
            "video_review_return",
            None,
            Some(OTHER_VIDEO),
            Some(REVIEW),
            (CHECK_VIOLATION, "messages_kind_sender"),
        ),
        (
            "system",
            "video_review_request",
            None,
            Some(OTHER_VIDEO),
            None,
            (CHECK_VIOLATION, "messages_kind_sender"),
        ),
        // 同じ動画の依頼・返却はスレッドに 1 通ずつ
        (
            "student",
            "video_review_request",
            None,
            Some(VIDEO),
            None,
            (UNIQUE_VIOLATION, "messages_one_per_video_and_kind"),
        ),
        (
            "coach",
            "video_review_return",
            None,
            Some(VIDEO),
            Some(REVIEW),
            (UNIQUE_VIOLATION, "messages_one_per_video_and_kind"),
        ),
    ] {
        let (code, constraint) =
            violation(post_message(&pool, &thread_id, sender, kind, body, video, review).await);
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            expected,
            "{sender} {kind} {body:?} {video:?} {review:?}"
        );
    }
}

/// スレッド `thread_id` に、スタンプ `stamp` の種類 `kind` のメッセージの行を入れる（本文 `body`、送信 10:00）。
async fn post_stamp(
    pool: &PgPool,
    thread_id: &str,
    sender_role: &str,
    kind: &str,
    body: Option<&str>,
    stamp: Option<&str>,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO messaging.messages (thread_id, sender_role, kind, body, stamp, sent_at)
         VALUES ($1::uuid, $2, $3, $4, $5, '2026-10-26T10:00:00Z')
         RETURNING message_id::text",
    )
    .bind(thread_id)
    .bind(sender_role)
    .bind(kind)
    .bind(body)
    .bind(stamp)
    .fetch_one(pool)
    .await
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn stamps_are_known_ids_from_a_participant_without_a_body(pool: PgPool) {
    use domain::messaging::Stamp;
    use infra::messaging_values::stamp_value;

    let stamps: Vec<&str> = Stamp::ALL.into_iter().map(stamp_value).collect();
    assert_eq!(
        check_values(&pool, "messaging.messages", "messages_stamp_check").await,
        stamps
    );

    let student_id = student(&pool).await;
    let coach_id = coach(&pool).await;
    let thread_id = open_thread(&pool, &student_id, &coach_id).await.unwrap();
    for (sender, stamp) in [("student", "thanks"), ("coach", "nice_shot")] {
        post_stamp(&pool, &thread_id, sender, "stamp", None, Some(stamp))
            .await
            .unwrap();
    }
    // 同じスタンプを何度送ってもよい（動画の 1 通ずつの索引は video_id のある行だけ）
    post_stamp(&pool, &thread_id, "student", "stamp", None, Some("thanks"))
        .await
        .unwrap();

    for (sender, kind, body, stamp, expected) in [
        (
            "student",
            "stamp",
            None,
            Some("heart"),
            (CHECK_VIOLATION, "messages_stamp_check"),
        ),
        (
            "student",
            "stamp",
            None,
            None,
            (CHECK_VIOLATION, "messages_kind_shape"),
        ),
        (
            "student",
            "stamp",
            Some("ありがとう"),
            Some("thanks"),
            (CHECK_VIOLATION, "messages_kind_shape"),
        ),
        (
            "student",
            "text",
            Some("ありがとう"),
            Some("thanks"),
            (CHECK_VIOLATION, "messages_stamp_only_for_stamp"),
        ),
        (
            "system",
            "stamp",
            None,
            Some("ok"),
            (CHECK_VIOLATION, "messages_kind_sender"),
        ),
    ] {
        let (code, constraint) =
            violation(post_stamp(&pool, &thread_id, sender, kind, body, stamp).await);
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            expected,
            "{sender} {kind} {body:?} {stamp:?}"
        );
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_lesson_is_reviewed_once_by_a_student_with_one_to_five(pool: PgPool) {
    const LESSON: &str = "00000000-0000-4000-8000-000000000001";
    let student_id = student(&pool).await;
    let coach_id = coach(&pool).await;

    // 点は 1〜5
    for rating in [0, 6] {
        let (code, constraint) =
            violation(review_lesson(&pool, LESSON, &student_id, &coach_id, rating, "よい").await);
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (CHECK_VIOLATION, "lesson_reviews_rating_check"),
            "{rating}"
        );
    }
    // 空の一言は入らない
    let (code, constraint) =
        violation(review_lesson(&pool, LESSON, &student_id, &coach_id, 3, "  ").await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "lesson_reviews_comment_check")
    );
    // 生徒の欄にコーチ、コーチの欄に生徒は入らない
    let (code, constraint) =
        violation(review_lesson(&pool, LESSON, &coach_id, &coach_id, 3, "よい").await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "lesson_reviews_student_fkey")
    );
    let (code, constraint) =
        violation(review_lesson(&pool, LESSON, &student_id, &student_id, 3, "よい").await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "lesson_reviews_coach_id_fkey")
    );

    for rating in [1, 5] {
        let lesson = format!("00000000-0000-4000-8000-00000000000{rating}");
        review_lesson(&pool, &lesson, &student_id, &coach_id, rating, "よい")
            .await
            .unwrap();
    }
    // 同じレッスンの評価は 1 つ
    let (code, constraint) =
        violation(review_lesson(&pool, LESSON, &student_id, &coach_id, 4, "もう一度").await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "lesson_reviews_lesson_id_key")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_review_reply_has_a_short_non_empty_comment_and_a_time_after_submission(pool: PgPool) {
    let student_id = student(&pool).await;
    let coach_id = coach(&pool).await;
    let review_id = review_lesson(
        &pool,
        "00000000-0000-4000-8000-000000000001",
        &student_id,
        &coach_id,
        4,
        "よい",
    )
    .await
    .unwrap();
    // 既存の評価（返信の列を足す前の書き方）は返信なしの NULL
    let (comment, replied_at): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT reply_comment, replied_at::text FROM review.lesson_reviews WHERE review_id = $1::uuid",
    )
    .bind(&review_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((comment, replied_at), (None, None));

    // 評価は '2026-10-27T16:00:00Z'
    let set = |comment: Option<String>, replied_at: Option<&'static str>| {
        sqlx::query(
            "UPDATE review.lesson_reviews SET reply_comment = $2, replied_at = $3::timestamptz
             WHERE review_id = $1::uuid",
        )
        .bind(review_id.clone())
        .bind(comment)
        .bind(replied_at)
        .execute(&pool)
    };
    for (comment, replied_at, expected) in [
        (
            Some("  ".to_string()),
            Some("2026-10-27T16:00:00Z"),
            "lesson_reviews_reply_comment_check",
        ),
        (
            Some("あ".repeat(201)),
            Some("2026-10-27T16:00:00Z"),
            "lesson_reviews_reply_comment_check",
        ),
        (
            Some("ありがとう".to_string()),
            None,
            "lesson_reviews_reply_shape",
        ),
        (
            None,
            Some("2026-10-27T16:00:00Z"),
            "lesson_reviews_reply_shape",
        ),
        (
            Some("ありがとう".to_string()),
            Some("2026-10-27T15:59:59Z"),
            "lesson_reviews_replied_after_submission",
        ),
    ] {
        let (code, constraint) = violation(set(comment.clone(), replied_at).await);
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (CHECK_VIOLATION, expected),
            "{comment:?} {replied_at:?}"
        );
    }
    // 評価と同じ時刻の 200 文字の返信は入る
    set(Some("あ".repeat(200)), Some("2026-10-27T16:00:00Z"))
        .await
        .unwrap();
}

/// アカウント `account_id` の通知の種類 `kind` の受け取り設定の行を入れる。
async fn set_notification(
    pool: &PgPool,
    account_id: &str,
    kind: &str,
    enabled: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO notification.notification_settings (account_id, kind, enabled)
         VALUES ($1::uuid, $2, $3)",
    )
    .bind(account_id)
    .bind(kind)
    .bind(enabled)
    .execute(pool)
    .await
    .map(|_| ())
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn notification_settings_are_one_per_account_and_known_kind(pool: PgPool) {
    use domain::notification::NotificationKind;
    use infra::notification_values::notification_kind_value;

    let kinds: Vec<&str> = NotificationKind::ALL
        .into_iter()
        .map(notification_kind_value)
        .collect();
    assert_eq!(
        check_values(
            &pool,
            "notification.notification_settings",
            "notification_settings_kind_check"
        )
        .await,
        kinds
    );

    // 生徒もコーチも持てる
    let student_id = student(&pool).await;
    let coach_id = coach(&pool).await;
    for kind in &kinds {
        set_notification(&pool, &student_id, kind, false)
            .await
            .unwrap();
        set_notification(&pool, &coach_id, kind, true)
            .await
            .unwrap();
    }

    // 未知の種類は入らない
    let (code, constraint) =
        violation(set_notification(&pool, &student_id, "booking_cancelled", false).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "notification_settings_kind_check")
    );
    // 同じアカウントの同じ種類は 1 つ
    let (code, constraint) = violation(set_notification(&pool, &student_id, kinds[0], true).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "notification_settings_pkey")
    );
    // ないアカウントの設定は入らない
    let (code, constraint) = violation(
        set_notification(
            &pool,
            "00000000-0000-4000-8000-000000000001",
            kinds[0],
            false,
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (
            FOREIGN_KEY_VIOLATION,
            "notification_settings_account_id_fkey"
        )
    );
}

/// お知らせの行を入れる。`push` は（booking_id, video_id, review_id, thread_id, message_id, sender_role）、`read_at` は時刻の文字列。
#[allow(clippy::type_complexity)]
async fn insert_notification(
    pool: &PgPool,
    recipient_id: &str,
    kind: &str,
    (student_id, coach_id): (&str, &str),
    push: (
        Option<&str>,
        Option<&str>,
        Option<&str>,
        Option<&str>,
        Option<&str>,
        Option<&str>,
    ),
    read_at: Option<&str>,
) -> Result<(), sqlx::Error> {
    let (booking_id, video_id, review_id, thread_id, message_id, sender_role) = push;
    sqlx::query(
        "INSERT INTO notification.notifications
             (notification_id, recipient_id, kind, student_id, coach_id, booking_id, video_id, review_id,
              thread_id, message_id, sender_role, created_at, read_at)
         VALUES (gen_random_uuid(), $1::uuid, $2, $3::uuid, $4::uuid, $5::uuid, $6::uuid, $7::uuid,
                 $8::uuid, $9::uuid, $10, '2026-11-01T10:00:00Z', $11::timestamptz)",
    )
    .bind(recipient_id)
    .bind(kind)
    .bind(student_id)
    .bind(coach_id)
    .bind(booking_id)
    .bind(video_id)
    .bind(review_id)
    .bind(thread_id)
    .bind(message_id)
    .bind(sender_role)
    .bind(read_at)
    .execute(pool)
    .await
    .map(|_| ())
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn notifications_have_the_shape_of_their_push_and_are_one_per_push(pool: PgPool) {
    use infra::notification_values::PUSH_NOTIFICATION_KIND_VALUES;

    assert_eq!(
        check_values(
            &pool,
            "notification.notifications",
            "notifications_kind_check"
        )
        .await,
        PUSH_NOTIFICATION_KIND_VALUES
    );

    let student_id = student(&pool).await;
    let coach_id = coach(&pool).await;
    let parties = (student_id.as_str(), coach_id.as_str());
    const A: &str = "00000000-0000-4000-8000-00000000000a";
    const B: &str = "00000000-0000-4000-8000-00000000000b";
    const C: &str = "00000000-0000-4000-8000-00000000000c";
    let booking = (Some(A), None, None, None, None, None);
    let review = (None, Some(A), Some(B), None, None, None);
    let message = (None, None, None, Some(A), Some(C), Some("coach"));
    let request = (None, Some(A), None, None, None, None);

    // 変種ごとの形で入る
    insert_notification(
        &pool,
        &student_id,
        "booking_confirmed",
        parties,
        booking,
        None,
    )
    .await
    .unwrap();
    insert_notification(&pool, &coach_id, "booking_received", parties, booking, None)
        .await
        .unwrap();
    insert_notification(
        &pool,
        &coach_id,
        "video_review_requested",
        parties,
        request,
        None,
    )
    .await
    .unwrap();
    insert_notification(
        &pool,
        &student_id,
        "video_review_returned",
        parties,
        review,
        None,
    )
    .await
    .unwrap();
    insert_notification(
        &pool,
        &student_id,
        "message_received",
        parties,
        message,
        Some("2026-11-01T10:00:00Z"),
    )
    .await
    .unwrap();

    // 同じプッシュ通知は 1 件
    let (code, constraint) = violation(
        insert_notification(
            &pool,
            &student_id,
            "booking_confirmed",
            parties,
            booking,
            None,
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "notifications_one_per_push")
    );
    // 同じ動画の添削の依頼も 1 件（同じ動画の添削の返却とは別）
    let (code, constraint) = violation(
        insert_notification(
            &pool,
            &coach_id,
            "video_review_requested",
            parties,
            request,
            None,
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (
            UNIQUE_VIOLATION,
            "notifications_one_per_video_review_request"
        )
    );

    // 種類に合わない中身は入らない
    for (kind, push) in [
        ("booking_confirmed", review),
        (
            "video_review_returned",
            (None, Some(B), None, None, None, None),
        ),
        ("video_review_requested", review),
        (
            "video_review_requested",
            (Some(B), Some(B), None, None, None, None),
        ),
        (
            "video_review_requested",
            (None, None, None, None, None, None),
        ),
        (
            "message_received",
            (None, None, None, Some(B), Some(B), None),
        ),
        (
            "message_received",
            (Some(B), None, None, Some(B), Some(B), Some("coach")),
        ),
    ] {
        let (code, constraint) =
            violation(insert_notification(&pool, &student_id, kind, parties, push, None).await);
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (CHECK_VIOLATION, "notifications_kind_shape"),
            "{kind} {push:?}"
        );
    }

    // 受け取る人は当事者のどちらか
    let other = student(&pool).await;
    let (code, constraint) = violation(
        insert_notification(
            &pool,
            &other,
            "booking_confirmed",
            parties,
            (Some(B), None, None, None, None, None),
            None,
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "notifications_recipient_is_a_party")
    );

    // 既読は作った後
    let (code, constraint) = violation(
        insert_notification(
            &pool,
            &student_id,
            "booking_confirmed",
            parties,
            (Some(C), None, None, None, None, None),
            Some("2026-11-01T09:59:59Z"),
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (CHECK_VIOLATION, "notifications_read_after_created")
    );

    // ないアカウントには残せない
    let (code, constraint) = violation(
        insert_notification(
            &pool,
            "00000000-0000-4000-8000-000000000001",
            "booking_confirmed",
            ("00000000-0000-4000-8000-000000000001", &coach_id),
            (Some(C), None, None, None, None, None),
            None,
        )
        .await,
    );
    assert_eq!(code, FOREIGN_KEY_VIOLATION, "{constraint}");
}

/// 本部 `created_by` の下書きを入れる。
async fn insert_announcement_draft(
    pool: &PgPool,
    created_by: &str,
    title: &str,
    body: &str,
    sport: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO notification.announcement_drafts
             (announcement_draft_id, created_by, title, body, sport, created_at, updated_at)
         VALUES (gen_random_uuid(), $1::uuid, $2, $3, $4, '2026-11-01T10:00:00Z', '2026-11-01T10:00:00Z')",
    )
    .bind(created_by)
    .bind(title)
    .bind(body)
    .bind(sport)
    .execute(pool)
    .await
    .map(|_| ())
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn announcement_drafts_may_be_empty_but_keep_the_limits_and_a_known_sport(pool: PgPool) {
    use domain::catalog::Sport;
    use infra::identity_values::sport_value;

    let sports: Vec<&str> = Sport::ALL.into_iter().map(sport_value).collect();
    assert_eq!(
        check_values(
            &pool,
            "notification.announcement_drafts",
            "announcement_drafts_sport_check"
        )
        .await,
        sports
    );

    let headquarters = account(&pool, "headquarters", None).await;
    for (title, body, sport) in [
        ("", "", None),
        (" ", "\n", None),
        (&*"あ".repeat(50), &*"あ".repeat(2000), Some("swimming")),
    ] {
        insert_announcement_draft(&pool, &headquarters, title, body, sport)
            .await
            .unwrap();
    }

    for (title, body, sport, constraint) in [
        (
            &*"あ".repeat(51),
            "",
            None,
            "announcement_drafts_title_check",
        ),
        (
            "",
            &*"あ".repeat(2001),
            None,
            "announcement_drafts_body_check",
        ),
        ("", "", Some("cricket"), "announcement_drafts_sport_check"),
    ] {
        let (code, actual) =
            violation(insert_announcement_draft(&pool, &headquarters, title, body, sport).await);
        assert_eq!(
            (code.as_str(), actual.as_str()),
            (CHECK_VIOLATION, constraint)
        );
    }

    let (code, constraint) = violation(
        insert_announcement_draft(&pool, "00000000-0000-4000-8000-000000000001", "", "", None)
            .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "announcement_drafts_created_by_fkey")
    );
}

/// 本部 `sender_id` の送った記録を入れて ID を返す。
async fn insert_announcement(
    pool: &PgPool,
    sender_id: &str,
    title: &str,
    body: &str,
    sport: Option<&str>,
    recipient_count: i32,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO notification.announcements
             (announcement_id, sender_id, title, body, sport, recipient_count, sent_at)
         VALUES (gen_random_uuid(), $1::uuid, $2, $3, $4, $5, '2026-11-01T10:00:00Z')
         RETURNING announcement_id::text",
    )
    .bind(sender_id)
    .bind(title)
    .bind(body)
    .bind(sport)
    .bind(recipient_count)
    .fetch_one(pool)
    .await
}

/// 種類 `kind` のお知らせの行を、コーチ `coach_id`（なければ NULL）・送った記録 `announcement_id`・予約 `booking_id` で入れる。
async fn insert_announcement_notification(
    pool: &PgPool,
    recipient_id: &str,
    kind: &str,
    (student_id, coach_id): (&str, Option<&str>),
    announcement_id: Option<&str>,
    booking_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO notification.notifications
             (notification_id, recipient_id, kind, student_id, coach_id, booking_id, announcement_id, created_at)
         VALUES (gen_random_uuid(), $1::uuid, $2, $3::uuid, $4::uuid, $5::uuid, $6::uuid, '2026-11-01T10:00:00Z')",
    )
    .bind(recipient_id)
    .bind(kind)
    .bind(student_id)
    .bind(coach_id)
    .bind(booking_id)
    .bind(announcement_id)
    .execute(pool)
    .await
    .map(|_| ())
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn announcements_have_a_title_a_body_a_known_sport_and_recipients(pool: PgPool) {
    use domain::catalog::Sport;
    use infra::identity_values::sport_value;

    let sports: Vec<&str> = Sport::ALL.into_iter().map(sport_value).collect();
    assert_eq!(
        check_values(
            &pool,
            "notification.announcements",
            "announcements_sport_check"
        )
        .await,
        sports
    );

    let headquarters = account(&pool, "headquarters", None).await;
    insert_announcement(&pool, &headquarters, "題", "本文", None, 1)
        .await
        .unwrap();
    insert_announcement(
        &pool,
        &headquarters,
        &"あ".repeat(50),
        &"あ".repeat(2000),
        Some("swimming"),
        3,
    )
    .await
    .unwrap();

    for (title, body, sport, recipient_count, constraint) in [
        (" ", "本文", None, 1, "announcements_title_check"),
        (
            &*"あ".repeat(51),
            "本文",
            None,
            1,
            "announcements_title_check",
        ),
        ("題", "", None, 1, "announcements_body_check"),
        (
            "題",
            &*"あ".repeat(2001),
            None,
            1,
            "announcements_body_check",
        ),
        (
            "題",
            "本文",
            Some("cricket"),
            1,
            "announcements_sport_check",
        ),
        ("題", "本文", None, 0, "announcements_recipient_count_check"),
    ] {
        let (code, actual) = violation(
            insert_announcement(&pool, &headquarters, title, body, sport, recipient_count).await,
        );
        assert_eq!(
            (code.as_str(), actual.as_str()),
            (CHECK_VIOLATION, constraint)
        );
    }

    let (code, constraint) = violation(
        insert_announcement(
            &pool,
            "00000000-0000-4000-8000-000000000001",
            "題",
            "本文",
            None,
            1,
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "announcements_sender_id_fkey")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn an_announcement_withdrawal_has_both_an_account_and_a_time(pool: PgPool) {
    let headquarters = account(&pool, "headquarters", None).await;
    let announcement = insert_announcement(&pool, &headquarters, "題", "本文", None, 1)
        .await
        .unwrap();
    let withdraw = |by: Option<&str>, at: Option<&str>| {
        sqlx::query(
            "UPDATE notification.announcements
             SET withdrawn_by = $2::uuid, withdrawn_at = $3::timestamptz
             WHERE announcement_id = $1::uuid",
        )
        .bind(announcement.clone())
        .bind(by.map(str::to_owned))
        .bind(at.map(str::to_owned))
        .execute(&pool)
    };

    for (by, at) in [
        (Some(headquarters.as_str()), None),
        (None, Some("2026-11-01T11:00:00Z")),
    ] {
        let (code, constraint) = violation(withdraw(by, at).await);
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (CHECK_VIOLATION, "announcements_withdrawal_check"),
            "片方だけは不可"
        );
    }
    let (code, constraint) = violation(
        withdraw(
            Some("00000000-0000-4000-8000-000000000001"),
            Some("2026-11-01T11:00:00Z"),
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "announcements_withdrawn_by_fkey")
    );
    withdraw(Some(&headquarters), Some("2026-11-01T11:00:00Z"))
        .await
        .unwrap();
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn announcement_notifications_go_to_a_student_without_a_coach_once(pool: PgPool) {
    let headquarters = account(&pool, "headquarters", None).await;
    let announcement = insert_announcement(&pool, &headquarters, "題", "本文", None, 2)
        .await
        .unwrap();
    let student_id = student(&pool).await;
    let other = student(&pool).await;
    let coach_id = coach(&pool).await;
    const BOOKING: &str = "00000000-0000-4000-8000-00000000000a";

    insert_announcement_notification(
        &pool,
        &student_id,
        "announcement",
        (&student_id, None),
        Some(&announcement),
        None,
    )
    .await
    .unwrap();
    insert_announcement_notification(
        &pool,
        &other,
        "announcement",
        (&other, None),
        Some(&announcement),
        None,
    )
    .await
    .unwrap();

    // 同じお知らせは同じ生徒に 1 件
    let (code, constraint) = violation(
        insert_announcement_notification(
            &pool,
            &student_id,
            "announcement",
            (&student_id, None),
            Some(&announcement),
            None,
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "notifications_one_per_announcement")
    );

    // 形に合わないものは入らない: コーチを持つ・送った記録がない・受け取る人が生徒でない・ほかの中身を持つ・
    // ほかの種類がコーチを持たない・ほかの種類が送った記録を持つ
    for (recipient, kind, coach, announcement_id, booking) in [
        (
            &student_id,
            "announcement",
            Some(coach_id.as_str()),
            Some(announcement.as_str()),
            None,
        ),
        (&student_id, "announcement", None, None, None),
        (
            &other,
            "announcement",
            None,
            Some(announcement.as_str()),
            None,
        ),
        (
            &student_id,
            "announcement",
            None,
            Some(announcement.as_str()),
            Some(BOOKING),
        ),
        (&student_id, "booking_confirmed", None, None, Some(BOOKING)),
        (
            &student_id,
            "booking_confirmed",
            Some(coach_id.as_str()),
            Some(announcement.as_str()),
            Some(BOOKING),
        ),
    ] {
        let (code, constraint) = violation(
            insert_announcement_notification(
                &pool,
                recipient,
                kind,
                (&student_id, coach),
                announcement_id,
                booking,
            )
            .await,
        );
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (CHECK_VIOLATION, "notifications_kind_shape"),
            "{kind} {coach:?} {announcement_id:?} {booking:?}"
        );
    }

    // ない送った記録は指せない
    let (code, constraint) = violation(
        insert_announcement_notification(
            &pool,
            &student_id,
            "announcement",
            (&student_id, None),
            Some("00000000-0000-4000-8000-000000000001"),
            None,
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "notifications_announcement_id_fkey")
    );
}

/// コーチ `coach_id` のレッスンメニューを入れる。
async fn insert_lesson_menu(
    pool: &PgPool,
    coach_id: &str,
    sport: &str,
    lesson_type: &str,
    name: &str,
    description: &str,
    duration_minutes: Option<i16>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO catalog.lesson_menus
             (lesson_menu_id, coach_id, sport, lesson_type, name, description, duration_minutes,
              is_published, created_at)
         VALUES (gen_random_uuid(), $1::uuid, $2, $3, $4, $5, $6, true, now())",
    )
    .bind(coach_id)
    .bind(sport)
    .bind(lesson_type)
    .bind(name)
    .bind(description)
    .bind(duration_minutes)
    .execute(pool)
    .await
    .map(|_| ())
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn catalog_values_match_the_lesson_menus_checks(pool: PgPool) {
    use domain::{catalog::Sport, lesson::LessonType};
    use infra::{catalog_values::lesson_type_value, identity_values::sport_value};

    assert_eq!(
        check_values(
            &pool,
            "catalog.lesson_menus",
            "lesson_menus_lesson_type_check"
        )
        .await,
        LessonType::ALL
            .into_iter()
            .map(lesson_type_value)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        check_values(&pool, "catalog.lesson_menus", "lesson_menus_sport_check").await,
        Sport::ALL.into_iter().map(sport_value).collect::<Vec<_>>()
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn lesson_menu_sport_is_one_of_the_coach_sports(pool: PgPool) {
    let coach_id = coach(&pool).await;
    sqlx::query("INSERT INTO identity.coach_sports (coach_id, sport) VALUES ($1::uuid, 'tennis')")
        .bind(&coach_id)
        .execute(&pool)
        .await
        .unwrap();
    insert_lesson_menu(&pool, &coach_id, "tennis", "async_lesson", "添削", "", None)
        .await
        .unwrap();

    // コーチが提供しないスポーツ、コーチでないアカウントのメニューは入らない
    let (code, constraint) = violation(
        insert_lesson_menu(&pool, &coach_id, "soccer", "async_lesson", "添削", "", None).await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "lesson_menus_coach_sport_fkey")
    );
    let student_id = student(&pool).await;
    let (code, constraint) = violation(
        insert_lesson_menu(
            &pool,
            &student_id,
            "tennis",
            "async_lesson",
            "添削",
            "",
            None,
        )
        .await,
    );
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "lesson_menus_coach_sport_fkey")
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn lesson_menu_headquarters_unpublish_has_both_time_and_reason_or_neither(pool: PgPool) {
    let coach_id = coach(&pool).await;
    sqlx::query("INSERT INTO identity.coach_sports (coach_id, sport) VALUES ($1::uuid, 'tennis')")
        .bind(&coach_id)
        .execute(&pool)
        .await
        .unwrap();
    insert_lesson_menu(&pool, &coach_id, "tennis", "async_lesson", "添削", "", None)
        .await
        .unwrap();
    let set = |at: Option<&'static str>, reason: Option<String>| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "UPDATE catalog.lesson_menus
                 SET headquarters_unpublished_at = $1::timestamptz, headquarters_unpublish_reason = $2",
            )
            .bind(at)
            .bind(reason)
            .execute(&pool)
            .await
            .map(|_| ())
        }
    };

    // 揃って値を持つ（理由は空でも 200 文字でもよい）か、揃って NULL
    for reason in [String::new(), "あ".repeat(200)] {
        set(Some("2026-11-01T00:00:00Z"), Some(reason))
            .await
            .unwrap();
    }
    set(None, None).await.unwrap();

    for (at, reason, constraint_name) in [
        (
            Some("2026-11-01T00:00:00Z"),
            None,
            "lesson_menus_headquarters_unpublish_shape",
        ),
        (
            None,
            Some("理由".to_string()),
            "lesson_menus_headquarters_unpublish_shape",
        ),
        (
            Some("2026-11-01T00:00:00Z"),
            Some("あ".repeat(201)),
            "lesson_menus_headquarters_unpublish_reason_check",
        ),
    ] {
        let (code, constraint) = violation(set(at, reason).await);
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (CHECK_VIOLATION, constraint_name)
        );
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn lesson_menu_shape_follows_the_lesson_type(pool: PgPool) {
    let coach_id = coach(&pool).await;
    sqlx::query("INSERT INTO identity.coach_sports (coach_id, sport) VALUES ($1::uuid, 'tennis')")
        .bind(&coach_id)
        .execute(&pool)
        .await
        .unwrap();
    for minutes in [15, 60, 120] {
        insert_lesson_menu(
            &pool,
            &coach_id,
            "tennis",
            "live_lesson",
            "通話",
            "",
            Some(minutes),
        )
        .await
        .unwrap();
    }
    let long_name = "あ".repeat(40);
    let long_description = "あ".repeat(400);
    insert_lesson_menu(
        &pool,
        &coach_id,
        "tennis",
        "async_lesson",
        &long_name,
        &long_description,
        None,
    )
    .await
    .unwrap();

    // 同期は 15〜120 分の 15 分刻みの時間が要り、非同期は時間を持てない
    for (lesson_type, minutes) in [
        ("live_lesson", None),
        ("live_lesson", Some(0)),
        ("live_lesson", Some(20)),
        ("live_lesson", Some(135)),
        ("async_lesson", Some(30)),
    ] {
        let (code, constraint) = violation(
            insert_lesson_menu(
                &pool,
                &coach_id,
                "tennis",
                lesson_type,
                "メニュー",
                "",
                minutes,
            )
            .await,
        );
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (CHECK_VIOLATION, "lesson_menus_duration_shape"),
            "{lesson_type} {minutes:?}"
        );
    }
    // 名前は 1〜40 文字、説明は 400 文字まで
    for (name, description, constraint_name) in [
        ("", "", "lesson_menus_name_check"),
        (" ", "", "lesson_menus_name_check"),
        (&*"あ".repeat(41), "", "lesson_menus_name_check"),
        (
            "メニュー",
            &*"あ".repeat(401),
            "lesson_menus_description_check",
        ),
    ] {
        let (code, constraint) = violation(
            insert_lesson_menu(
                &pool,
                &coach_id,
                "tennis",
                "async_lesson",
                name,
                description,
                None,
            )
            .await,
        );
        assert_eq!(
            (code.as_str(), constraint.as_str()),
            (CHECK_VIOLATION, constraint_name)
        );
    }
}

/// 生徒 `student_id` のお気に入りにコーチ `coach_id` を入れる行を入れる。
async fn favorite(pool: &PgPool, student_id: &str, coach_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO matching.favorite_coaches (student_id, coach_id, favorited_at)
         VALUES ($1::uuid, $2::uuid, now())",
    )
    .bind(student_id)
    .bind(coach_id)
    .execute(pool)
    .await
    .map(|_| ())
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn favorite_coaches_are_one_per_student_and_coach(pool: PgPool) {
    let student_id = student(&pool).await;
    let other_student = student(&pool).await;
    let coach_id = coach(&pool).await;
    favorite(&pool, &student_id, &coach_id).await.unwrap();
    favorite(&pool, &other_student, &coach_id).await.unwrap();

    // 同じ生徒と同じコーチの組は 1 つ
    let (code, constraint) = violation(favorite(&pool, &student_id, &coach_id).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "favorite_coaches_pkey")
    );
    // お気に入りを持てるのは生徒だけ
    let (code, constraint) = violation(favorite(&pool, &coach_id, &coach_id).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "favorite_coaches_student_fkey")
    );
    // 入れる相手はコーチだけ
    let (code, constraint) = violation(favorite(&pool, &student_id, &other_student).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (FOREIGN_KEY_VIOLATION, "favorite_coaches_coach_id_fkey")
    );
}

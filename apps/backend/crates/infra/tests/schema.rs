//! スキーマの検証。空の DB にマイグレーションを当て、DB で守る不変条件を確かめる。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`apps/backend/README.md` の手順で
//! Postgres を起動し、`DATABASE_URL` を設定して `cargo test -p infra -- --include-ignored` で走らせる。
//! `#[sqlx::test]` はテストごとに新しい DB を作って `infra::MIGRATOR` を当てる。

use sqlx::PgPool;

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
         WHERE table_schema IN ('identity', 'scheduling', 'entitlement', 'live_lesson', 'async_lesson')
         ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        tables,
        [
            "async_lesson.video_reviews",
            "async_lesson.videos",
            "entitlement.entitlements",
            "entitlement.live_lesson_quotas",
            "entitlement.subscriptions",
            "identity.accounts",
            "identity.coaches",
            "live_lesson.sessions",
            "scheduling.bookings",
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
async fn slot_cannot_be_double_booked(pool: PgPool) {
    let slot_id = slot(&pool).await;
    let first = book(&pool, &slot_id, &student(&pool).await).await.unwrap();

    let other_student = student(&pool).await;
    let (code, constraint) = violation(book(&pool, &slot_id, &other_student).await);
    assert_eq!(
        (code.as_str(), constraint.as_str()),
        (UNIQUE_VIOLATION, "bookings_one_active_per_slot")
    );

    // キャンセルされた予約は枠を占めない
    sqlx::query(
        "UPDATE scheduling.bookings SET status = 'cancelled', cancelled_at = now()
         WHERE booking_id = $1::uuid",
    )
    .bind(first)
    .execute(&pool)
    .await
    .unwrap();
    book(&pool, &slot_id, &other_student).await.unwrap();
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
async fn live_lesson_quota_is_two_per_calendar_month(pool: PgPool) {
    let student_id = student(&pool).await;
    let entitlement_id = subscription_entitlement(&pool, &student_id).await;

    sqlx::query(
        "INSERT INTO entitlement.live_lesson_quotas (entitlement_id, month) VALUES ($1::uuid, '2026-11-01')",
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
    // 3 回目は入らない（不変条件 1）
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
            "INSERT INTO entitlement.live_lesson_quotas (entitlement_id, month) VALUES ($1::uuid, '2026-12-15')",
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
            "INSERT INTO entitlement.live_lesson_quotas (entitlement_id, month) VALUES ($1::uuid, '2026-11-01')",
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
    use domain::identity::{Role, Sport};
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
async fn video_sports_match_the_identity_values(pool: PgPool) {
    use domain::identity::Sport;
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

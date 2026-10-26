use std::time::SystemTime;

use sqlx::PgPool;
use time::OffsetDateTime;

/// DB に書かれた合成データの件数と分布（同じ種で 2 回走らせた結果の比較と、受け入れ条件の確認に使う）。
///
/// 「生徒の出来事」は行動イベントの `student_id` を持つ出来事と同じ（登録・予約の作成 / 確定 / キャンセル・通話の
/// 開始 / 終了・動画の送付・添削の返却・完了記録・月額契約の開始）で、DB のそれぞれの時刻の列から読む。
#[derive(Debug, Clone, PartialEq)]
pub struct SeedReport {
    pub coaches: i64,
    /// スポーツごとのコーチの数（スポーツの値の順）
    pub coaches_by_sport: Vec<(String, i64)>,
    pub students: i64,
    /// `since` 以後に出来事がある生徒の数
    pub active_students: i64,
    /// コーチごとの生徒の数（予約か動画の相手として現れた生徒。多い順）
    pub students_per_coach: Vec<i64>,
    pub slots: i64,
    pub bookings: i64,
    pub trial_bookings: i64,
    pub cancelled_bookings: i64,
    /// 開始 24 時間前以降のキャンセル（回数を消費する側）
    pub late_cancellations: i64,
    pub sessions: i64,
    /// 良好な回線の範囲（損失 1% 以内）を外れた通話
    pub low_quality_sessions: i64,
    pub videos: i64,
    pub reviews: i64,
    pub completions: i64,
    pub first_lessons: i64,
    pub subscriptions: i64,
    /// `since` より前に出来事が途絶えた（離脱した）生徒の、初回レッスンから最後の出来事までの日数の区分
    /// （30 日以内・31〜90 日・91〜180 日・181 日以降）
    pub churned_after_first_lesson: [i64; 4],
}

/// 生徒の出来事（student_id, at）の CTE を前に付けた SQL。
macro_rules! with_activity {
    ($query:literal) => {
        concat!(
            "WITH activity (student_id, at) AS (
                SELECT account_id, created_at FROM identity.accounts WHERE role = 'student'
                UNION ALL SELECT student_id, created_at FROM scheduling.bookings
                UNION ALL SELECT student_id, confirmed_at FROM scheduling.bookings WHERE confirmed_at IS NOT NULL
                UNION ALL SELECT student_id, cancelled_at FROM scheduling.bookings WHERE cancelled_at IS NOT NULL
                UNION ALL SELECT student_id, started_at FROM live_lesson.sessions
                UNION ALL SELECT student_id, ended_at FROM live_lesson.sessions WHERE ended_at IS NOT NULL
                UNION ALL SELECT student_id, submitted_at FROM async_lesson.videos
                UNION ALL SELECT v.student_id, r.returned_at
                    FROM async_lesson.video_reviews r JOIN async_lesson.videos v ON v.video_id = r.video_id
                UNION ALL SELECT student_id, completed_at FROM live_lesson.lesson_completions
                UNION ALL SELECT student_id, completed_at FROM async_lesson.lesson_completions
                UNION ALL SELECT student_id, started_at FROM entitlement.subscriptions
            ) ",
            $query
        )
    };
}

impl SeedReport {
    /// `since`（直近 30 日の始まり）で活動と離脱を分けて読む。
    pub async fn load(pool: &PgPool, since: SystemTime) -> Result<Self, sqlx::Error> {
        let since = OffsetDateTime::from(since);
        let count = async |sql: &'static str| -> Result<i64, sqlx::Error> {
            sqlx::query_scalar(sql).fetch_one(pool).await
        };
        let coaches_by_sport = sqlx::query_as(
            "SELECT sport, count(*) FROM identity.coach_sports GROUP BY sport ORDER BY sport",
        )
        .fetch_all(pool)
        .await?;
        let active_students = sqlx::query_scalar(with_activity!(
            "SELECT count(DISTINCT student_id) FROM activity WHERE at >= $1"
        ))
        .bind(since)
        .fetch_one(pool)
        .await?;
        let students_per_coach = sqlx::query_scalar(
            "SELECT count(DISTINCT student_id) FROM (
                 SELECT b.student_id, s.coach_id
                 FROM scheduling.bookings b JOIN scheduling.slots s ON s.slot_id = b.slot_id
                 UNION ALL SELECT student_id, coach_id FROM async_lesson.videos
             ) parties GROUP BY coach_id ORDER BY 1 DESC",
        )
        .fetch_all(pool)
        .await?;
        let buckets: Vec<(i32, i64)> = sqlx::query_as(with_activity!(
            ", last_activity AS (SELECT student_id, max(at) AS last_at FROM activity GROUP BY student_id),
             first_lesson AS (
                 SELECT student_id, completed_at FROM live_lesson.lesson_completions WHERE is_first_lesson
                 UNION ALL
                 SELECT student_id, completed_at FROM async_lesson.lesson_completions WHERE is_first_lesson
             )
             SELECT CASE
                        WHEN l.last_at - f.completed_at <= interval '30 days' THEN 0
                        WHEN l.last_at - f.completed_at <= interval '90 days' THEN 1
                        WHEN l.last_at - f.completed_at <= interval '180 days' THEN 2
                        ELSE 3
                    END AS bucket,
                    count(*)
             FROM first_lesson f JOIN last_activity l ON l.student_id = f.student_id
             WHERE l.last_at < $1
             GROUP BY bucket"
        ))
        .bind(since)
        .fetch_all(pool)
        .await?;
        let mut churned_after_first_lesson = [0; 4];
        for (bucket, students) in buckets {
            churned_after_first_lesson[bucket as usize] = students;
        }

        Ok(Self {
            coaches: count("SELECT count(*) FROM identity.accounts WHERE role = 'coach'").await?,
            coaches_by_sport,
            students: count("SELECT count(*) FROM identity.accounts WHERE role = 'student'").await?,
            active_students,
            students_per_coach,
            slots: count("SELECT count(*) FROM scheduling.slots").await?,
            bookings: count("SELECT count(*) FROM scheduling.bookings").await?,
            trial_bookings: count("SELECT count(*) FROM scheduling.bookings WHERE is_trial").await?,
            cancelled_bookings: count(
                "SELECT count(*) FROM scheduling.bookings WHERE status = 'cancelled'",
            )
            .await?,
            late_cancellations: count(
                "SELECT count(*) FROM scheduling.bookings b JOIN scheduling.slots s ON s.slot_id = b.slot_id
                 WHERE b.status = 'cancelled' AND b.cancelled_at >= s.starts_at - interval '24 hours'",
            )
            .await?,
            sessions: count("SELECT count(*) FROM live_lesson.sessions").await?,
            low_quality_sessions: count(
                "SELECT count(*) FROM live_lesson.sessions WHERE packet_loss_rate > 1",
            )
            .await?,
            videos: count("SELECT count(*) FROM async_lesson.videos").await?,
            reviews: count("SELECT count(*) FROM async_lesson.video_reviews").await?,
            completions: count(
                "SELECT (SELECT count(*) FROM live_lesson.lesson_completions)
                      + (SELECT count(*) FROM async_lesson.lesson_completions)",
            )
            .await?,
            first_lessons: count(
                "SELECT (SELECT count(*) FROM live_lesson.lesson_completions WHERE is_first_lesson)
                      + (SELECT count(*) FROM async_lesson.lesson_completions WHERE is_first_lesson)",
            )
            .await?,
            subscriptions: count("SELECT count(*) FROM entitlement.subscriptions").await?,
            churned_after_first_lesson,
        })
    }

    /// 上位 [`crate::SeedPlan::TOP_COACHES`] 人のコーチに付いた生徒の割合（0〜1）。
    pub fn top_coach_share(&self) -> f64 {
        let total: i64 = self.students_per_coach.iter().sum();
        let top: i64 = self
            .students_per_coach
            .iter()
            .take(crate::SeedPlan::TOP_COACHES)
            .sum();
        top as f64 / total as f64
    }
}

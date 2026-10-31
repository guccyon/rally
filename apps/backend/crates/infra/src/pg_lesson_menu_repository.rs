//! レッスンメニューの保存先（Postgres の `catalog.lesson_menus`）。

use application::{
    Unavailable,
    catalog::{LessonMenuRepository, lesson_menu_id_uuid},
    identity::account_id_uuid,
};
use domain::{
    catalog::{
        HeadquartersUnpublish, HeadquartersUnpublishReason, LessonDuration, LessonFormat,
        LessonMenu, LessonMenuContent, LessonMenuDescription, LessonMenuId, LessonMenuName,
    },
    identity::AccountId,
};
use sqlx::{
    PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::{
    catalog_values::{lesson_type_from_value, lesson_type_value},
    identity_values::{sport_from_value, sport_value},
    pg_support::timestamp,
};

/// [`LessonMenuRepository`] の Postgres 実装。
///
/// スポーツがコーチの提供スポーツであることは DB でも `lesson_menus_coach_sport_fkey` で守る（判定は domain）。
/// 変更は行の置き換えで、並行な変更は後から保存した方が残る。本部が非公開にしたこと（時刻と理由）も同じ行に持つ
/// （どちらも NULL なら本部は非公開にしていない）。
#[derive(Debug, Clone)]
pub struct PgLessonMenuRepository {
    pool: PgPool,
}

/// メニューの列（lesson_menu_id, coach_id, sport, lesson_type, name, description, duration_minutes, is_published,
/// created_at, headquarters_unpublished_at, headquarters_unpublish_reason）。
type MenuRow = (
    Uuid,
    Uuid,
    String,
    String,
    String,
    String,
    Option<i16>,
    bool,
    OffsetDateTime,
    Option<OffsetDateTime>,
    Option<String>,
);

impl PgLessonMenuRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl LessonMenuRepository for PgLessonMenuRepository {
    fn next_id(&self) -> LessonMenuId {
        lesson_menu_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn insert(&self, menu: LessonMenu) -> Result<(), Unavailable> {
        sqlx::query(
            "INSERT INTO catalog.lesson_menus
                 (lesson_menu_id, coach_id, sport, lesson_type, name, description, duration_minutes,
                  is_published, created_at, headquarters_unpublished_at, headquarters_unpublish_reason)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(lesson_menu_id_uuid::to_uuid(menu.id()))
        .bind(account_id_uuid::to_uuid(menu.coach_id()))
        .bind(sport_value(menu.sport()))
        .bind(lesson_type_value(menu.format().lesson_type()))
        .bind(menu.name().as_str())
        .bind(menu.description().as_str())
        .bind(duration_minutes(&menu))
        .bind(menu.is_published())
        .bind(timestamp(menu.created_at()))
        .bind(headquarters_unpublished_at(&menu))
        .bind(headquarters_unpublish_reason(&menu))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn find(&self, id: LessonMenuId) -> Result<Option<LessonMenu>, Unavailable> {
        let row: Option<MenuRow> = sqlx::query_as(
            "SELECT lesson_menu_id, coach_id, sport, lesson_type, name, description, duration_minutes,
                    is_published, created_at, headquarters_unpublished_at, headquarters_unpublish_reason
             FROM catalog.lesson_menus WHERE lesson_menu_id = $1",
        )
        .bind(lesson_menu_id_uuid::to_uuid(id))
        .fetch_optional(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        row.map(menu_from_row).transpose()
    }

    async fn update(&self, menu: LessonMenu) -> Result<(), Unavailable> {
        sqlx::query(
            "UPDATE catalog.lesson_menus
             SET sport = $2, lesson_type = $3, name = $4, description = $5, duration_minutes = $6,
                 is_published = $7, headquarters_unpublished_at = $8, headquarters_unpublish_reason = $9
             WHERE lesson_menu_id = $1",
        )
        .bind(lesson_menu_id_uuid::to_uuid(menu.id()))
        .bind(sport_value(menu.sport()))
        .bind(lesson_type_value(menu.format().lesson_type()))
        .bind(menu.name().as_str())
        .bind(menu.description().as_str())
        .bind(duration_minutes(&menu))
        .bind(menu.is_published())
        .bind(headquarters_unpublished_at(&menu))
        .bind(headquarters_unpublish_reason(&menu))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn find_by_coach(&self, coach_id: AccountId) -> Result<Vec<LessonMenu>, Unavailable> {
        let rows: Vec<MenuRow> = sqlx::query_as(
            "SELECT lesson_menu_id, coach_id, sport, lesson_type, name, description, duration_minutes,
                    is_published, created_at, headquarters_unpublished_at, headquarters_unpublish_reason
             FROM catalog.lesson_menus WHERE coach_id = $1 ORDER BY created_at",
        )
        .bind(account_id_uuid::to_uuid(coach_id))
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        rows.into_iter().map(menu_from_row).collect()
    }

    async fn find_all(&self) -> Result<Vec<LessonMenu>, Unavailable> {
        let rows: Vec<MenuRow> =
            sqlx::query_as(
            "SELECT lesson_menu_id, coach_id, sport, lesson_type, name, description, duration_minutes,
                    is_published, created_at, headquarters_unpublished_at, headquarters_unpublish_reason
             FROM catalog.lesson_menus",
        )
                .fetch_all(&self.pool)
                .await
                .map_err(Unavailable::new)?;
        rows.into_iter().map(menu_from_row).collect()
    }
}

fn duration_minutes(menu: &LessonMenu) -> Option<i16> {
    menu.format()
        .duration()
        .map(|duration| i16::from(duration.minutes()))
}

fn headquarters_unpublished_at(menu: &LessonMenu) -> Option<OffsetDateTime> {
    menu.headquarters_unpublish()
        .map(|unpublish| timestamp(unpublish.unpublished_at()))
}

fn headquarters_unpublish_reason(menu: &LessonMenu) -> Option<&str> {
    menu.headquarters_unpublish()
        .map(|unpublish| unpublish.reason().as_str())
}

fn menu_from_row(
    (
        lesson_menu_id,
        coach_id,
        sport,
        lesson_type,
        name,
        description,
        duration_minutes,
        is_published,
        created_at,
        headquarters_unpublished_at,
        headquarters_unpublish_reason,
    ): MenuRow,
) -> Result<LessonMenu, Unavailable> {
    let sport = sport_from_value(&sport)
        .ok_or_else(|| Unavailable::new(format!("未知のスポーツ: {sport}")))?;
    let lesson_type = lesson_type_from_value(&lesson_type)
        .ok_or_else(|| Unavailable::new(format!("未知のレッスンの種類: {lesson_type}")))?;
    let duration = duration_minutes
        .map(|minutes| LessonDuration::from_minutes(i64::from(minutes)))
        .transpose()
        .map_err(Unavailable::new)?;
    Ok(LessonMenu::restore(
        lesson_menu_id_uuid::from_uuid(lesson_menu_id),
        account_id_uuid::from_uuid(coach_id),
        LessonMenuContent {
            sport,
            format: LessonFormat::new(lesson_type, duration).map_err(Unavailable::new)?,
            name: LessonMenuName::new(name).map_err(Unavailable::new)?,
            description: LessonMenuDescription::new(description).map_err(Unavailable::new)?,
        },
        is_published,
        created_at.into(),
    )
    .with_headquarters_unpublish(headquarters_unpublish(
        headquarters_unpublished_at,
        headquarters_unpublish_reason,
    )?))
}

/// 本部が非公開にした時刻と理由の列から組み立てる。両方 NULL なら本部は非公開にしていない
/// （片方だけ NULL の行は `lesson_menus_headquarters_unpublish_shape` で入らない）。
fn headquarters_unpublish(
    unpublished_at: Option<OffsetDateTime>,
    reason: Option<String>,
) -> Result<Option<HeadquartersUnpublish>, Unavailable> {
    match (unpublished_at, reason) {
        (None, None) => Ok(None),
        (Some(unpublished_at), Some(reason)) => Ok(Some(HeadquartersUnpublish::new(
            HeadquartersUnpublishReason::new(reason).map_err(Unavailable::new)?,
            unpublished_at.into(),
        ))),
        _ => Err(Unavailable::new(
            "本部が非公開にした時刻と理由の片方だけがある",
        )),
    }
}

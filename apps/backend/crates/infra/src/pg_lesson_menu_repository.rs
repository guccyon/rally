//! レッスンメニューの保存先（Postgres の `catalog.lesson_menus`）。

use application::{
    Unavailable,
    catalog::{LessonMenuRepository, lesson_menu_id_uuid},
    identity::account_id_uuid,
};
use domain::{
    catalog::{
        LessonDuration, LessonMenu, LessonMenuContent, LessonMenuDescription, LessonMenuId,
        LessonMenuName,
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
#[derive(Debug, Clone)]
pub struct PgLessonMenuRepository {
    pool: PgPool,
}

/// メニューの列（lesson_menu_id, coach_id, sport, lesson_type, name, description, duration_minutes, is_published,
/// created_at, updated_at）。
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
    OffsetDateTime,
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
                  is_published, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(lesson_menu_id_uuid::to_uuid(menu.id()))
        .bind(account_id_uuid::to_uuid(menu.coach_id()))
        .bind(sport_value(menu.sport()))
        .bind(lesson_type_value(menu.lesson_type()))
        .bind(menu.name().as_str())
        .bind(menu.description().as_str())
        .bind(duration_minutes(&menu))
        .bind(menu.is_published())
        .bind(timestamp(menu.created_at()))
        .bind(timestamp(menu.updated_at()))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn find(&self, id: LessonMenuId) -> Result<Option<LessonMenu>, Unavailable> {
        let row: Option<MenuRow> =
            sqlx::query_as(
            "SELECT lesson_menu_id, coach_id, sport, lesson_type, name, description, duration_minutes,
                    is_published, created_at, updated_at
             FROM catalog.lesson_menus WHERE lesson_menu_id = $1",
        )
                .bind(lesson_menu_id_uuid::to_uuid(id))
                .fetch_optional(&self.pool)
                .await
                .map_err(Unavailable::new)?;
        row.map(menu_from_row).transpose()
    }

    async fn update(&self, menu: LessonMenu) -> Result<(), Unavailable> {
        // コーチ・スポーツ・レッスンの種類・作った時刻は変えない（domain の LessonMenu::revise が変えないもの）
        sqlx::query(
            "UPDATE catalog.lesson_menus
             SET name = $2, description = $3, duration_minutes = $4, is_published = $5, updated_at = $6
             WHERE lesson_menu_id = $1",
        )
        .bind(lesson_menu_id_uuid::to_uuid(menu.id()))
        .bind(menu.name().as_str())
        .bind(menu.description().as_str())
        .bind(duration_minutes(&menu))
        .bind(menu.is_published())
        .bind(timestamp(menu.updated_at()))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn find_by_coach(&self, coach_id: AccountId) -> Result<Vec<LessonMenu>, Unavailable> {
        let rows: Vec<MenuRow> = sqlx::query_as(
            "SELECT lesson_menu_id, coach_id, sport, lesson_type, name, description, duration_minutes,
                    is_published, created_at, updated_at
             FROM catalog.lesson_menus WHERE coach_id = $1",
        )
            .bind(account_id_uuid::to_uuid(coach_id))
            .fetch_all(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        rows.into_iter().map(menu_from_row).collect()
    }
}

/// `duration_minutes` の列に書く値。
fn duration_minutes(menu: &LessonMenu) -> Option<i16> {
    menu.duration()
        .map(|duration| i16::from(duration.minutes()))
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
        updated_at,
    ): MenuRow,
) -> Result<LessonMenu, Unavailable> {
    let sport = sport_from_value(&sport)
        .ok_or_else(|| Unavailable::new(format!("未知の sport: {sport}")))?;
    let lesson_type = lesson_type_from_value(&lesson_type)
        .ok_or_else(|| Unavailable::new(format!("未知の lesson_type: {lesson_type}")))?;
    Ok(LessonMenu::restore(
        lesson_menu_id_uuid::from_uuid(lesson_menu_id),
        account_id_uuid::from_uuid(coach_id),
        sport,
        lesson_type,
        LessonMenuContent {
            name: LessonMenuName::new(name).map_err(Unavailable::new)?,
            description: LessonMenuDescription::new(description).map_err(Unavailable::new)?,
            duration: duration_minutes
                .map(|minutes| LessonDuration::new(i64::from(minutes)))
                .transpose()
                .map_err(Unavailable::new)?,
            is_published,
        },
        created_at.into(),
        updated_at.into(),
    ))
}

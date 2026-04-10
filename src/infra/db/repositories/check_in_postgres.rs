use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::entities::{DailyCheckIn, UserStreak};
use crate::domain::repositories::{CheckInRepository, RepoError};
use crate::domain::value_objects::UserId;
use crate::infra::db::postgres_connection::PgPool;
use crate::infra::db::schema::{daily_check_ins, user_streaks};

use super::error_mapping::{map_diesel_error, map_pool_error};

#[derive(Queryable, Selectable)]
#[diesel(table_name = user_streaks)]
struct UserStreakRow {
    user_id: Uuid,
    current_streak: i32,
    last_check_in_date: Option<NaiveDate>,
    updated_at: DateTime<Utc>,
}

impl UserStreakRow {
    fn into_entity(self) -> UserStreak {
        UserStreak::from_existing(
            UserId::from_uuid(self.user_id),
            self.current_streak,
            self.last_check_in_date,
            self.updated_at,
        )
    }
}

#[derive(Insertable)]
#[diesel(table_name = user_streaks)]
struct UpsertUserStreakRow<'a> {
    user_id: &'a Uuid,
    current_streak: i32,
    last_check_in_date: Option<NaiveDate>,
    updated_at: DateTime<Utc>,
}

impl<'a> UpsertUserStreakRow<'a> {
    fn from_entity(entity: &'a UserStreak) -> Self {
        Self {
            user_id: entity.user_id().as_uuid(),
            current_streak: entity.current_streak(),
            last_check_in_date: entity.last_check_in_date(),
            updated_at: *entity.updated_at(),
        }
    }
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = daily_check_ins)]
struct DailyCheckInRow {
    id: Uuid,
    user_id: Uuid,
    checked_in_date: NaiveDate,
    streak_day: i32,
    credits_earned: i32,
    created_at: DateTime<Utc>,
}

impl DailyCheckInRow {
    fn into_entity(self) -> DailyCheckIn {
        DailyCheckIn::from_existing(
            self.id,
            UserId::from_uuid(self.user_id),
            self.checked_in_date,
            self.streak_day,
            self.credits_earned,
            self.created_at,
        )
    }
}

#[derive(Insertable)]
#[diesel(table_name = daily_check_ins)]
struct NewDailyCheckInRow<'a> {
    id: &'a Uuid,
    user_id: &'a Uuid,
    checked_in_date: NaiveDate,
    streak_day: i32,
    credits_earned: i32,
    created_at: DateTime<Utc>,
}

impl<'a> NewDailyCheckInRow<'a> {
    fn from_entity(entity: &'a DailyCheckIn) -> Self {
        Self {
            id: entity.id(),
            user_id: entity.user_id().as_uuid(),
            checked_in_date: entity.checked_in_date(),
            streak_day: entity.streak_day(),
            credits_earned: entity.credits_earned(),
            created_at: *entity.created_at(),
        }
    }
}

pub struct CheckInPostgres {
    pool: Arc<PgPool>,
}

impl CheckInPostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CheckInRepository for CheckInPostgres {
    async fn find_streak_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<UserStreak>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = user_streaks::table
            .find(user_id.as_uuid())
            .first::<UserStreakRow>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("check_in.find_streak_by_user_id", e))?;

        Ok(result.map(|row| row.into_entity()))
    }

    async fn upsert_streak(&self, streak: &UserStreak) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let row = UpsertUserStreakRow::from_entity(streak);

        diesel::insert_into(user_streaks::table)
            .values(&row)
            .on_conflict(user_streaks::user_id)
            .do_update()
            .set((
                user_streaks::current_streak
                    .eq(diesel::upsert::excluded(user_streaks::current_streak)),
                user_streaks::last_check_in_date
                    .eq(diesel::upsert::excluded(user_streaks::last_check_in_date)),
                user_streaks::updated_at.eq(diesel::upsert::excluded(user_streaks::updated_at)),
            ))
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("check_in.upsert_streak", e))?;

        Ok(())
    }

    async fn create_check_in(&self, check_in: &DailyCheckIn) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let new_row = NewDailyCheckInRow::from_entity(check_in);

        diesel::insert_into(daily_check_ins::table)
            .values(&new_row)
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("check_in.create_check_in", e))?;

        Ok(())
    }

    async fn find_check_ins_by_user_id(
        &self,
        user_id: &UserId,
        limit: i64,
    ) -> Result<Vec<DailyCheckIn>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let rows = daily_check_ins::table
            .filter(daily_check_ins::user_id.eq(user_id.as_uuid()))
            .order(daily_check_ins::checked_in_date.desc())
            .limit(limit)
            .load::<DailyCheckInRow>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("check_in.find_check_ins_by_user_id", e))?;

        Ok(rows.into_iter().map(|row| row.into_entity()).collect())
    }
}

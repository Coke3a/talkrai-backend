use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::entities::User;
use crate::domain::repositories::{ReengagementTarget, RepoError, UserRepository};
use crate::domain::value_objects::{UserId, UserStatus};
use crate::infra::db::postgres_connection::PgPool;
use crate::infra::db::schema::users;

use super::error_mapping::{map_diesel_error, map_pool_error};

#[derive(Queryable, Selectable)]
#[diesel(table_name = users)]
struct UserRow {
    id: Uuid,
    line_user_id: String,
    display_name: String,
    picture_url: Option<String>,
    language: String,
    status: String,
    terms_accepted_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    check_in_streak: i32,
    longest_streak: i32,
    last_check_in_on: Option<NaiveDate>,
    last_reminder_sent_on: Option<NaiveDate>,
}

impl UserRow {
    fn into_entity(self) -> User {
        let status: UserStatus = self.status.parse().unwrap_or(UserStatus::Active);
        User::from_existing(
            UserId::from_uuid(self.id),
            self.line_user_id,
            self.display_name,
            self.picture_url,
            self.language,
            status,
            self.terms_accepted_at,
            self.created_at,
            self.updated_at,
            self.check_in_streak,
            self.longest_streak,
            self.last_check_in_on,
            self.last_reminder_sent_on,
        )
    }
}

#[derive(Insertable)]
#[diesel(table_name = users)]
struct NewUserRow<'a> {
    id: &'a Uuid,
    line_user_id: &'a str,
    display_name: &'a str,
    picture_url: Option<&'a str>,
    language: &'a str,
    status: &'a str,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl<'a> NewUserRow<'a> {
    fn from_entity(entity: &'a User) -> Self {
        Self {
            id: entity.id().as_uuid(),
            line_user_id: entity.line_user_id(),
            display_name: entity.display_name(),
            picture_url: entity.picture_url(),
            language: entity.language(),
            status: entity.status().as_str(),
            created_at: *entity.created_at(),
            updated_at: *entity.updated_at(),
        }
    }
}

pub struct UserPostgres {
    pool: Arc<PgPool>,
}

impl UserPostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for UserPostgres {
    async fn find_by_id(&self, id: &UserId) -> Result<Option<User>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = users::table
            .find(id.as_uuid())
            .first::<UserRow>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("user.find_by_id", e))?;

        Ok(result.map(|row| row.into_entity()))
    }

    async fn find_by_line_user_id(&self, line_user_id: &str) -> Result<Option<User>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = users::table
            .filter(users::line_user_id.eq(line_user_id))
            .first::<UserRow>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("user.find_by_line_user_id", e))?;

        Ok(result.map(|row| row.into_entity()))
    }

    async fn update(&self, user: &User) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        diesel::update(users::table.find(user.id().as_uuid()))
            .set((
                users::display_name.eq(user.display_name()),
                users::picture_url.eq(user.picture_url()),
                users::status.eq(user.status().as_str()),
                users::terms_accepted_at.eq(user.terms_accepted_at().copied()),
                users::check_in_streak.eq(user.check_in_streak()),
                users::longest_streak.eq(user.longest_streak()),
                users::last_check_in_on.eq(user.last_check_in_on()),
                users::last_reminder_sent_on.eq(user.last_reminder_sent_on()),
                users::updated_at.eq(user.updated_at()),
            ))
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("user.update", e))?;

        Ok(())
    }

    async fn upsert(&self, user: &User) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let new_row = NewUserRow::from_entity(user);

        diesel::insert_into(users::table)
            .values(&new_row)
            .on_conflict(users::line_user_id)
            .do_update()
            .set((
                users::display_name.eq(user.display_name()),
                users::picture_url.eq(user.picture_url()),
                users::status.eq(user.status().as_str()),
                users::updated_at.eq(user.updated_at()),
            ))
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("user.upsert", e))?;

        Ok(())
    }

    async fn find_reengagement_targets(
        &self,
        today: NaiveDate,
        window_days: i64,
        batch_cap: i64,
    ) -> Result<Vec<ReengagementTarget>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        let window_start = today - chrono::Duration::days(window_days);

        // NULL last_check_in_on is excluded by the `< today` comparison (NULL is not < today),
        // which doubles as "engaged at least once". "Not reminded today" allows NULL or < today.
        let rows: Vec<(Uuid, String)> = users::table
            .filter(users::status.eq("active"))
            .filter(users::last_check_in_on.lt(today))
            .filter(users::last_check_in_on.ge(window_start))
            .filter(
                users::last_reminder_sent_on
                    .lt(today)
                    .or(users::last_reminder_sent_on.is_null().nullable()),
            )
            .order(users::last_check_in_on.desc())
            .limit(batch_cap)
            .select((users::id, users::line_user_id))
            .load::<(Uuid, String)>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("user.find_reengagement_targets", e))?;

        Ok(rows
            .into_iter()
            .map(|(id, line_user_id)| ReengagementTarget {
                user_id: UserId::from_uuid(id),
                line_user_id,
            })
            .collect())
    }

    async fn mark_reminded(&self, user_ids: &[UserId], today: NaiveDate) -> Result<(), RepoError> {
        if user_ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        let uuids: Vec<Uuid> = user_ids.iter().map(|id| *id.as_uuid()).collect();

        diesel::update(users::table.filter(users::id.eq_any(uuids)))
            .set(users::last_reminder_sent_on.eq(today))
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("user.mark_reminded", e))?;

        Ok(())
    }
}

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::entities::User;
use crate::domain::repositories::{RepoError, UserRepository};
use crate::domain::value_objects::UserId;
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
    terms_accepted_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl UserRow {
    fn into_entity(self) -> User {
        User::from_existing(
            UserId::from_uuid(self.id),
            self.line_user_id,
            self.display_name,
            self.picture_url,
            self.language,
            self.terms_accepted_at,
            self.created_at,
            self.updated_at,
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
                users::terms_accepted_at.eq(user.terms_accepted_at().copied()),
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
                users::updated_at.eq(user.updated_at()),
            ))
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("user.upsert", e))?;

        Ok(())
    }
}

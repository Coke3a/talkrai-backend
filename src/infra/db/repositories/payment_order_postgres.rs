use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use std::str::FromStr;

use crate::domain::entities::PaymentOrder;
use crate::domain::repositories::{PaymentOrderRepository, RepoError};
use crate::domain::value_objects::{PaymentOrderId, PaymentOrderStatus, UserId};
use crate::infra::db::postgres_connection::PgPool;
use crate::infra::db::schema::payment_orders;

use super::error_mapping::{map_diesel_error, map_pool_error};

#[derive(Queryable, Selectable)]
#[diesel(table_name = payment_orders)]
struct PaymentOrderRow {
    id: Uuid,
    user_id: Uuid,
    beam_payment_link_id: String,
    package_id: String,
    credits_amount: i32,
    price_thb: i32,
    status: String,
    beam_status: Option<String>,
    redirect_url: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PaymentOrderRow {
    fn into_entity(self) -> PaymentOrder {
        PaymentOrder::from_existing(
            PaymentOrderId::from_uuid(self.id),
            UserId::from_uuid(self.user_id),
            self.beam_payment_link_id,
            self.package_id,
            self.credits_amount,
            self.price_thb,
            PaymentOrderStatus::from_str(&self.status).expect("invalid payment_order_status in DB"),
            self.beam_status,
            self.redirect_url,
            self.created_at,
            self.updated_at,
        )
    }
}

#[derive(Insertable)]
#[diesel(table_name = payment_orders)]
struct NewPaymentOrderRow<'a> {
    id: &'a Uuid,
    user_id: &'a Uuid,
    beam_payment_link_id: &'a str,
    package_id: &'a str,
    credits_amount: i32,
    price_thb: i32,
    status: &'a str,
    beam_status: Option<&'a str>,
    redirect_url: Option<&'a str>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl<'a> NewPaymentOrderRow<'a> {
    fn from_entity(entity: &'a PaymentOrder) -> Self {
        Self {
            id: entity.id_uuid(),
            user_id: entity.user_id().as_uuid(),
            beam_payment_link_id: entity.beam_payment_link_id(),
            package_id: entity.package_id(),
            credits_amount: entity.credits_amount(),
            price_thb: entity.price_thb(),
            status: entity.status().as_str(),
            beam_status: entity.beam_status(),
            redirect_url: entity.redirect_url(),
            created_at: *entity.created_at(),
            updated_at: *entity.updated_at(),
        }
    }
}

pub struct PaymentOrderPostgres {
    pool: Arc<PgPool>,
}

impl PaymentOrderPostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PaymentOrderRepository for PaymentOrderPostgres {
    async fn settle_and_credit(
        &self,
        id: &PaymentOrderId,
        beam_status: &str,
    ) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        diesel::sql_query("SELECT settle_payment($1,$2)")
            .bind::<diesel::sql_types::Uuid, _>(id.as_uuid())
            .bind::<diesel::sql_types::Text, _>(beam_status)
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("payment.settle", e))?;
        Ok(())
    }

    async fn create(&self, order: &PaymentOrder) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let new_row = NewPaymentOrderRow::from_entity(order);

        diesel::insert_into(payment_orders::table)
            .values(&new_row)
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("payment_order.create", e))?;

        Ok(())
    }

    async fn find_by_id(&self, id: &PaymentOrderId) -> Result<Option<PaymentOrder>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = payment_orders::table
            .filter(payment_orders::id.eq(id.as_uuid()))
            .first::<PaymentOrderRow>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("payment_order.find_by_id", e))?;

        Ok(result.map(|row| row.into_entity()))
    }

    async fn find_by_beam_payment_link_id(
        &self,
        beam_id: &str,
    ) -> Result<Option<PaymentOrder>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = payment_orders::table
            .filter(payment_orders::beam_payment_link_id.eq(beam_id))
            .first::<PaymentOrderRow>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("payment_order.find_by_beam_id", e))?;

        Ok(result.map(|row| row.into_entity()))
    }

    async fn update_status(
        &self,
        id: &PaymentOrderId,
        status: &PaymentOrderStatus,
        beam_status: Option<&str>,
    ) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        diesel::update(payment_orders::table.filter(payment_orders::id.eq(id.as_uuid())))
            .set((
                payment_orders::status.eq(status.as_str()),
                payment_orders::beam_status.eq(beam_status),
                payment_orders::updated_at.eq(Utc::now()),
            ))
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("payment_order.update_status", e))?;

        Ok(())
    }

    async fn update_beam_payment_link_id(
        &self,
        id: &PaymentOrderId,
        beam_payment_link_id: &str,
    ) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        diesel::update(payment_orders::table.filter(payment_orders::id.eq(id.as_uuid())))
            .set((
                payment_orders::beam_payment_link_id.eq(beam_payment_link_id),
                payment_orders::updated_at.eq(Utc::now()),
            ))
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("payment_order.update_beam_id", e))?;

        Ok(())
    }
}

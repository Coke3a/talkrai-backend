use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::scoped_futures::ScopedFutureExt;
use diesel_async::{AsyncConnection, RunQueryDsl};
use uuid::Uuid;

use std::str::FromStr;

use crate::domain::entities::{CreditBalance, CreditTransaction};
use crate::domain::repositories::{CreditRepository, RepoError};
use crate::domain::value_objects::{CreditTransactionId, CreditTransactionType, UserId};
use crate::infra::db::postgres_connection::PgPool;
use crate::infra::db::schema::{credit_balances, credit_transactions};

use super::error_mapping::{map_diesel_error, map_pool_error};

#[derive(Queryable, Selectable)]
#[diesel(table_name = credit_balances)]
struct CreditBalanceRow {
    id: Uuid,
    user_id: Uuid,
    balance: i32,
    total_purchased: i32,
    total_consumed: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl CreditBalanceRow {
    fn into_entity(self) -> CreditBalance {
        CreditBalance::from_existing(
            self.id,
            UserId::from_uuid(self.user_id),
            self.balance,
            self.total_purchased,
            self.total_consumed,
            self.created_at,
            self.updated_at,
        )
    }
}

#[derive(Insertable)]
#[diesel(table_name = credit_balances)]
struct NewCreditBalanceRow<'a> {
    id: &'a Uuid,
    user_id: &'a Uuid,
    balance: i32,
    total_purchased: i32,
    total_consumed: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl<'a> NewCreditBalanceRow<'a> {
    fn from_entity(entity: &'a CreditBalance) -> Self {
        Self {
            id: entity.id(),
            user_id: entity.user_id().as_uuid(),
            balance: entity.balance(),
            total_purchased: entity.total_purchased(),
            total_consumed: entity.total_consumed(),
            created_at: *entity.created_at(),
            updated_at: *entity.updated_at(),
        }
    }
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = credit_transactions)]
struct CreditTransactionRow {
    id: Uuid,
    user_id: Uuid,
    #[diesel(column_name = type_)]
    type_: String,
    amount: i32,
    balance_after: i32,
    reference_id: Option<Uuid>,
    description: Option<String>,
    created_at: DateTime<Utc>,
}

impl CreditTransactionRow {
    fn into_entity(self) -> CreditTransaction {
        CreditTransaction::from_existing(
            CreditTransactionId::from_uuid(self.id),
            UserId::from_uuid(self.user_id),
            CreditTransactionType::from_str(&self.type_)
                .expect("invalid credit_transaction_type in DB"),
            self.amount,
            self.balance_after,
            self.reference_id,
            self.description,
            self.created_at,
        )
    }
}

#[derive(Insertable)]
#[diesel(table_name = credit_transactions)]
struct NewCreditTransactionRow<'a> {
    id: &'a Uuid,
    user_id: &'a Uuid,
    #[diesel(column_name = type_)]
    type_: &'a str,
    amount: i32,
    balance_after: i32,
    reference_id: Option<&'a Uuid>,
    description: Option<&'a str>,
    created_at: DateTime<Utc>,
}

impl<'a> NewCreditTransactionRow<'a> {
    fn from_entity(entity: &'a CreditTransaction) -> Self {
        Self {
            id: entity.id().as_uuid(),
            user_id: entity.user_id().as_uuid(),
            type_: entity.transaction_type().as_str(),
            amount: entity.amount(),
            balance_after: entity.balance_after(),
            reference_id: entity.reference_id(),
            description: entity.description(),
            created_at: *entity.created_at(),
        }
    }
}

pub struct CreditPostgres {
    pool: Arc<PgPool>,
}

impl CreditPostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CreditRepository for CreditPostgres {
    async fn find_balance_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<CreditBalance>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = credit_balances::table
            .filter(credit_balances::user_id.eq(user_id.as_uuid()))
            .first::<CreditBalanceRow>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("credit.find_balance_by_user_id", e))?;

        Ok(result.map(|row| row.into_entity()))
    }

    async fn create_balance(&self, balance: &CreditBalance) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let new_row = NewCreditBalanceRow::from_entity(balance);

        diesel::insert_into(credit_balances::table)
            .values(&new_row)
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("credit.create_balance", e))?;

        Ok(())
    }

    async fn deduct_and_log(
        &self,
        user_id: &UserId,
        amount: i32,
        transaction: &CreditTransaction,
    ) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let user_uuid = *user_id.as_uuid();
        let now = Utc::now();
        let new_txn = NewCreditTransactionRow::from_entity(transaction);

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            async move {
                let rows_affected = diesel::update(
                    credit_balances::table.filter(credit_balances::user_id.eq(user_uuid)),
                )
                .set((
                    credit_balances::balance.eq(credit_balances::balance - amount),
                    credit_balances::total_consumed.eq(credit_balances::total_consumed + amount),
                    credit_balances::updated_at.eq(now),
                ))
                .execute(conn)
                .await?;

                if rows_affected == 0 {
                    return Err(diesel::result::Error::NotFound);
                }

                diesel::insert_into(credit_transactions::table)
                    .values(&new_txn)
                    .execute(conn)
                    .await?;

                Ok(())
            }
            .scope_boxed()
        })
        .await
        .map_err(|e| map_diesel_error("credit.deduct_and_log", e))?;

        Ok(())
    }

    async fn add_and_log(
        &self,
        user_id: &UserId,
        amount: i32,
        transaction: &CreditTransaction,
    ) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let user_uuid = *user_id.as_uuid();
        let now = Utc::now();
        let new_txn = NewCreditTransactionRow::from_entity(transaction);

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            async move {
                let rows_affected = diesel::update(
                    credit_balances::table.filter(credit_balances::user_id.eq(user_uuid)),
                )
                .set((
                    credit_balances::balance.eq(credit_balances::balance + amount),
                    credit_balances::total_purchased.eq(credit_balances::total_purchased + amount),
                    credit_balances::updated_at.eq(now),
                ))
                .execute(conn)
                .await?;

                if rows_affected == 0 {
                    return Err(diesel::result::Error::NotFound);
                }

                diesel::insert_into(credit_transactions::table)
                    .values(&new_txn)
                    .execute(conn)
                    .await?;

                Ok(())
            }
            .scope_boxed()
        })
        .await
        .map_err(|e| map_diesel_error("credit.add_and_log", e))?;

        Ok(())
    }

    async fn find_transactions_by_user_id(
        &self,
        user_id: &UserId,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<CreditTransaction>, i64), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let total = credit_transactions::table
            .filter(credit_transactions::user_id.eq(user_id.as_uuid()))
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("credit.count_transactions_by_user_id", e))?;

        let rows = credit_transactions::table
            .filter(credit_transactions::user_id.eq(user_id.as_uuid()))
            .order(credit_transactions::created_at.desc())
            .limit(limit)
            .offset(offset)
            .load::<CreditTransactionRow>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("credit.find_transactions_by_user_id", e))?;

        let transactions = rows.into_iter().map(|row| row.into_entity()).collect();

        Ok((transactions, total))
    }
}

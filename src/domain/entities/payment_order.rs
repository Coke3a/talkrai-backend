use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value_objects::{PaymentOrderId, PaymentOrderStatus, UserId};

pub struct PaymentOrder {
    id: PaymentOrderId,
    user_id: UserId,
    beam_payment_link_id: String,
    package_id: String,
    credits_amount: i32,
    price_thb: i32,
    status: PaymentOrderStatus,
    beam_status: Option<String>,
    redirect_url: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PaymentOrder {
    pub fn new(
        user_id: UserId,
        package_id: String,
        credits_amount: i32,
        price_thb: i32,
        redirect_url: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: PaymentOrderId::new(),
            user_id,
            beam_payment_link_id: String::new(),
            package_id,
            credits_amount,
            price_thb,
            status: PaymentOrderStatus::Pending,
            beam_status: None,
            redirect_url,
            created_at: now,
            updated_at: now,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_existing(
        id: PaymentOrderId,
        user_id: UserId,
        beam_payment_link_id: String,
        package_id: String,
        credits_amount: i32,
        price_thb: i32,
        status: PaymentOrderStatus,
        beam_status: Option<String>,
        redirect_url: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            user_id,
            beam_payment_link_id,
            package_id,
            credits_amount,
            price_thb,
            status,
            beam_status,
            redirect_url,
            created_at,
            updated_at,
        }
    }

    pub fn set_beam_payment_link_id(&mut self, beam_id: String) {
        self.beam_payment_link_id = beam_id;
        self.updated_at = Utc::now();
    }

    pub fn mark_completed(&mut self, beam_status: Option<String>) {
        self.status = PaymentOrderStatus::Completed;
        self.beam_status = beam_status;
        self.updated_at = Utc::now();
    }

    pub fn mark_failed(&mut self, beam_status: Option<String>) {
        self.status = PaymentOrderStatus::Failed;
        self.beam_status = beam_status;
        self.updated_at = Utc::now();
    }

    // Getters
    pub fn id(&self) -> &PaymentOrderId {
        &self.id
    }
    pub fn id_uuid(&self) -> &Uuid {
        self.id.as_uuid()
    }
    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }
    pub fn beam_payment_link_id(&self) -> &str {
        &self.beam_payment_link_id
    }
    pub fn package_id(&self) -> &str {
        &self.package_id
    }
    pub fn credits_amount(&self) -> i32 {
        self.credits_amount
    }
    pub fn price_thb(&self) -> i32 {
        self.price_thb
    }
    pub fn status(&self) -> &PaymentOrderStatus {
        &self.status
    }
    pub fn beam_status(&self) -> Option<&str> {
        self.beam_status.as_deref()
    }
    pub fn redirect_url(&self) -> Option<&str> {
        self.redirect_url.as_deref()
    }
    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }
    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}

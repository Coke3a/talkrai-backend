use async_trait::async_trait;

use super::line_client_error::LineClientError;

#[async_trait]
pub trait LineClient: Send + Sync {
    /// Verify LINE webhook signature
    fn verify_signature(&self, body: &[u8], signature: &str) -> Result<bool, LineClientError>;

    /// Push messages to user with sender override
    async fn push_messages(
        &self,
        line_user_id: &str,
        messages: Vec<LineMessage>,
    ) -> Result<(), LineClientError>;

    /// Get LINE user profile
    async fn get_profile(&self, line_user_id: &str) -> Result<LineProfile, LineClientError>;

    /// Link rich menu to user
    async fn link_rich_menu(
        &self,
        line_user_id: &str,
        rich_menu_id: &str,
    ) -> Result<(), LineClientError>;

    /// Unlink rich menu from user
    async fn unlink_rich_menu(&self, line_user_id: &str) -> Result<(), LineClientError>;

    /// Show typing indicator (loading animation) in chat
    async fn show_loading_animation(
        &self,
        line_user_id: &str,
        loading_seconds: Option<u32>,
    ) -> Result<(), LineClientError>;

    /// Verify LIFF access token by calling LINE Profile API with user's token
    async fn verify_liff_token(&self, access_token: &str) -> Result<LineProfile, LineClientError>;

    /// Reply to a webhook event using a reply token (free, no push quota consumed)
    async fn reply_messages(
        &self,
        reply_token: &str,
        messages: Vec<LineReplyMessage>,
    ) -> Result<(), LineClientError>;
}

pub enum LineReplyMessage {
    Text {
        text: String,
    },
    Flex {
        alt_text: String,
        contents: serde_json::Value,
    },
}

pub enum LineMessage {
    Text {
        text: String,
        sender_name: String,
        sender_icon_url: String,
    },
    Flex {
        alt_text: String,
        contents: serde_json::Value,
        sender_name: String,
        sender_icon_url: String,
        /// Optional quick-reply option texts (plain strings, not yet LINE JSON).
        /// Serialized + validated against the LINE quick-reply spec at the push
        /// boundary (`build_quick_reply_object`); `None` => no quick reply attached.
        quick_reply: Option<Vec<String>>,
    },
}

pub struct LineProfile {
    pub user_id: String,
    pub display_name: String,
    pub picture_url: Option<String>,
    pub language: Option<String>,
}

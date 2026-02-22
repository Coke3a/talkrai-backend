use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::repositories::UserRepository;
use crate::domain::services::line_client::LineClient;
use crate::usecases::UsecaseError;

pub struct AcceptTermsInput {
    pub line_user_id: String,
    pub rich_menu_a_id: String,
}

pub struct AcceptTermsOutput {
    pub user_id: Uuid,
    pub terms_accepted_at: DateTime<Utc>,
}

pub struct AcceptTermsUseCase {
    user_repo: Arc<dyn UserRepository>,
    line_client: Arc<dyn LineClient>,
}

impl AcceptTermsUseCase {
    pub fn new(user_repo: Arc<dyn UserRepository>, line_client: Arc<dyn LineClient>) -> Self {
        Self {
            user_repo,
            line_client,
        }
    }

    pub async fn execute(
        &self,
        input: AcceptTermsInput,
    ) -> Result<AcceptTermsOutput, UsecaseError> {
        let mut user = self
            .user_repo
            .find_by_line_user_id(&input.line_user_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("User not found".into()))?;

        user.accept_terms()?;
        self.user_repo.update(&user).await?;

        // Best-effort: link rich menu A
        if let Err(e) = self
            .line_client
            .link_rich_menu(&input.line_user_id, &input.rich_menu_a_id)
            .await
        {
            tracing::warn!(
                error = %e,
                line_user_id = %input.line_user_id,
                "Failed to link rich menu A after accepting terms"
            );
        }

        Ok(AcceptTermsOutput {
            user_id: *user.id().as_uuid(),
            terms_accepted_at: *user.terms_accepted_at().unwrap(),
        })
    }
}

use std::sync::Arc;

use crate::domain::repositories::UserRepository;
use crate::usecases::liff::require_active_user::require_active_user;
use crate::usecases::UsecaseError;

pub struct GetMeInput {
    pub line_user_id: String,
}

pub struct GetMeOutput {
    /// Whether the user has already accepted the Terms of Service. The LIFF app uses this to
    /// decide whether to show the one-time consent gate before starting a session.
    pub terms_accepted: bool,
}

pub struct GetMeUseCase {
    user_repo: Arc<dyn UserRepository>,
}

impl GetMeUseCase {
    pub fn new(user_repo: Arc<dyn UserRepository>) -> Self {
        Self { user_repo }
    }

    pub async fn execute(&self, input: GetMeInput) -> Result<GetMeOutput, UsecaseError> {
        let user = require_active_user(&*self.user_repo, &input.line_user_id).await?;

        Ok(GetMeOutput {
            terms_accepted: user.has_accepted_terms(),
        })
    }
}

use crate::domain::entities::User;
use crate::domain::repositories::UserRepository;
use crate::usecases::UsecaseError;

pub async fn require_active_user(
    user_repo: &dyn UserRepository,
    line_user_id: &str,
) -> Result<User, UsecaseError> {
    let user = user_repo
        .find_by_line_user_id(line_user_id)
        .await?
        .ok_or_else(|| UsecaseError::NotFound("User not found".to_string()))?;

    if !user.is_active() {
        return Err(UsecaseError::UserInactive);
    }

    Ok(user)
}

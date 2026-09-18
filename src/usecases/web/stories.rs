use crate::domain::web::{Persona, StoryMutation, TurnRepository, WebDataRepository, WebError};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;
pub struct WebStories {
    pub data: Arc<dyn WebDataRepository>,
    pub turns: Arc<dyn TurnRepository>,
}
impl WebStories {
    pub async fn mutate(
        &self,
        owner: Uuid,
        operation: &str,
        mut input: StoryMutation,
        key: &str,
    ) -> Result<Value, WebError> {
        if let Some(persona) = input.persona {
            input.persona = Some(Persona::new(persona.name, persona.description)?);
        }
        if operation == "create" && (input.scene_id.is_none() || input.persona.is_none())
            || operation != "create"
                && (input.session_id.is_none() || input.expected_version.is_none())
            || operation == "persona" && input.persona.is_none()
        {
            return Err(WebError::Rejected("VALIDATION_ERROR"));
        }
        if matches!(operation, "create" | "transfer") {
            validate_key(key)?;
        }
        self.data.mutate_story(owner, operation, input, key).await
    }
    pub async fn turn(
        &self,
        owner: Uuid,
        session: Uuid,
        kind: &str,
        key: &str,
        input: Value,
    ) -> Result<Value, WebError> {
        validate_key(key)?;
        if input["expected_version"]
            .as_i64()
            .filter(|v| *v >= 0)
            .is_none()
        {
            return Err(WebError::Rejected("VALIDATION_ERROR"));
        }
        if kind == "turn" {
            let content = input["content"].as_str().unwrap_or("");
            if content.trim().is_empty() || content.chars().count() > 4000 {
                return Err(WebError::Rejected("VALIDATION_ERROR"));
            }
        } else if input["message_id"]
            .as_str()
            .and_then(|s| s.parse::<Uuid>().ok())
            .is_none()
        {
            return Err(WebError::Rejected("VALIDATION_ERROR"));
        }
        self.turns
            .admit(owner, session, "web", kind, key, input)
            .await
    }
}
pub fn validate_key(key: &str) -> Result<(), WebError> {
    if key.is_empty()
        || key.len() > 128
        || !key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_:".contains(&c))
    {
        Err(WebError::Rejected("VALIDATION_ERROR"))
    } else {
        Ok(())
    }
}

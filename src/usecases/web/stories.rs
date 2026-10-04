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
        if !matches!(operation, "create" | "resume" | "persona") {
            return Err(WebError::Rejected("VALIDATION_ERROR"));
        }
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
        if operation == "create" {
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
        if kind != "turn" {
            return Err(WebError::Rejected("VALIDATION_ERROR"));
        }
        if input["expected_version"]
            .as_i64()
            .filter(|v| *v >= 0)
            .is_none()
        {
            return Err(WebError::Rejected("VALIDATION_ERROR"));
        }
        let content = input["content"].as_str().unwrap_or("");
        if content.trim().is_empty() || content.chars().count() > 4000 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::web::{TurnRepository, WebRead};
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct StubData(AtomicUsize);
    #[async_trait::async_trait]
    impl WebDataRepository for StubData {
        async fn read(&self, _: Option<Uuid>, _: WebRead) -> Result<Value, WebError> {
            unimplemented!()
        }
        async fn mutate_story(
            &self,
            _: Uuid,
            _: &str,
            _: StoryMutation,
            _: &str,
        ) -> Result<Value, WebError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(json!({}))
        }
        async fn accept_terms(&self, _: Uuid, _: &str) -> Result<Value, WebError> {
            unimplemented!()
        }
    }

    #[derive(Default)]
    struct StubTurns(AtomicUsize);
    #[async_trait::async_trait]
    impl TurnRepository for StubTurns {
        async fn admit(
            &self,
            _: Uuid,
            _: Uuid,
            _: &str,
            _: &str,
            _: &str,
            _: Value,
        ) -> Result<Value, WebError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(json!({}))
        }
        async fn pending(&self) -> Result<Vec<Uuid>, WebError> {
            unimplemented!()
        }
        async fn claim(&self, _: Uuid) -> Result<Option<Value>, WebError> {
            unimplemented!()
        }
        async fn save_input(&self, _: Uuid, _: Uuid, _: Value) -> Result<bool, WebError> {
            unimplemented!()
        }
        async fn settle(&self, _: Uuid, _: Uuid, _: Value) -> Result<Value, WebError> {
            unimplemented!()
        }
        async fn fail(&self, _: Uuid, _: Uuid) -> Result<(), WebError> {
            unimplemented!()
        }
    }

    fn stories() -> (WebStories, Arc<StubData>, Arc<StubTurns>) {
        let data = Arc::new(StubData::default());
        let turns = Arc::new(StubTurns::default());
        (
            WebStories {
                data: data.clone(),
                turns: turns.clone(),
            },
            data,
            turns,
        )
    }

    #[tokio::test]
    async fn regeneration_is_rejected_without_admitting() {
        let (stories, _, turns) = stories();
        let result = stories
            .turn(
                Uuid::new_v4(),
                Uuid::new_v4(),
                "regeneration",
                "valid-key-123456",
                json!({"expected_version":0,"message_id":Uuid::new_v4()}),
            )
            .await;
        assert!(matches!(
            result,
            Err(WebError::Rejected("VALIDATION_ERROR"))
        ));
        assert_eq!(turns.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn turn_is_admitted() {
        let (stories, _, turns) = stories();
        stories
            .turn(
                Uuid::new_v4(),
                Uuid::new_v4(),
                "turn",
                "valid-key-123456",
                json!({"expected_version":0,"content":"hi"}),
            )
            .await
            .unwrap();
        assert_eq!(turns.0.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn transfer_is_rejected_without_mutating() {
        let (stories, data, _) = stories();
        let result = stories
            .mutate(
                Uuid::new_v4(),
                "transfer",
                StoryMutation {
                    scene_id: None,
                    session_id: Some(Uuid::new_v4()),
                    persona: None,
                    expected_version: Some(0),
                },
                "valid-key-123456",
            )
            .await;
        assert!(matches!(
            result,
            Err(WebError::Rejected("VALIDATION_ERROR"))
        ));
        assert_eq!(data.0.load(Ordering::SeqCst), 0);
    }
}

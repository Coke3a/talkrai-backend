use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharacterGender {
    Male,
    Female,
}

impl CharacterGender {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Male => "male",
            Self::Female => "female",
        }
    }
}

impl std::str::FromStr for CharacterGender {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "male" => Ok(Self::Male),
            "female" => Ok(Self::Female),
            _ => Err(DomainError::InvalidField {
                field: "character_gender",
                reason: "invalid character gender value",
            }),
        }
    }
}

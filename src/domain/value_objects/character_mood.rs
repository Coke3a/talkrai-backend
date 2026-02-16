use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharacterMood {
    Neutral,
    Happy,
    Sad,
    Excited,
    Angry,
    Shy,
    Playful,
    Serious,
    Worried,
}

impl CharacterMood {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Happy => "happy",
            Self::Sad => "sad",
            Self::Excited => "excited",
            Self::Angry => "angry",
            Self::Shy => "shy",
            Self::Playful => "playful",
            Self::Serious => "serious",
            Self::Worried => "worried",
        }
    }
}

impl std::str::FromStr for CharacterMood {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "neutral" => Ok(Self::Neutral),
            "happy" => Ok(Self::Happy),
            "sad" => Ok(Self::Sad),
            "excited" => Ok(Self::Excited),
            "angry" => Ok(Self::Angry),
            "shy" => Ok(Self::Shy),
            "playful" => Ok(Self::Playful),
            "serious" => Ok(Self::Serious),
            "worried" => Ok(Self::Worried),
            _ => Err(DomainError::InvalidField {
                field: "character_mood",
                reason: "invalid character mood value",
            }),
        }
    }
}

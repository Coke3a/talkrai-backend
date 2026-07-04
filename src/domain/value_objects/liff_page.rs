use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiffPage {
    Scenes,
    Credits,
    Profile,
    Status,
    HowTo,
}

impl LiffPage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Scenes => "scenes",
            Self::Credits => "credits",
            Self::Profile => "profile",
            Self::Status => "status",
            Self::HowTo => "how_to",
        }
    }
}

impl std::str::FromStr for LiffPage {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "scenes" => Ok(Self::Scenes),
            "credits" => Ok(Self::Credits),
            "profile" => Ok(Self::Profile),
            "status" => Ok(Self::Status),
            "how_to" => Ok(Self::HowTo),
            _ => Err(DomainError::InvalidField {
                field: "page",
                reason: "unknown LIFF page",
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_variant() {
        let variants = [
            LiffPage::Scenes,
            LiffPage::Credits,
            LiffPage::Profile,
            LiffPage::Status,
            LiffPage::HowTo,
        ];
        for variant in variants {
            let s = variant.as_str();
            assert_eq!(s.parse::<LiffPage>().unwrap(), variant);
        }
    }

    #[test]
    fn how_to_uses_underscore() {
        assert_eq!(LiffPage::HowTo.as_str(), "how_to");
    }

    #[test]
    fn unknown_string_is_err() {
        assert!("how-to".parse::<LiffPage>().is_err());
        assert!("foo".parse::<LiffPage>().is_err());
    }
}

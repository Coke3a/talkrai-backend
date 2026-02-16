use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationshipLevel {
    Stranger,
    Acquaintance,
    Friend,
    CloseFriend,
}

impl RelationshipLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stranger => "stranger",
            Self::Acquaintance => "acquaintance",
            Self::Friend => "friend",
            Self::CloseFriend => "close_friend",
        }
    }

    /// Number of messages required to advance to the next level
    pub fn messages_threshold(&self) -> Option<u32> {
        match self {
            Self::Stranger => Some(20),
            Self::Acquaintance => Some(50),
            Self::Friend => Some(100),
            Self::CloseFriend => None,
        }
    }

    /// System prompt modifier based on relationship level
    pub fn relationship_prompt_modifier(&self) -> &'static str {
        match self {
            Self::Stranger => "พูดสุภาพ ถามทั่วไป ยังไม่เปิดเผยเรื่องส่วนตัว",
            Self::Acquaintance => "เริ่มแซว ตั้งชื่อเล่น คุยสบายขึ้น",
            Self::Friend => "เล่าเรื่องลึก ขอคำแนะนำ ไว้ใจ",
            Self::CloseFriend => "มุขส่วนตัว จำทุกเรื่อง พูดตรงๆ ไม่เกรงใจ",
        }
    }

    pub fn next(&self) -> Option<Self> {
        match self {
            Self::Stranger => Some(Self::Acquaintance),
            Self::Acquaintance => Some(Self::Friend),
            Self::Friend => Some(Self::CloseFriend),
            Self::CloseFriend => None,
        }
    }
}

impl std::str::FromStr for RelationshipLevel {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "stranger" => Ok(Self::Stranger),
            "acquaintance" => Ok(Self::Acquaintance),
            "friend" => Ok(Self::Friend),
            "close_friend" => Ok(Self::CloseFriend),
            _ => Err(DomainError::InvalidField {
                field: "relationship_level",
                reason: "invalid relationship level value",
            }),
        }
    }
}

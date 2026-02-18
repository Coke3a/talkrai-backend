use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipThresholds {
    acquaintance_at: u32,
    friend_at: u32,
    close_friend_at: u32,
}

impl RelationshipThresholds {
    pub fn new(
        acquaintance_at: u32,
        friend_at: u32,
        close_friend_at: u32,
    ) -> Result<Self, DomainError> {
        if acquaintance_at == 0 || friend_at == 0 || close_friend_at == 0 {
            return Err(DomainError::InvalidField {
                field: "relationship_thresholds",
                reason: "all thresholds must be greater than 0",
            });
        }

        if !(acquaintance_at < friend_at && friend_at < close_friend_at) {
            return Err(DomainError::InvalidField {
                field: "relationship_thresholds",
                reason:
                    "thresholds must be strictly increasing: acquaintance < friend < close_friend",
            });
        }

        Ok(Self {
            acquaintance_at,
            friend_at,
            close_friend_at,
        })
    }

    pub fn acquaintance_at(&self) -> u32 {
        self.acquaintance_at
    }

    pub fn friend_at(&self) -> u32 {
        self.friend_at
    }

    pub fn close_friend_at(&self) -> u32 {
        self.close_friend_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_thresholds() {
        let t = RelationshipThresholds::new(20, 50, 100);
        assert!(t.is_ok());
        let t = t.unwrap();
        assert_eq!(t.acquaintance_at(), 20);
        assert_eq!(t.friend_at(), 50);
        assert_eq!(t.close_friend_at(), 100);
    }

    #[test]
    fn rejects_zero_acquaintance() {
        assert!(RelationshipThresholds::new(0, 50, 100).is_err());
    }

    #[test]
    fn rejects_zero_friend() {
        assert!(RelationshipThresholds::new(20, 0, 100).is_err());
    }

    #[test]
    fn rejects_zero_close_friend() {
        assert!(RelationshipThresholds::new(20, 50, 0).is_err());
    }

    #[test]
    fn rejects_non_increasing_acquaintance_equals_friend() {
        assert!(RelationshipThresholds::new(50, 50, 100).is_err());
    }

    #[test]
    fn rejects_non_increasing_friend_equals_close_friend() {
        assert!(RelationshipThresholds::new(20, 100, 100).is_err());
    }

    #[test]
    fn rejects_decreasing_order() {
        assert!(RelationshipThresholds::new(100, 50, 20).is_err());
    }

    #[test]
    fn minimal_valid_thresholds() {
        let t = RelationshipThresholds::new(1, 2, 3);
        assert!(t.is_ok());
    }
}

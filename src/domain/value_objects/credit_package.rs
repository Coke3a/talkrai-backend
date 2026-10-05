/// Credits one roleplay turn costs.
///
/// Must match the SQL that charges turns: `reserved+2` in `admit_turn`
/// (migration 024) and `'cost_per_turn',2` in `web_scene()` (migration 023)
/// and in the web credits read (`web_data_postgres.rs`).
pub const TURN_COST: i32 = 2;

/// Show the "running low" nudge when this many turns or fewer remain (but more than zero).
pub const LOW_CREDIT_TURNS: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreditPackage {
    pub id: &'static str,
    pub credits: i32,
    pub price_thb: i32,
}

/// Must match the `CASE p_package` in `web_payment_order()` (migration 023)
/// and the web credits read (`web_data_postgres.rs`).
pub const CREDIT_PACKAGES: [CreditPackage; 3] = [
    CreditPackage {
        id: "basic",
        credits: 50,
        price_thb: 29,
    },
    CreditPackage {
        id: "plus",
        credits: 150,
        price_thb: 69,
    },
    CreditPackage {
        id: "premium",
        credits: 400,
        price_thb: 149,
    },
];

impl CreditPackage {
    pub fn find(id: &str) -> Option<&'static CreditPackage> {
        CREDIT_PACKAGES.iter().find(|p| p.id == id)
    }

    /// The cheapest package, offered first when a user runs out.
    pub fn starter() -> &'static CreditPackage {
        &CREDIT_PACKAGES[0]
    }

    pub fn turns(&self) -> i32 {
        self.credits / TURN_COST
    }
}

/// Whole turns the available credits still cover.
pub fn turns_left(available_credits: i32) -> i32 {
    available_credits.max(0) / TURN_COST
}

/// `Some(turns)` when the user should be nudged to top up before running out.
pub fn low_credit_turns(available_credits: i32) -> Option<i32> {
    let turns = turns_left(available_credits);
    (1..=LOW_CREDIT_TURNS).contains(&turns).then_some(turns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_is_basic_and_covers_25_turns() {
        let starter = CreditPackage::starter();
        assert_eq!(starter.id, "basic");
        assert_eq!(starter.price_thb, 29);
        assert_eq!(starter.turns(), 25);
    }

    #[test]
    fn find_known_and_unknown_packages() {
        assert_eq!(CreditPackage::find("plus").map(|p| p.credits), Some(150));
        assert!(CreditPackage::find("gold").is_none());
    }

    #[test]
    fn turns_left_floors_and_never_goes_negative() {
        assert_eq!(turns_left(5), 2);
        assert_eq!(turns_left(1), 0);
        assert_eq!(turns_left(-4), 0);
    }

    #[test]
    fn low_credit_nudge_only_for_one_or_two_turns() {
        assert_eq!(low_credit_turns(0), None);
        assert_eq!(low_credit_turns(1), None);
        assert_eq!(low_credit_turns(2), Some(1));
        assert_eq!(low_credit_turns(3), Some(1));
        assert_eq!(low_credit_turns(4), Some(2));
        assert_eq!(low_credit_turns(5), Some(2));
        assert_eq!(low_credit_turns(6), None);
        assert_eq!(low_credit_turns(-2), None);
    }
}

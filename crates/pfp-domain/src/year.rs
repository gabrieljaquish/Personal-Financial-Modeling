//! `Year`: a calendar year (`DOMAIN-MODEL.md` §2.1).

/// A calendar year.
///
/// `i32`, not `u16`: the ledger subtracts years (`year - base_year`,
/// `year - death_year`) and an unsigned difference underflows silently. The
/// `1900..=2200` domain is a validator check ([`year_in_domain`]), not a type
/// bound (`DOMAIN-MODEL.md` §2.1).
pub type Year = i32;

/// First year of the validator's domain (`DOMAIN-MODEL.md` §2.1).
pub const YEAR_MIN: Year = 1900;

/// Last year of the validator's domain (`DOMAIN-MODEL.md` §2.1).
pub const YEAR_MAX: Year = 2200;

/// Whether `year` lies inside the validator's domain, [`YEAR_MIN`]`..=`[`YEAR_MAX`].
#[must_use]
pub const fn year_in_domain(year: Year) -> bool {
    year >= YEAR_MIN && year <= YEAR_MAX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_bounds_are_inclusive() {
        assert!(year_in_domain(1900));
        assert!(year_in_domain(2026));
        assert!(year_in_domain(2200));
        assert!(!year_in_domain(1899));
        assert!(!year_in_domain(2201));
        assert!(!year_in_domain(Year::MIN));
        assert!(!year_in_domain(Year::MAX));
    }

    #[test]
    fn year_differences_are_signed() {
        let base: Year = 2024;
        let earlier: Year = 2017;
        assert_eq!(earlier - base, -7);
    }
}

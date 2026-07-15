use pinocchio_idl_macros::p_constant;

/// Minimum amount (in token base units, before decimal scaling) that a fundraiser must target.
#[p_constant]
pub const MIN_AMOUNT_TO_RAISE: u64 = 3;

/// Number of seconds in one day, used to convert timestamps to a day-based duration.
#[p_constant]
pub const SECONDS_TO_DAYS: i64 = 86_400;

/// Maximum single-contribution size as a percentage of the fundraiser's target amount.
#[p_constant]
pub const MAX_CONTRIBUTION_PERCENTAGE: u64 = 10;

/// Divisor used when computing percentage-based limits.
#[p_constant]
pub const PERCENTAGE_SCALER: u64 = 100;

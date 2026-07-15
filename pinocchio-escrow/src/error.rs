use pinocchio_idl_macros::p_error;

#[p_error]
pub enum EscrowError {
    /// Target amount is not satisfied.
    TargetNotMet,
    /// Invalid vault owner.
    InvalidVaultOwner,
    /// Invalid token program.
    InvalidTokenProgram,
}

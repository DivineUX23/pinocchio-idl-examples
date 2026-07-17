use pinocchio_idl_macros::p_error;

#[p_error]
pub enum CounterError {
    /// Unauthorized to perform this action.
    Unauthorized,
    /// Counter overflow.
    Overflow,
    /// Counter underflow.
    Underflow,
}

use pinocchio::{AccountView, error::ProgramError};
use pinocchio_idl_macros::p_state;

/// On-chain state account tracking the cumulative contribution of a single contributor.
///
/// Derived via PDA: `["contributor", contributor, bump]`.
#[p_state(inject)]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contributor {
    /// Total amount contributed by this contributor in token base units (little-endian u64).
    amount: [u8; 8],
}

impl Contributor {
    /// Total on-chain allocation for this account (no discriminator — raw 8 bytes).
    pub const LEN: usize = 8;

    /// Borrows a mutable reference to a `Contributor` from raw account data.
    ///
    /// Returns `ProgramError::InvalidAccountData` if the account data length
    /// does not exactly match `Contributor::LEN`.
    pub fn from_account_info(account: &mut AccountView) -> Result<&mut Self, ProgramError> {
        let data = unsafe { account.borrow_unchecked_mut() };
        if data.len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self) })
    }

    pub fn amount(&self) -> u64 {
        u64::from_le_bytes(self.amount)
    }

    pub fn set_amount(&mut self, amount: u64) {
        self.amount = amount.to_le_bytes();
    }
}

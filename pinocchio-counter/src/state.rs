use pinocchio::{AccountView, Address, error::ProgramError};
use pinocchio_idl_macros::p_state;

/// On-chain state tracking a user's counter.
///
/// Derived via PDA: `["counter", authority, bump]`.
#[p_state(inject)]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counter {
    /// Authority allowed to increment/decrement the counter.
    pub authority: [u8; 32],
    /// The current counter value.
    pub count: [u8; 8],
    /// PDA bump seed.
    pub bump: u8,
}

impl Counter {
    /// The exact space required for on-chain state storage.
    pub const LEN: usize = Self::SPACE;

    /// Casts raw account view data to a mutable reference of `Counter`.
    pub fn from_account_info(account: &mut AccountView) -> Result<&mut Self, ProgramError> {
        let data = unsafe { account.borrow_unchecked_mut() };
        if data.len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self) })
    }

    pub fn authority(&self) -> &Address {
        unsafe { &*(self.authority.as_ptr() as *const Address) }
    }

    pub fn set_authority(&mut self, authority: &Address) {
        self.authority.copy_from_slice(authority.as_ref());
    }

    pub fn count(&self) -> u64 {
        u64::from_le_bytes(self.count)
    }

    pub fn set_count(&mut self, count: u64) {
        self.count = count.to_le_bytes();
    }
}

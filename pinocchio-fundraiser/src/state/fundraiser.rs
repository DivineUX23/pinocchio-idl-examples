use pinocchio::{AccountView, Address, error::ProgramError};
use pinocchio_idl_macros::p_state;

/// On-chain state account for a fundraiser campaign.
///
/// Derived via PDA: `["fundraiser", maker, bump]`.
/// All multi-byte fields are stored in little-endian byte arrays to avoid
/// alignment assumptions under `#[repr(C)]`.
#[p_state]
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fundraiser {
    /// Public key of the fundraiser creator.
    maker: [u8; 32],
    /// Mint account of the token being raised.
    mint_to_raise: [u8; 32],
    /// Target amount in token base units (little-endian u64).
    amount_to_raise: [u8; 8],
    /// Amount collected so far in token base units (little-endian u64).
    current_amount: [u8; 8],
    /// Unix timestamp at which the campaign started (little-endian i64).
    time_started: [u8; 8],
    /// Campaign duration in days.
    pub duration: u8,
    /// Canonical bump seed for the fundraiser PDA.
    pub bump: u8,
}

impl Fundraiser {
    /// Total on-chain allocation for this account, including the 8-byte discriminator.
    pub const LEN: usize = Self::SPACE;

    /// Borrows a mutable reference to a `Fundraiser` from raw account data.
    ///
    /// Returns `ProgramError::InvalidAccountData` if the account data length
    /// does not exactly match `Fundraiser::LEN`.
    pub fn from_account_info(account: &mut AccountView) -> Result<&mut Self, ProgramError> {
        let data = unsafe { account.borrow_unchecked_mut() };
        if data.len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self) })
    }

    pub fn maker(&self) -> &Address {
        unsafe { &*(self.maker.as_ptr() as *const Address) }
    }

    pub fn set_maker(&mut self, maker: &Address) {
        self.maker.copy_from_slice(maker.as_ref());
    }

    pub fn mint_to_raise(&self) -> &Address {
        unsafe { &*(self.mint_to_raise.as_ptr() as *const Address) }
    }

    pub fn set_mint_to_raise(&mut self, mint: &Address) {
        self.mint_to_raise.copy_from_slice(mint.as_ref());
    }

    pub fn amount_to_raise(&self) -> u64 {
        u64::from_le_bytes(self.amount_to_raise)
    }

    pub fn set_amount_to_raise(&mut self, amount: u64) {
        self.amount_to_raise = amount.to_le_bytes();
    }

    pub fn current_amount(&self) -> u64 {
        u64::from_le_bytes(self.current_amount)
    }

    pub fn set_current_amount(&mut self, amount: u64) {
        self.current_amount = amount.to_le_bytes();
    }

    pub fn time_started(&self) -> i64 {
        i64::from_le_bytes(self.time_started)
    }

    pub fn set_time_started(&mut self, time: i64) {
        self.time_started = time.to_le_bytes();
    }
}

use pinocchio::{AccountView, Address, error::ProgramError};
use pinocchio_idl_macros::p_state;

/// On-chain state representation of an escrow campaign.
#[p_state(inject)]
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Escrow {
    /// Unique identifier / seed for this escrow.
    pub seed: u64,
    /// The creator of the escrow.
    pub maker: [u8; 32],
    /// The mint of the token deposited by the maker (held in the vault).
    pub mint_a: [u8; 32],
    /// The mint of the token expected in return from the taker.
    pub mint_b: [u8; 32],
    /// Amount of mint_a deposited.
    pub amount_a: u64,
    /// Amount of mint_b expected in return.
    pub amount_b: u64,
    /// PDA bump seed.
    pub bump: u8,
}

impl Escrow {
    /// The exact space required for on-chain state storage.
    pub const LEN: usize = Self::SPACE;

    /// Casts raw account view data to a mutable reference of `Escrow`.
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

    pub fn mint_a(&self) -> &Address {
        unsafe { &*(self.mint_a.as_ptr() as *const Address) }
    }

    pub fn set_mint_a(&mut self, mint: &Address) {
        self.mint_a.copy_from_slice(mint.as_ref());
    }

    pub fn mint_b(&self) -> &Address {
        unsafe { &*(self.mint_b.as_ptr() as *const Address) }
    }

    pub fn set_mint_b(&mut self, mint: &Address) {
        self.mint_b.copy_from_slice(mint.as_ref());
    }
}

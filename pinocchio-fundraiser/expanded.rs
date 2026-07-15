#![feature(prelude_import)]
#![allow(unexpected_cfgs)]
extern crate std;
#[prelude_import]
use std::prelude::rust_2024::*;
use pinocchio::{
    AccountView, Address, ProgramResult, address::declare_id, entrypoint,
    error::ProgramError,
};
mod constants {
    use pinocchio_idl_macros::p_constant;
    /// Minimum amount (in token base units, before decimal scaling) that a fundraiser must target.
    pub const MIN_AMOUNT_TO_RAISE: u64 = 3;
    /// Number of seconds in one day, used to convert timestamps to a day-based duration.
    pub const SECONDS_TO_DAYS: i64 = 86_400;
    /// Maximum single-contribution size as a percentage of the fundraiser's target amount.
    pub const MAX_CONTRIBUTION_PERCENTAGE: u64 = 10;
    /// Divisor used when computing percentage-based limits.
    pub const PERCENTAGE_SCALER: u64 = 100;
}
mod instructions {
    pub mod initialize {
        use pinocchio::{
            AccountView, ProgramResult, cpi::{Seed, Signer},
            error::ProgramError, sysvars::{Sysvar, rent::Rent},
        };
        use pinocchio_system::instructions::CreateAccount;
        use pinocchio_idl_macros::p_instruction;
        use crate::{MIN_AMOUNT_TO_RAISE, state::Fundraiser};
        /// Creates and initialises a new fundraiser campaign.
        ///
        /// # Accounts
        ///
        /// | Index | Name                      | Description |
        /// |-------|---------------------------|-------------|
        /// | 0     | `maker`                   | Campaign creator. Must be a signer. |
        /// | 1     | `fundraiser`              | PDA account that stores campaign state. Created by this instruction. |
        /// | 2     | `mint_to_raise`           | Mint of the token being raised. |
        /// | 3     | `vault`                   | ATA owned by the fundraiser PDA that will hold collected tokens. |
        /// | 4     | `system_program`          | Auto-resolved. |
        /// | 5     | `token_program`           | Auto-resolved. |
        /// | 6     | `associated_token_program`| Auto-resolved. |
        ///
        /// # Instruction Data
        ///
        /// | Field            | Type | Offset |
        /// |------------------|------|--------|
        /// | `amount_to_raise`| u64  | 0..8   |
        /// | `time_started`   | i64  | 8..16  |
        /// | `duration`       | u8   | 16     |
        /// | `bump`           | u8   | 17     |
        pub fn process_initialize_instruction(
            accounts: &mut [AccountView],
            data: &[u8],
        ) -> ProgramResult {
            if accounts.len() < 7usize {
                return Err(ProgramError::NotEnoughAccountKeys);
            }
            let [maker, mint_to_raise, fundraiser, vault, system_program, token_program,
            _associated_token_program] = accounts else {
                return Err(ProgramError::NotEnoughAccountKeys);
            };
            let amount_to_raise = <u64>::from_le_bytes(
                data
                    .get(0..8)
                    .and_then(|s| s.try_into().ok())
                    .ok_or(ProgramError::InvalidArgument)?,
            );
            let time_started = <i64>::from_le_bytes(
                data
                    .get(8..16)
                    .and_then(|s| s.try_into().ok())
                    .ok_or(ProgramError::InvalidArgument)?,
            );
            let duration: u8 = *data.get(16).ok_or(ProgramError::InvalidArgument)?;
            let bump: u8 = *data.get(17).ok_or(ProgramError::InvalidArgument)?;
            if !maker.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            if !maker.is_signer() {
                return Err(ProgramError::MissingRequiredSignature);
            }
            if !fundraiser.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            {
                let __expected_pda = ::pinocchio::Address::from(
                    pinocchio_pubkey::derive_address(
                        &[b"fundraiser", maker.address().as_ref(), &bump.to_le_bytes()],
                        None,
                        &crate::ID.to_bytes(),
                    ),
                );
                if fundraiser.address() != &__expected_pda {
                    return Err(ProgramError::InvalidArgument);
                }
            }
            if !vault.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            let mint_data = mint_to_raise.try_borrow()?;
            if mint_data.len() < 45 {
                return Err(ProgramError::InvalidAccountData);
            }
            let decimals = mint_data[44];
            drop(mint_data);
            let scaled_min = MIN_AMOUNT_TO_RAISE
                .checked_mul(10u64.pow(decimals as u32))
                .ok_or(ProgramError::ArithmeticOverflow)?;
            if amount_to_raise < scaled_min {
                return Err(ProgramError::InvalidArgument);
            }
            if fundraiser.owned_by(&crate::ID) {
                return Err(ProgramError::AccountAlreadyInitialized);
            }
            let bump_bytes = [bump];
            let signer_seeds = [
                Seed::from(b"fundraiser"),
                Seed::from(maker.address().as_array()),
                Seed::from(bump_bytes.as_ref()),
            ];
            let signer = Signer::from(&signer_seeds);
            CreateAccount {
                from: maker,
                to: fundraiser,
                lamports: Rent::get()?.try_minimum_balance(Fundraiser::LEN)?,
                space: Fundraiser::LEN as u64,
                owner: &crate::ID,
            }
                .invoke_signed(&[signer])?;
            let fundraiser_data = Fundraiser::from_account_info(fundraiser)?;
            fundraiser_data.set_maker(maker.address());
            fundraiser_data.set_mint_to_raise(mint_to_raise.address());
            fundraiser_data.set_amount_to_raise(amount_to_raise);
            fundraiser_data.set_current_amount(0);
            fundraiser_data.set_time_started(time_started);
            fundraiser_data.duration = duration;
            fundraiser_data.bump = bump;
            pinocchio_associated_token_account::instructions::Create {
                funding_account: maker,
                account: vault,
                wallet: fundraiser,
                mint: mint_to_raise,
                token_program,
                system_program,
            }
                .invoke()?;
            Ok(())
        }
    }
    pub mod contribute {
        use pinocchio::{
            AccountView, ProgramResult, cpi::{Seed, Signer},
            error::ProgramError, sysvars::{Sysvar, clock::Clock, rent::Rent},
        };
        use pinocchio_system::instructions::CreateAccount;
        use pinocchio_idl_macros::p_instruction;
        use crate::{
            MAX_CONTRIBUTION_PERCENTAGE, PERCENTAGE_SCALER, SECONDS_TO_DAYS,
            state::{Contributor, Fundraiser},
        };
        /// Transfers tokens from a contributor's ATA into the fundraiser vault.
        ///
        /// If this is the contributor's first contribution, a contributor PDA account
        /// is created to track their total. Contributions are capped at
        /// `MAX_CONTRIBUTION_PERCENTAGE` percent of the campaign target.
        ///
        /// # Accounts
        ///
        /// | Index | Name                      | Description |
        /// |-------|---------------------------|-------------|
        /// | 0     | `contributor`             | Token sender. Must be a signer. |
        /// | 1     | `mint_to_raise`           | Mint of the token being raised. |
        /// | 2     | `fundraiser`              | Fundraiser campaign state PDA. |
        /// | 3     | `contributor_account`     | Per-contributor tracking PDA. Created on first contribution. |
        /// | 4     | `contributor_ata`         | Contributor's ATA for `mint_to_raise`. |
        /// | 5     | `vault`                   | Fundraiser vault ATA. |
        /// | 6     | `system_program`          | Auto-resolved. |
        /// | 7     | `token_program`           | Auto-resolved. |
        /// | 8     | `associated_token_program`| Auto-resolved. |
        ///
        /// # Instruction Data
        ///
        /// | Field  | Type | Offset |
        /// |--------|------|--------|
        /// | amount | u64  | 0..8   |
        /// | bump   | u8   | 8      |
        pub fn process_contribute_instruction(
            accounts: &mut [AccountView],
            data: &[u8],
        ) -> ProgramResult {
            if accounts.len() < 9usize {
                return Err(ProgramError::NotEnoughAccountKeys);
            }
            let [contributor, mint_to_raise, fundraiser, contributor_account,
            contributor_ata, vault, _system_program, _token_program,
            _associated_token_program] = accounts else {
                return Err(ProgramError::NotEnoughAccountKeys);
            };
            let amount = <u64>::from_le_bytes(
                data
                    .get(0..8)
                    .and_then(|s| s.try_into().ok())
                    .ok_or(ProgramError::InvalidArgument)?,
            );
            let bump: u8 = *data.get(8).ok_or(ProgramError::InvalidArgument)?;
            if !contributor.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            if !contributor.is_signer() {
                return Err(ProgramError::MissingRequiredSignature);
            }
            if !fundraiser.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            if !contributor_account.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            {
                let __expected_pda = ::pinocchio::Address::from(
                    pinocchio_pubkey::derive_address(
                        &[
                            b"contributor",
                            contributor.address().as_ref(),
                            &bump.to_le_bytes(),
                        ],
                        None,
                        &crate::ID.to_bytes(),
                    ),
                );
                if contributor_account.address() != &__expected_pda {
                    return Err(ProgramError::InvalidArgument);
                }
            }
            if !contributor_ata.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            {
                let __ata_state = ::pinocchio_token::state::Account::from_account_view(
                    contributor_ata,
                )?;
                if __ata_state.owner() != contributor.address() {
                    return Err(ProgramError::IllegalOwner);
                }
                if __ata_state.mint() != mint_to_raise.address() {
                    return Err(ProgramError::InvalidAccountData);
                }
            }
            if !vault.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            {
                let __ata_state = ::pinocchio_token::state::Account::from_account_view(
                    vault,
                )?;
                if __ata_state.owner() != fundraiser.address() {
                    return Err(ProgramError::IllegalOwner);
                }
                if __ata_state.mint() != mint_to_raise.address() {
                    return Err(ProgramError::InvalidAccountData);
                }
            }
            let fundraiser_data = Fundraiser::from_account_info(fundraiser)?;
            let mint_data = mint_to_raise.try_borrow()?;
            if mint_data.len() < 45 {
                return Err(ProgramError::InvalidAccountData);
            }
            let decimals = mint_data[44];
            drop(mint_data);
            if amount <= 10u64.pow(decimals as u32) {
                return Err(ProgramError::InvalidArgument);
            }
            let cap = (fundraiser_data.amount_to_raise() * MAX_CONTRIBUTION_PERCENTAGE)
                / PERCENTAGE_SCALER;
            if amount > cap {
                return Err(ProgramError::InvalidArgument);
            }
            let current_time = Clock::get()?.unix_timestamp;
            let elapsed_days = ((current_time - fundraiser_data.time_started())
                / SECONDS_TO_DAYS) as u8;
            if elapsed_days >= fundraiser_data.duration {
                return Err(ProgramError::InvalidArgument);
            }
            if !contributor_account.owned_by(&crate::ID) {
                let bump_bytes = [bump];
                let signer_seeds = [
                    Seed::from(b"contributor"),
                    Seed::from(contributor.address().as_array()),
                    Seed::from(bump_bytes.as_ref()),
                ];
                let signer = Signer::from(&signer_seeds);
                CreateAccount {
                    from: contributor,
                    to: contributor_account,
                    lamports: Rent::get()?.try_minimum_balance(Contributor::LEN)?,
                    space: Contributor::LEN as u64,
                    owner: &crate::ID,
                }
                    .invoke_signed(&[signer])?;
            }
            let contributor_data = Contributor::from_account_info(contributor_account)?;
            let new_total = contributor_data
                .amount()
                .checked_add(amount)
                .ok_or(ProgramError::ArithmeticOverflow)?;
            if new_total > cap {
                return Err(ProgramError::InvalidArgument);
            }
            pinocchio_token::instructions::Transfer::new(
                    contributor_ata,
                    vault,
                    contributor,
                    amount,
                )
                .invoke()?;
            contributor_data.set_amount(new_total);
            let new_current = fundraiser_data
                .current_amount()
                .checked_add(amount)
                .ok_or(ProgramError::ArithmeticOverflow)?;
            fundraiser_data.set_current_amount(new_current);
            Ok(())
        }
    }
    pub mod checker {
        use pinocchio::{
            AccountView, ProgramResult, cpi::{Seed, Signer},
            error::ProgramError,
        };
        use pinocchio_idl_macros::p_instruction;
        use crate::state::Fundraiser;
        /// Checks whether the fundraiser target has been met and, if so, transfers all
        /// collected tokens to the maker's ATA and closes the fundraiser account.
        ///
        /// This instruction may only be called by the campaign `maker`. It will fail
        /// if the vault balance has not yet reached `amount_to_raise`.
        ///
        /// # Accounts
        ///
        /// | Index | Name           | Description |
        /// |-------|----------------|-------------|
        /// | 0     | `maker`        | Campaign creator and fund recipient. Must be a signer. |
        /// | 1     | `mint_to_raise`| Mint of the token being raised. |
        /// | 2     | `fundraiser`   | Fundraiser campaign state PDA. Closed after a successful check. |
        /// | 3     | `vault`        | Fundraiser vault ATA. Drained by this instruction. |
        /// | 4     | `maker_ata`    | Maker's ATA that receives the collected tokens. |
        /// | 5     | `system_program`| Auto-resolved. |
        /// | 6     | `token_program` | Auto-resolved. |
        ///
        /// # Instruction Data
        ///
        /// | Field | Type | Offset |
        /// |-------|------|--------|
        /// | bump  | u8   | 0      |
        pub fn process_checker_instruction(
            accounts: &mut [AccountView],
            data: &[u8],
        ) -> ProgramResult {
            if accounts.len() < 7usize {
                return Err(ProgramError::NotEnoughAccountKeys);
            }
            let [maker, mint_to_raise, fundraiser, vault, maker_ata, _system_program,
            _token_program] = accounts else {
                return Err(ProgramError::NotEnoughAccountKeys);
            };
            let bump: u8 = *data.get(0).ok_or(ProgramError::InvalidArgument)?;
            if !maker.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            if !maker.is_signer() {
                return Err(ProgramError::MissingRequiredSignature);
            }
            if !fundraiser.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            {
                let __expected_pda = ::pinocchio::Address::from(
                    pinocchio_pubkey::derive_address(
                        &[b"fundraiser", maker.address().as_ref(), &bump.to_le_bytes()],
                        None,
                        &crate::ID.to_bytes(),
                    ),
                );
                if fundraiser.address() != &__expected_pda {
                    return Err(ProgramError::InvalidArgument);
                }
            }
            if !vault.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            {
                let __ata_state = ::pinocchio_token::state::Account::from_account_view(
                    vault,
                )?;
                if __ata_state.owner() != fundraiser.address() {
                    return Err(ProgramError::IllegalOwner);
                }
                if __ata_state.mint() != mint_to_raise.address() {
                    return Err(ProgramError::InvalidAccountData);
                }
            }
            if !maker_ata.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            {
                let __ata_state = ::pinocchio_token::state::Account::from_account_view(
                    maker_ata,
                )?;
                if __ata_state.owner() != maker.address() {
                    return Err(ProgramError::IllegalOwner);
                }
                if __ata_state.mint() != mint_to_raise.address() {
                    return Err(ProgramError::InvalidAccountData);
                }
            }
            let fundraiser_data = Fundraiser::from_account_info(fundraiser)?;
            let vault_data = unsafe { vault.borrow_unchecked() };
            let vault_amount = u64::from_le_bytes(
                vault_data[64..72]
                    .try_into()
                    .map_err(|_| ProgramError::InvalidAccountData)?,
            );
            if vault_amount < fundraiser_data.amount_to_raise() {
                return Err(ProgramError::InvalidArgument);
            }
            let bump_bytes = [bump];
            let signer_seeds = [
                Seed::from(b"fundraiser"),
                Seed::from(maker.address().as_array()),
                Seed::from(bump_bytes.as_ref()),
            ];
            let signer = Signer::from(&signer_seeds);
            pinocchio_token::instructions::Transfer::new(
                    vault,
                    maker_ata,
                    fundraiser,
                    vault_amount,
                )
                .invoke_signed(&[signer])?;
            let fundraiser_lamports = fundraiser.lamports();
            maker.set_lamports(maker.lamports() + fundraiser_lamports);
            fundraiser.set_lamports(0);
            let _ = fundraiser.close();
            Ok(())
        }
    }
    pub mod refund {
        use pinocchio::{
            AccountView, ProgramResult, cpi::{Seed, Signer},
            error::ProgramError, sysvars::{Sysvar, clock::Clock},
        };
        use pinocchio_idl_macros::p_instruction;
        use crate::{SECONDS_TO_DAYS, state::{Contributor, Fundraiser}};
        /// Refunds a contributor's tokens after a campaign has expired without meeting its target.
        ///
        /// This instruction may only be invoked after the campaign duration has elapsed
        /// and only if the vault balance has not reached `amount_to_raise`. On success,
        /// the contributor's tokens are returned to their ATA and the contributor account
        /// is closed, returning its lamports to the contributor.
        ///
        /// # Accounts
        ///
        /// | Index | Name                | Description |
        /// |-------|---------------------|-------------|
        /// | 0     | `contributor`       | The contributor requesting a refund. Must be a signer. |
        /// | 1     | `maker`             | Campaign creator, used to validate the fundraiser PDA. |
        /// | 2     | `mint_to_raise`     | Mint of the token being raised. |
        /// | 3     | `fundraiser`        | Fundraiser campaign state PDA. |
        /// | 4     | `contributor_account` | Contributor tracking PDA. Closed after refund. |
        /// | 5     | `contributor_ata`   | Contributor's ATA that receives the refunded tokens. |
        /// | 6     | `vault`             | Fundraiser vault ATA. |
        /// | 7     | `system_program`    | Auto-resolved. |
        /// | 8     | `token_program`     | Auto-resolved. |
        ///
        /// # Instruction Data
        ///
        /// | Field              | Type | Offset |
        /// |--------------------|------|--------|
        /// | bump               | u8   | 0      |
        /// | contributor_bump   | u8   | 1      |
        pub fn process_refund_instruction(
            accounts: &mut [AccountView],
            data: &[u8],
        ) -> ProgramResult {
            if accounts.len() < 9usize {
                return Err(ProgramError::NotEnoughAccountKeys);
            }
            let [contributor, maker, mint_to_raise, fundraiser, contributor_account,
            contributor_ata, vault, _system_program, _token_program] = accounts else {
                return Err(ProgramError::NotEnoughAccountKeys);
            };
            let bump: u8 = *data.get(0).ok_or(ProgramError::InvalidArgument)?;
            let contributor_bump: u8 = *data
                .get(1)
                .ok_or(ProgramError::InvalidArgument)?;
            if !contributor.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            if !contributor.is_signer() {
                return Err(ProgramError::MissingRequiredSignature);
            }
            if !fundraiser.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            {
                let __expected_pda = ::pinocchio::Address::from(
                    pinocchio_pubkey::derive_address(
                        &[b"fundraiser", maker.address().as_ref(), &bump.to_le_bytes()],
                        None,
                        &crate::ID.to_bytes(),
                    ),
                );
                if fundraiser.address() != &__expected_pda {
                    return Err(ProgramError::InvalidArgument);
                }
            }
            if !contributor_account.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            {
                let __expected_pda = ::pinocchio::Address::from(
                    pinocchio_pubkey::derive_address(
                        &[
                            b"contributor",
                            contributor.address().as_ref(),
                            &contributor_bump.to_le_bytes(),
                        ],
                        None,
                        &crate::ID.to_bytes(),
                    ),
                );
                if contributor_account.address() != &__expected_pda {
                    return Err(ProgramError::InvalidArgument);
                }
            }
            if !contributor_ata.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            {
                let __ata_state = ::pinocchio_token::state::Account::from_account_view(
                    contributor_ata,
                )?;
                if __ata_state.owner() != contributor.address() {
                    return Err(ProgramError::IllegalOwner);
                }
                if __ata_state.mint() != mint_to_raise.address() {
                    return Err(ProgramError::InvalidAccountData);
                }
            }
            if !vault.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            {
                let __ata_state = ::pinocchio_token::state::Account::from_account_view(
                    vault,
                )?;
                if __ata_state.owner() != fundraiser.address() {
                    return Err(ProgramError::IllegalOwner);
                }
                if __ata_state.mint() != mint_to_raise.address() {
                    return Err(ProgramError::InvalidAccountData);
                }
            }
            let fundraiser_data = Fundraiser::from_account_info(fundraiser)?;
            if fundraiser_data.maker() != maker.address() {
                return Err(ProgramError::InvalidAccountData);
            }
            let contributor_data = Contributor::from_account_info(contributor_account)?;
            let current_time = Clock::get()?.unix_timestamp;
            let elapsed_days = ((current_time - fundraiser_data.time_started())
                / SECONDS_TO_DAYS) as u8;
            if elapsed_days < fundraiser_data.duration {
                return Err(ProgramError::InvalidArgument);
            }
            let vault_data = unsafe { vault.borrow_unchecked() };
            let vault_amount = u64::from_le_bytes(
                vault_data[64..72]
                    .try_into()
                    .map_err(|_| ProgramError::InvalidAccountData)?,
            );
            if vault_amount >= fundraiser_data.amount_to_raise() {
                return Err(ProgramError::InvalidArgument);
            }
            let refund_amount = contributor_data.amount();
            let new_current = fundraiser_data
                .current_amount()
                .saturating_sub(refund_amount);
            fundraiser_data.set_current_amount(new_current);
            let bump_bytes = [bump];
            let signer_seeds = [
                Seed::from(b"fundraiser"),
                Seed::from(maker.address().as_array()),
                Seed::from(bump_bytes.as_ref()),
            ];
            let signer = Signer::from(&signer_seeds);
            pinocchio_token::instructions::Transfer::new(
                    vault,
                    contributor_ata,
                    fundraiser,
                    refund_amount,
                )
                .invoke_signed(&[signer])?;
            let contributor_lamports = contributor_account.lamports();
            contributor.set_lamports(contributor.lamports() + contributor_lamports);
            contributor_account.set_lamports(0);
            let _ = contributor_account.close();
            Ok(())
        }
    }
    pub use initialize::*;
    pub use contribute::*;
    pub use checker::*;
    pub use refund::*;
    use pinocchio::error::ProgramError;
    /// Instruction discriminator enum.
    ///
    /// The first byte of `instruction_data` in `process_instruction` is used
    /// to select which handler is invoked.
    pub enum FundraiserInstructions {
        Initialize = 0,
        Contributor = 1,
        Checker = 2,
        Refund = 3,
    }
    impl TryFrom<&u8> for FundraiserInstructions {
        type Error = ProgramError;
        fn try_from(value: &u8) -> Result<Self, Self::Error> {
            match value {
                0 => Ok(FundraiserInstructions::Initialize),
                1 => Ok(FundraiserInstructions::Contributor),
                2 => Ok(FundraiserInstructions::Checker),
                3 => Ok(FundraiserInstructions::Refund),
                _ => Err(ProgramError::InvalidInstructionData),
            }
        }
    }
}
mod state {
    pub mod fundraiser {
        use pinocchio::{AccountView, Address, error::ProgramError};
        use pinocchio_idl_macros::p_state;
        #[repr(C)]
        /// On-chain state account for a fundraiser campaign.
        ///
        /// Derived via PDA: `["fundraiser", maker, bump]`.
        /// All multi-byte fields are stored in little-endian byte arrays to avoid
        /// alignment assumptions under `#[repr(C)]`.
        #[repr(C)]
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
        #[automatically_derived]
        #[doc(hidden)]
        unsafe impl ::core::clone::TrivialClone for Fundraiser {}
        #[automatically_derived]
        impl ::core::clone::Clone for Fundraiser {
            #[inline]
            fn clone(&self) -> Fundraiser {
                let _: ::core::clone::AssertParamIsClone<[u8; 32]>;
                let _: ::core::clone::AssertParamIsClone<[u8; 32]>;
                let _: ::core::clone::AssertParamIsClone<[u8; 8]>;
                let _: ::core::clone::AssertParamIsClone<[u8; 8]>;
                let _: ::core::clone::AssertParamIsClone<[u8; 8]>;
                let _: ::core::clone::AssertParamIsClone<u8>;
                *self
            }
        }
        #[automatically_derived]
        impl ::core::marker::Copy for Fundraiser {}
        #[automatically_derived]
        impl ::core::fmt::Debug for Fundraiser {
            #[inline]
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                let names: &'static _ = &[
                    "maker",
                    "mint_to_raise",
                    "amount_to_raise",
                    "current_amount",
                    "time_started",
                    "duration",
                    "bump",
                ];
                let values: &[&dyn ::core::fmt::Debug] = &[
                    &self.maker,
                    &self.mint_to_raise,
                    &self.amount_to_raise,
                    &self.current_amount,
                    &self.time_started,
                    &self.duration,
                    &&self.bump,
                ];
                ::core::fmt::Formatter::debug_struct_fields_finish(
                    f,
                    "Fundraiser",
                    names,
                    values,
                )
            }
        }
        #[automatically_derived]
        impl ::core::marker::StructuralPartialEq for Fundraiser {}
        #[automatically_derived]
        impl ::core::cmp::PartialEq for Fundraiser {
            #[inline]
            fn eq(&self, other: &Fundraiser) -> bool {
                self.duration == other.duration && self.bump == other.bump
                    && self.maker == other.maker
                    && self.mint_to_raise == other.mint_to_raise
                    && self.amount_to_raise == other.amount_to_raise
                    && self.current_amount == other.current_amount
                    && self.time_started == other.time_started
            }
        }
        impl Fundraiser {
            pub const SPACE: usize = std::mem::size_of::<Self>();
            pub const DISCRIMINATOR: [u8; 8] = [
                167u8, 106u8, 143u8, 202u8, 135u8, 131u8, 204u8, 196u8,
            ];
        }
        impl Fundraiser {
            /// Total on-chain allocation for this account, including the 8-byte discriminator.
            pub const LEN: usize = Self::SPACE;
            /// Borrows a mutable reference to a `Fundraiser` from raw account data.
            ///
            /// Returns `ProgramError::InvalidAccountData` if the account data length
            /// does not exactly match `Fundraiser::LEN`.
            pub fn from_account_info(
                account: &mut AccountView,
            ) -> Result<&mut Self, ProgramError> {
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
    }
    pub mod contributor {
        use pinocchio::{AccountView, error::ProgramError};
        use pinocchio_idl_macros::p_state;
        #[repr(C)]
        /// On-chain state account tracking the cumulative contribution of a single contributor.
        ///
        /// Derived via PDA: `["contributor", contributor, bump]`.
        #[repr(C)]
        pub struct Contributor {
            /// Total amount contributed by this contributor in token base units (little-endian u64).
            amount: [u8; 8],
        }
        #[automatically_derived]
        #[doc(hidden)]
        unsafe impl ::core::clone::TrivialClone for Contributor {}
        #[automatically_derived]
        impl ::core::clone::Clone for Contributor {
            #[inline]
            fn clone(&self) -> Contributor {
                let _: ::core::clone::AssertParamIsClone<[u8; 8]>;
                *self
            }
        }
        #[automatically_derived]
        impl ::core::marker::Copy for Contributor {}
        #[automatically_derived]
        impl ::core::fmt::Debug for Contributor {
            #[inline]
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::debug_struct_field1_finish(
                    f,
                    "Contributor",
                    "amount",
                    &&self.amount,
                )
            }
        }
        #[automatically_derived]
        impl ::core::default::Default for Contributor {
            #[inline]
            fn default() -> Contributor {
                Contributor {
                    amount: ::core::default::Default::default(),
                }
            }
        }
        #[automatically_derived]
        impl ::core::marker::StructuralPartialEq for Contributor {}
        #[automatically_derived]
        impl ::core::cmp::PartialEq for Contributor {
            #[inline]
            fn eq(&self, other: &Contributor) -> bool {
                self.amount == other.amount
            }
        }
        impl Contributor {
            pub const SPACE: usize = std::mem::size_of::<Self>();
            pub const DISCRIMINATOR: [u8; 8] = [
                222u8, 222u8, 255u8, 212u8, 133u8, 49u8, 27u8, 93u8,
            ];
        }
        impl Contributor {
            /// Total on-chain allocation for this account (no discriminator — raw 8 bytes).
            pub const LEN: usize = 8;
            /// Borrows a mutable reference to a `Contributor` from raw account data.
            ///
            /// Returns `ProgramError::InvalidAccountData` if the account data length
            /// does not exactly match `Contributor::LEN`.
            pub fn from_account_info(
                account: &mut AccountView,
            ) -> Result<&mut Self, ProgramError> {
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
    }
    pub use fundraiser::*;
    pub use contributor::*;
}
use constants::*;
use instructions::*;
/// Program entrypoint.
#[no_mangle]
pub unsafe extern "C" fn entrypoint(input: *mut u8) -> u64 {
    ::pinocchio::entrypoint::process_entrypoint::<
        { ::pinocchio::MAX_TX_ACCOUNTS },
    >(input, process_instruction)
}
/// A default allocator for when the program is compiled on a target different
/// than `"solana"`.
///
/// This links the `std` library, which will set up a default global allocator.
mod __private_alloc {
    extern crate std as __std;
}
/// The const program ID.
pub const ID: ::solana_address::Address = ::solana_address::Address::from_str_const(
    "96TFrsG998MvvrfuShRQmSemkzN555pnidGF4gquJsKr",
);
/// Returns `true` if given address is the ID.
pub fn check_id(id: &::solana_address::Address) -> bool {
    id == &ID
}
/// Returns the ID.
pub const fn id() -> ::solana_address::Address {
    { ID }
}
pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    match (&program_id, &&ID) {
        (left_val, right_val) => {
            if !(*left_val == *right_val) {
                let kind = ::core::panicking::AssertKind::Eq;
                ::core::panicking::assert_failed(
                    kind,
                    &*left_val,
                    &*right_val,
                    ::core::option::Option::None,
                );
            }
        }
    };
    let (discriminator, data) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidAccountData)?;
    match FundraiserInstructions::try_from(discriminator)? {
        FundraiserInstructions::Initialize => {
            process_initialize_instruction(accounts, data)?
        }
        FundraiserInstructions::Contributor => {
            process_contribute_instruction(accounts, data)?
        }
        FundraiserInstructions::Checker => process_checker_instruction(accounts, data)?,
        FundraiserInstructions::Refund => process_refund_instruction(accounts, data)?,
    }
    Ok(())
}

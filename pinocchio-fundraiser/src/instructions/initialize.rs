use pinocchio::{
    AccountView, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{Sysvar, rent::Rent},
};
use pinocchio_idl_macros::p_instruction;
use pinocchio_system::instructions::CreateAccount;

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
#[p_instruction(
    id = 0,
    inject,
    accounts = [
        maker(signer, mut),
        fundraiser(mut,
            pda = [b"fundraiser", maker, bump],
            state = Fundraiser
        ),
        mint_to_raise(relations = [fundraiser]),
        vault(mut, init = [fundraiser, mint_to_raise])
    ],
    data = [
        amount_to_raise: u64 = data[0..8],
        time_started:    i64 = data[8..16],
        duration:        u8  = data[16],
        bump:            u8  = data[17]
    ]
)]
pub fn process_initialize_instruction(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    // All account bindings must appear before any other statements.
    let [
        maker,
        mint_to_raise,
        fundraiser,
        vault,
        system_program,
        token_program,
        _associated_token_program,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate the mint account has a readable decimals byte at offset 44.
    let mint_data = mint_to_raise.try_borrow()?;
    if mint_data.len() < 45 {
        return Err(ProgramError::InvalidAccountData);
    }
    let decimals = mint_data[44];
    drop(mint_data);

    // Enforce the minimum raise amount, scaled to the mint's decimal precision.
    let scaled_min = MIN_AMOUNT_TO_RAISE
        .checked_mul(10u64.pow(decimals as u32))
        .ok_or(ProgramError::ArithmeticOverflow)?;
    if amount_to_raise < scaled_min {
        return Err(ProgramError::InvalidArgument);
    }

    // Prevent re-initialisation.
    if fundraiser.owned_by(&crate::ID) {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    // Create the fundraiser PDA account.
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

    // Populate the fundraiser state.
    let fundraiser_data = Fundraiser::from_account_info(fundraiser)?;
    fundraiser_data.set_maker(maker.address());
    fundraiser_data.set_mint_to_raise(mint_to_raise.address());
    fundraiser_data.set_amount_to_raise(amount_to_raise);
    fundraiser_data.set_current_amount(0);
    fundraiser_data.set_time_started(time_started);
    fundraiser_data.duration = duration;
    fundraiser_data.bump = bump;

    // Initialise the vault ATA owned by the fundraiser PDA.
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

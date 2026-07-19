use pinocchio::{
    AccountView, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{Sysvar, clock::Clock, rent::Rent},
};
use pinocchio_idl_macros::p_instruction;
use pinocchio_system::instructions::CreateAccount;

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
#[p_instruction(
    id = 1,
    inject,
    accounts = [
        contributor(signer, mut),
        mint_to_raise(relations = [fundraiser]),
        fundraiser(mut, state = Fundraiser),
        contributor_account(mut,
            pda = [b"contributor", contributor, bump],
            state = Contributor
        ),
        contributor_ata(mut, ata = [contributor, mint_to_raise]),
        vault(mut, ata = [fundraiser, mint_to_raise]),
    ],
    data = [
        amount: u64 = data[0..8],
        bump:   u8  = data[8]
    ]
)]
pub fn process_contribute_instruction(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    // All account bindings must appear before any other statements.
    let [
        contributor,
        mint_to_raise,
        fundraiser,
        contributor_account,
        contributor_ata,
        vault,
        _system_program,
        _token_program,
        _associated_token_program,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let fundraiser_data = Fundraiser::from_account_info(fundraiser)?;

    // Validate the mint account has a readable decimals byte at offset 44.
    let mint_data = mint_to_raise.try_borrow()?;
    if mint_data.len() < 45 {
        return Err(ProgramError::InvalidAccountData);
    }
    let decimals = mint_data[44];
    drop(mint_data);

    // Reject dust contributions below one base unit.
    if amount <= 10u64.pow(decimals as u32) {
        return Err(ProgramError::InvalidArgument);
    }

    // Reject contributions above the per-contributor cap.
    let cap = (fundraiser_data.amount_to_raise() * MAX_CONTRIBUTION_PERCENTAGE) / PERCENTAGE_SCALER;
    if amount > cap {
        return Err(ProgramError::InvalidArgument);
    }

    // Reject contributions after the campaign duration has elapsed.
    let current_time = Clock::get()?.unix_timestamp;
    let elapsed_days = ((current_time - fundraiser_data.time_started()) / SECONDS_TO_DAYS) as u8;
    if elapsed_days >= fundraiser_data.duration {
        return Err(ProgramError::InvalidArgument);
    }

    // Create the contributor tracking account if this is the first contribution.
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

    // Reject if this contribution would push the contributor over their lifetime cap.
    let new_total = contributor_data
        .amount()
        .checked_add(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    if new_total > cap {
        return Err(ProgramError::InvalidArgument);
    }

    // Transfer tokens from the contributor's ATA to the vault.
    pinocchio_token::instructions::Transfer::new(contributor_ata, vault, contributor, amount)
        .invoke()?;

    // Update on-chain balances.
    contributor_data.set_amount(new_total);

    let new_current = fundraiser_data
        .current_amount()
        .checked_add(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    fundraiser_data.set_current_amount(new_current);

    Ok(())
}

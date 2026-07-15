use pinocchio::{
    AccountView, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{Sysvar, clock::Clock},
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
#[p_instruction(
    id = 3,
    accounts = [
        contributor(signer, mut),
        maker(),
        mint_to_raise(relations = [fundraiser]),
        fundraiser(mut,
            pda = [b"fundraiser", maker, bump],
            state = Fundraiser
        ),
        contributor_account(mut,
            pda = [b"contributor", contributor, contributor_bump],
            state = Contributor
        ),
        contributor_ata(mut, ata = [contributor, mint_to_raise]),
        vault(mut, ata = [fundraiser, mint_to_raise]),
        system_program,
        token_program
    ],
    data = [
        bump:             u8 = data[0],
        contributor_bump: u8 = data[1]
    ]
)]
pub fn process_refund_instruction(
    accounts: &mut [AccountView],
    data: &[u8],
) -> ProgramResult {
    // All account bindings must appear before any other statements.
    let [
        contributor,
        maker,
        mint_to_raise,
        fundraiser,
        contributor_account,
        contributor_ata,
        vault,
        _system_program,
        _token_program,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let fundraiser_data = Fundraiser::from_account_info(fundraiser)?;

    // Verify the maker account matches the one stored in campaign state.
    if fundraiser_data.maker() != maker.address() {
        return Err(ProgramError::InvalidAccountData);
    }

    let contributor_data = Contributor::from_account_info(contributor_account)?;

    // Refunds are only available after the campaign duration has elapsed.
    let current_time = Clock::get()?.unix_timestamp;
    let elapsed_days = ((current_time - fundraiser_data.time_started()) / SECONDS_TO_DAYS) as u8;
    if elapsed_days < fundraiser_data.duration {
        return Err(ProgramError::InvalidArgument);
    }

    // Read the vault balance and confirm the target was not met.
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

    // Deduct refunded amount from campaign running total.
    let new_current = fundraiser_data
        .current_amount()
        .saturating_sub(refund_amount);
    fundraiser_data.set_current_amount(new_current);

    // Transfer tokens from the vault back to the contributor's ATA,
    // signing on behalf of the fundraiser PDA.
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

    // Close the contributor account and return its lamports to the contributor.
    let contributor_lamports = contributor_account.lamports();
    contributor.set_lamports(contributor.lamports() + contributor_lamports);
    contributor_account.set_lamports(0);
    let _ = contributor_account.close();

    Ok(())
}

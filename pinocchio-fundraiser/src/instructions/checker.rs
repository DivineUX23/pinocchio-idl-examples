use pinocchio::{
    AccountView, ProgramResult,
    cpi::{Seed, Signer},
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
#[p_instruction(
    id = 2,
    accounts = [
        maker(mut, signer),
        mint_to_raise(relations = [fundraiser]),
        fundraiser(mut,
            pda = [b"fundraiser", maker, bump],
            state = Fundraiser
        ),
        vault(mut, ata = [fundraiser, mint_to_raise]),
        maker_ata(mut, ata = [maker, mint_to_raise]),
    ],
    data = [
        bump: u8 = data[0]
    ]
)]
pub fn process_checker_instruction(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    // All account bindings must appear before any other statements.
    let [
        maker,
        mint_to_raise,
        fundraiser,
        vault,
        maker_ata,
        _system_program,
        _token_program,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let fundraiser_data = Fundraiser::from_account_info(fundraiser)?;

    // Read the vault's token balance from the raw account data (offset 64..72).
    let vault_data = unsafe { vault.borrow_unchecked() };
    let vault_amount = u64::from_le_bytes(
        vault_data[64..72]
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?,
    );

    // The target must be fully met before the maker can withdraw.
    if vault_amount < fundraiser_data.amount_to_raise() {
        return Err(ProgramError::InvalidArgument);
    }

    // Transfer all collected tokens from the vault to the maker's ATA,
    // signing on behalf of the fundraiser PDA.
    let bump_bytes = [bump];
    let signer_seeds = [
        Seed::from(b"fundraiser"),
        Seed::from(maker.address().as_array()),
        Seed::from(bump_bytes.as_ref()),
    ];
    let signer = Signer::from(&signer_seeds);

    pinocchio_token::instructions::Transfer::new(vault, maker_ata, fundraiser, vault_amount)
        .invoke_signed(&[signer])?;

    // Close the fundraiser account and return its lamports to the maker.
    let fundraiser_lamports = fundraiser.lamports();
    maker.set_lamports(maker.lamports() + fundraiser_lamports);
    fundraiser.set_lamports(0);
    let _ = fundraiser.close();

    Ok(())
}

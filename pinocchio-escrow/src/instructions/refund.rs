use pinocchio::{
    AccountView, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
};
use pinocchio_idl_macros::p_instruction;

use crate::state::Escrow;

#[p_instruction(
    id = 2,
    accounts = [
        maker(signer, mut),
        mint_a(relations = [escrow]),
        escrow(mut,
            pda = [b"escrow", maker, seed, bump],
            state = Escrow
        ),
        vault(mut, ata = [escrow, mint_a]),
        maker_ata(mut, ata = [maker, mint_a])
    ],
    data = [
        seed:     u64 = data[0..8],
        bump:     u8  = data[8],
        amount_a: u64 = data[9..17]
    ]
)]
pub fn process_refund_instruction(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    // Extract ALL account bindings contiguously at the start of the function body.
    let [
        maker,
        mint_a,
        escrow,
        vault,
        maker_ata,
        _token_program,
        _system_program,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    {
        let escrow_data = Escrow::from_account_info(escrow)?;
        if escrow_data.maker() != maker.address() {
            return Err(ProgramError::InvalidAccountData);
        }
    }

    // Refund tokens from vault back to maker (signed by escrow PDA)
    let seed_bytes = seed.to_le_bytes();
    let bump_bytes = [bump];
    let signer_seeds = [
        Seed::from(b"escrow"),
        Seed::from(maker.address().as_array()),
        Seed::from(seed_bytes.as_ref()),
        Seed::from(bump_bytes.as_ref()),
    ];
    let signer = Signer::from(&signer_seeds);

    pinocchio_token::instructions::Transfer::new(vault, maker_ata, escrow, amount_a)
        .invoke_signed(&[signer])?;

    // Close the escrow account by reclaiming its rent lamports
    let escrow_lamports = escrow.lamports();
    maker.set_lamports(maker.lamports() + escrow_lamports);
    escrow.set_lamports(0);
    let _ = escrow.close();

    Ok(())
}

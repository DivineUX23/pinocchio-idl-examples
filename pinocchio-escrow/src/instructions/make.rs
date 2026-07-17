use pinocchio::{
    AccountView, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{Sysvar, rent::Rent},
};
use pinocchio_idl_macros::p_instruction;
use pinocchio_system::instructions::CreateAccount;

use crate::state::Escrow;

#[p_instruction(
    id = 0,
    accounts = [
        maker(signer, mut),
        mint_a(relations = [escrow]),
        mint_b(relations = [escrow]),
        escrow(mut,
            pda = [b"escrow", maker, seed, bump],
            state = Escrow
        ),
        vault(mut, init = [escrow, mint_a]),
        maker_ata(mut, ata = [maker, mint_a])
    ],
    data = [
        seed:     u64 = data[0..8],
        amount_a: u64 = data[8..16],
        amount_b: u64 = data[16..24],
        bump:     u8  = data[24]
    ]
)]
pub fn process_make_instruction(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    // Extract ALL account bindings contiguously at the start of the function body.
    let [
        maker,
        mint_a,
        mint_b,
        escrow,
        vault,
        maker_ata,
        system_program,
        token_program,
        _associated_token_program,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if escrow.owned_by(&crate::ID) {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    let seed_bytes = seed.to_le_bytes();
    let bump_bytes = [bump];
    let signer_seeds = [
        Seed::from(b"escrow"),
        Seed::from(maker.address().as_array()),
        Seed::from(seed_bytes.as_ref()),
        Seed::from(bump_bytes.as_ref()),
    ];
    let signer = Signer::from(&signer_seeds);

    CreateAccount {
        from: maker,
        to: escrow,
        lamports: Rent::get()?.try_minimum_balance(Escrow::LEN)?,
        space: Escrow::LEN as u64,
        owner: &crate::ID,
    }
    .invoke_signed(&[signer])?;

    let escrow_data = Escrow::from_account_info(escrow)?;
    escrow_data.seed = seed;
    escrow_data.set_maker(maker.address());
    escrow_data.set_mint_a(mint_a.address());
    escrow_data.set_mint_b(mint_b.address());
    escrow_data.amount_a = amount_a;
    escrow_data.amount_b = amount_b;
    escrow_data.bump = bump;

    pinocchio_associated_token_account::instructions::Create {
        funding_account: maker,
        account: vault,
        wallet: escrow,
        mint: mint_a,
        token_program,
        system_program,
    }
    .invoke()?;

    pinocchio_token::instructions::Transfer::new(maker_ata, vault, maker, amount_a).invoke()?;

    Ok(())
}

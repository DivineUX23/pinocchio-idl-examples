use pinocchio::{
    AccountView, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{Sysvar, rent::Rent},
};
use pinocchio_idl_macros::p_instruction;
use pinocchio_system::instructions::CreateAccount;

use crate::state::Counter;

#[p_instruction(
    id = 0,
    inject,
    accounts = [
        authority(signer, mut),
        counter(mut,
            pda = [b"counter", authority, bump],
            state = Counter
        ),
    ],
    data = [
        bump: u8 = data[0]
    ]
)]
pub fn process_initialize_instruction(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    // Extract ALL account bindings contiguously at the start of the function body.
    let [authority, counter, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if counter.owned_by(&crate::ID) {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    let bump_bytes = [bump];
    let signer_seeds = [
        Seed::from(b"counter"),
        Seed::from(authority.address().as_array()),
        Seed::from(bump_bytes.as_ref()),
    ];
    let signer = Signer::from(&signer_seeds);

    CreateAccount {
        from: authority,
        to: counter,
        lamports: Rent::get()?.try_minimum_balance(Counter::LEN)?,
        space: Counter::LEN as u64,
        owner: &crate::ID,
    }
    .invoke_signed(&[signer])?;

    let counter_data = Counter::from_account_info(counter)?;
    counter_data.set_authority(authority.address());
    counter_data.set_count(0);
    counter_data.bump = bump;

    Ok(())
}

use pinocchio::{AccountView, ProgramResult, error::ProgramError};
use pinocchio_idl_macros::p_instruction;

use crate::{error::CounterError, state::Counter};

#[p_instruction(
    id = 2,
    accounts = [
        authority(signer),
        counter(mut,
            pda = [b"counter", authority, bump],
            state = Counter
        )
    ],
    data = [
        bump: u8 = data[0]
    ]
)]
pub fn process_decrement_instruction(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    // Extract ALL account bindings contiguously at the start of the function body.
    let [authority, counter] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let mut counter_data = Counter::from_account_info(counter)?;
    if counter_data.authority() != authority.address() {
        return Err(ProgramError::InvalidAccountData);
    }

    let new_count = counter_data.count().checked_sub(1).unwrap();

    counter_data.set_count(new_count);

    Ok(())
}

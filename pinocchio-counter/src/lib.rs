#![allow(unexpected_cfgs)]

use pinocchio::{
    AccountView, Address, ProgramResult,
    address::declare_id,
    entrypoint,
    error::ProgramError,
};

mod error;
mod instructions;
mod state;

use instructions::*;

entrypoint!(process_instruction);

declare_id!("DM5R2269qS18hfHq54eHqZSEMkajVQDFgxn3UuYYhCJP");

pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    assert_eq!(program_id, &ID);

    let (discriminator, data) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidAccountData)?;

    match CounterInstruction::try_from(discriminator)? {
        CounterInstruction::Initialize => process_initialize_instruction(accounts, data)?,
        CounterInstruction::Increment  => process_increment_instruction(accounts, data)?,
        CounterInstruction::Decrement  => process_decrement_instruction(accounts, data)?,
    }

    Ok(())
}

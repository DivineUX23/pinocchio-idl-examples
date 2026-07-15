#![allow(unexpected_cfgs)]

use pinocchio::{
    AccountView, Address, ProgramResult,
    address::declare_id,
    entrypoint,
    error::ProgramError,
};

mod constants;
mod error;
mod instructions;
mod state;

use instructions::*;

entrypoint!(process_instruction);

declare_id!("Escrow1111111111111111111111111111111111111");

pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    assert_eq!(program_id, &ID);

    let (discriminator, data) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidAccountData)?;

    match EscrowInstruction::try_from(discriminator)? {
        EscrowInstruction::Make   => process_make_instruction(accounts, data)?,
        EscrowInstruction::Take   => process_take_instruction(accounts, data)?,
        EscrowInstruction::Refund => process_refund_instruction(accounts, data)?,
    }

    Ok(())
}

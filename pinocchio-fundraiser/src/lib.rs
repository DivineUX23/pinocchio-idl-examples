#![allow(unexpected_cfgs)]

use pinocchio::{
    AccountView, Address, ProgramResult,
    address::declare_id,
    entrypoint,
    error::ProgramError,
};

mod constants;
mod instructions;
mod state;

#[cfg(test)]
mod tests;

use constants::*;
use instructions::*;

entrypoint!(process_instruction);

declare_id!("96TFrsG998MvvrfuShRQmSemkzN555pnidGF4gquJsKr");

pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    assert_eq!(program_id, &ID);

    let (discriminator, data) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidAccountData)?;

    match FundraiserInstructions::try_from(discriminator)? {
        FundraiserInstructions::Initialize  => process_initialize_instruction(accounts, data)?,
        FundraiserInstructions::Contributor => process_contribute_instruction(accounts, data)?,
        FundraiserInstructions::Checker     => process_checker_instruction(accounts, data)?,
        FundraiserInstructions::Refund      => process_refund_instruction(accounts, data)?,
    }

    Ok(())
}

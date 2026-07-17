use litesvm::LiteSVM;
use solana_sdk::{
    instruction::{AccountMeta, Instruction, InstructionError},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use std::str::FromStr;

const PROGRAM_ID_STR: &str = "FDYpkuY64WaazRCsReFHTc32VeQwBxyev5DfUqBhwySA";

fn token_program_id() -> Pubkey {
    Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap()
}

fn associated_token_program_id() -> Pubkey {
    Pubkey::from_str("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL").unwrap()
}

fn system_program_id() -> Pubkey {
    Pubkey::default()
}

fn setup_svm() -> (LiteSVM, Pubkey, Keypair) {
    let mut svm = LiteSVM::new();
    let program_id = Pubkey::from_str(PROGRAM_ID_STR).unwrap();

    // Load SBF program
    svm.add_program_from_file(program_id, "target/deploy/pinocchio_escrow.so")
        .expect("Failed to load pinocchio_escrow.so");

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    (svm, program_id, payer)
}

fn create_mint(svm: &mut LiteSVM, payer: &Keypair) -> Pubkey {
    litesvm_token::CreateMint::new(svm, payer)
        .decimals(9)
        .send()
        .unwrap()
}

fn create_ata_with_balance(
    svm: &mut LiteSVM,
    payer: &Keypair,
    owner: &Pubkey,
    mint: &Pubkey,
    amount: u64,
) -> Pubkey {
    let ata = litesvm_token::CreateAssociatedTokenAccount::new(svm, payer, mint)
        .owner(owner)
        .send()
        .unwrap();

    if amount > 0 {
        litesvm_token::MintTo::new(svm, payer, mint, &ata, amount)
            .send()
            .unwrap();
    }

    ata
}

#[test]
fn test_escrow_happy_path_take() {
    let (mut svm, program_id, payer) = setup_svm();

    let maker = Keypair::new();
    let taker = Keypair::new();
    svm.airdrop(&maker.pubkey(), 2_000_000_000).unwrap();
    svm.airdrop(&taker.pubkey(), 2_000_000_000).unwrap();

    let mint_a = create_mint(&mut svm, &payer);
    let mint_b = create_mint(&mut svm, &payer);

    let maker_ata_a = create_ata_with_balance(&mut svm, &payer, &maker.pubkey(), &mint_a, 1000);
    let taker_ata_b = create_ata_with_balance(&mut svm, &payer, &taker.pubkey(), &mint_b, 500);

    // ATAs for exchange receiving
    let taker_ata_a = create_ata_with_balance(&mut svm, &payer, &taker.pubkey(), &mint_a, 0);
    let maker_ata_b = create_ata_with_balance(&mut svm, &payer, &maker.pubkey(), &mint_b, 0);

    let seed: u64 = 42;
    let amount_a: u64 = 400;
    let amount_b: u64 = 200;

    let (escrow_pda, bump) = Pubkey::find_program_address(
        &[b"escrow", maker.pubkey().as_ref(), &seed.to_le_bytes()],
        &program_id,
    );

    let vault = spl_associated_token_account::get_associated_token_address_with_program_id(
        &escrow_pda,
        &mint_a,
        &token_program_id(),
    );

    // 1. MAKE
    let accounts = vec![
        AccountMeta::new(maker.pubkey(), true),
        AccountMeta::new_readonly(mint_a, false),
        AccountMeta::new_readonly(mint_b, false),
        AccountMeta::new(escrow_pda, false),
        AccountMeta::new(vault, false),
        AccountMeta::new(maker_ata_a, false),
        AccountMeta::new_readonly(system_program_id(), false),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new_readonly(associated_token_program_id(), false),
    ];

    let mut data = vec![0]; // Make discriminator (0)
    data.extend_from_slice(&seed.to_le_bytes());
    data.extend_from_slice(&amount_a.to_le_bytes());
    data.extend_from_slice(&amount_b.to_le_bytes());
    data.push(bump);

    let ix = Instruction {
        program_id,
        accounts,
        data,
    };

    let tx = Transaction::new(
        &[&maker],
        Message::new(&[ix], Some(&maker.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx)
        .expect("Failed to call MAKE instruction");

    // Verify maker ATA has been debited
    let maker_ata_a_acc = svm.get_account(&maker_ata_a).unwrap();
    let balance_maker_a = u64::from_le_bytes(maker_ata_a_acc.data[64..72].try_into().unwrap());
    assert_eq!(balance_maker_a, 600);

    // Verify vault has been credited
    let vault_acc = svm.get_account(&vault).unwrap();
    let balance_vault = u64::from_le_bytes(vault_acc.data[64..72].try_into().unwrap());
    assert_eq!(balance_vault, 400);

    // Verify escrow PDA state
    let escrow_acc = svm.get_account(&escrow_pda).unwrap();
    assert_eq!(escrow_acc.data.len(), 128);
    let stored_seed = u64::from_le_bytes(escrow_acc.data[0..8].try_into().unwrap());
    let stored_maker: [u8; 32] = escrow_acc.data[8..40].try_into().unwrap();
    let stored_mint_a: [u8; 32] = escrow_acc.data[40..72].try_into().unwrap();
    let stored_mint_b: [u8; 32] = escrow_acc.data[72..104].try_into().unwrap();
    let stored_amount_a = u64::from_le_bytes(escrow_acc.data[104..112].try_into().unwrap());
    let stored_amount_b = u64::from_le_bytes(escrow_acc.data[112..120].try_into().unwrap());
    let stored_bump = escrow_acc.data[120];

    assert_eq!(stored_seed, seed);
    assert_eq!(stored_maker, maker.pubkey().to_bytes());
    assert_eq!(stored_mint_a, mint_a.to_bytes());
    assert_eq!(stored_mint_b, mint_b.to_bytes());
    assert_eq!(stored_amount_a, amount_a);
    assert_eq!(stored_amount_b, amount_b);
    assert_eq!(stored_bump, bump);

    // 2. TAKE
    let accounts = vec![
        AccountMeta::new(taker.pubkey(), true),
        AccountMeta::new(maker.pubkey(), false),
        AccountMeta::new_readonly(mint_a, false),
        AccountMeta::new_readonly(mint_b, false),
        AccountMeta::new(escrow_pda, false),
        AccountMeta::new(vault, false),
        AccountMeta::new(taker_ata_a, false),
        AccountMeta::new(taker_ata_b, false),
        AccountMeta::new(maker_ata_b, false),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new_readonly(system_program_id(), false),
    ];

    let mut data = vec![1]; // Take discriminator (1)
    data.extend_from_slice(&seed.to_le_bytes());
    data.push(bump);
    data.extend_from_slice(&amount_a.to_le_bytes());
    data.extend_from_slice(&amount_b.to_le_bytes());

    let ix = Instruction {
        program_id,
        accounts,
        data,
    };

    let tx = Transaction::new(
        &[&taker],
        Message::new(&[ix], Some(&taker.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx)
        .expect("Failed to call TAKE instruction");

    // Verify Taker ATA A has received the escrowed tokens
    let taker_ata_a_acc = svm.get_account(&taker_ata_a).unwrap();
    let balance_taker_a = u64::from_le_bytes(taker_ata_a_acc.data[64..72].try_into().unwrap());
    assert_eq!(balance_taker_a, 400);

    // Verify Maker ATA B has received the expected return tokens
    let maker_ata_b_acc = svm.get_account(&maker_ata_b).unwrap();
    let balance_maker_b = u64::from_le_bytes(maker_ata_b_acc.data[64..72].try_into().unwrap());
    assert_eq!(balance_maker_b, 200);

    // Verify Taker ATA B was debited
    let taker_ata_b_acc = svm.get_account(&taker_ata_b).unwrap();
    let balance_taker_b = u64::from_le_bytes(taker_ata_b_acc.data[64..72].try_into().unwrap());
    assert_eq!(balance_taker_b, 300);

    // Verify escrow account is closed (returns None or 0 lamports)
    assert!(svm.get_account(&escrow_pda).is_none());
}

#[test]
fn test_escrow_happy_path_refund() {
    let (mut svm, program_id, payer) = setup_svm();

    let maker = Keypair::new();
    svm.airdrop(&maker.pubkey(), 2_000_000_000).unwrap();

    let mint_a = create_mint(&mut svm, &payer);
    let mint_b = create_mint(&mut svm, &payer);

    let maker_ata_a = create_ata_with_balance(&mut svm, &payer, &maker.pubkey(), &mint_a, 1000);

    let seed: u64 = 100;
    let amount_a: u64 = 500;
    let amount_b: u64 = 250;

    let (escrow_pda, bump) = Pubkey::find_program_address(
        &[b"escrow", maker.pubkey().as_ref(), &seed.to_le_bytes()],
        &program_id,
    );

    let vault = spl_associated_token_account::get_associated_token_address_with_program_id(
        &escrow_pda,
        &mint_a,
        &token_program_id(),
    );

    // 1. MAKE
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(maker.pubkey(), true),
            AccountMeta::new_readonly(mint_a, false),
            AccountMeta::new_readonly(mint_b, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(maker_ata_a, false),
            AccountMeta::new_readonly(system_program_id(), false),
            AccountMeta::new_readonly(token_program_id(), false),
            AccountMeta::new_readonly(associated_token_program_id(), false),
        ],
        data: {
            let mut data = vec![0];
            data.extend_from_slice(&seed.to_le_bytes());
            data.extend_from_slice(&amount_a.to_le_bytes());
            data.extend_from_slice(&amount_b.to_le_bytes());
            data.push(bump);
            data
        },
    };

    let tx = Transaction::new(
        &[&maker],
        Message::new(&[ix], Some(&maker.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    // 2. REFUND
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(maker.pubkey(), true),
            AccountMeta::new_readonly(mint_a, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(maker_ata_a, false),
            AccountMeta::new_readonly(token_program_id(), false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: {
            let mut data = vec![2]; // Refund discriminator (2)
            data.extend_from_slice(&seed.to_le_bytes());
            data.push(bump);
            data.extend_from_slice(&amount_a.to_le_bytes());
            data
        },
    };

    let tx = Transaction::new(
        &[&maker],
        Message::new(&[ix], Some(&maker.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).expect("Failed to call REFUND");

    // Verify maker ATA has all its tokens back
    let maker_ata_a_acc = svm.get_account(&maker_ata_a).unwrap();
    let balance_maker_a = u64::from_le_bytes(maker_ata_a_acc.data[64..72].try_into().unwrap());
    assert_eq!(balance_maker_a, 1000);

    // Verify escrow is closed
    assert!(svm.get_account(&escrow_pda).is_none());
}

#[test]
fn test_make_security_checks() {
    let (mut svm, program_id, payer) = setup_svm();
    let maker = Keypair::new();
    svm.airdrop(&maker.pubkey(), 2_000_000_000).unwrap();

    let mint_a = create_mint(&mut svm, &payer);
    let mint_b = create_mint(&mut svm, &payer);

    let maker_ata_a = create_ata_with_balance(&mut svm, &payer, &maker.pubkey(), &mint_a, 1000);

    let seed: u64 = 42;
    let (escrow_pda, bump) = Pubkey::find_program_address(
        &[b"escrow", maker.pubkey().as_ref(), &seed.to_le_bytes()],
        &program_id,
    );
    let vault = spl_associated_token_account::get_associated_token_address_with_program_id(
        &escrow_pda,
        &mint_a,
        &token_program_id(),
    );

    let fee_payer = Keypair::new();
    svm.airdrop(&fee_payer.pubkey(), 10_000_000_000).unwrap();

    // 1. Not Enough Accounts
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(maker.pubkey(), true),
            AccountMeta::new_readonly(mint_a, false),
        ],
        data: vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, bump,
        ],
    };
    let tx = Transaction::new(
        &[&maker],
        Message::new(&[ix], Some(&maker.pubkey())),
        svm.latest_blockhash(),
    );
    let err = svm.send_transaction(tx).unwrap_err();
    assert!(matches!(
        err.err,
        TransactionError::InstructionError(0, InstructionError::NotEnoughAccountKeys)
    ));

    // 2. Maker Not Signer
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(maker.pubkey(), false),
            AccountMeta::new_readonly(mint_a, false),
            AccountMeta::new_readonly(mint_b, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(maker_ata_a, false),
            AccountMeta::new_readonly(system_program_id(), false),
            AccountMeta::new_readonly(token_program_id(), false),
            AccountMeta::new_readonly(associated_token_program_id(), false),
        ],
        data: {
            let mut d = vec![0];
            d.extend_from_slice(&seed.to_le_bytes());
            d.extend_from_slice(&400u64.to_le_bytes());
            d.extend_from_slice(&200u64.to_le_bytes());
            d.push(bump);
            d
        },
    };
    let tx = Transaction::new(
        &[&fee_payer],
        Message::new(&[ix], Some(&fee_payer.pubkey())),
        svm.latest_blockhash(),
    );
    let err = svm.send_transaction(tx).unwrap_err();
    assert!(matches!(
        err.err,
        TransactionError::InstructionError(0, InstructionError::MissingRequiredSignature)
    ));
}

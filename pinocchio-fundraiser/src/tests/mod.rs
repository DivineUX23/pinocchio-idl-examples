use litesvm::LiteSVM;
use solana_sdk::{
    instruction::{AccountMeta, Instruction, InstructionError},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    sysvar::clock::Clock as SolanaClock,
    transaction::{Transaction, TransactionError},
};
use std::str::FromStr;

const PROGRAM_ID_STR: &str = "96TFrsG998MvvrfuShRQmSemkzN555pnidGF4gquJsKr";

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
    svm.add_program_from_file(program_id, "target/deploy/pinocchio_fundraiser.so")
        .expect("Failed to load pinocchio_fundraiser.so");

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    (svm, program_id, payer)
}

fn create_mint(svm: &mut LiteSVM, payer: &Keypair) -> Pubkey {
    litesvm_token::CreateMint::new(svm, payer)
        .decimals(0) // 0 decimals makes calculations super simple
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
fn test_fundraiser_happy_path_success() {
    let (mut svm, program_id, payer) = setup_svm();

    let maker = Keypair::new();
    svm.airdrop(&maker.pubkey(), 2_000_000_000).unwrap();

    let mint_to_raise = create_mint(&mut svm, &payer);

    let (fundraiser_pda, bump) =
        Pubkey::find_program_address(&[b"fundraiser", maker.pubkey().as_ref()], &program_id);

    let vault = spl_associated_token_account::get_associated_token_address_with_program_id(
        &fundraiser_pda,
        &mint_to_raise,
        &token_program_id(),
    );

    let current_time = svm.get_sysvar::<SolanaClock>().unix_timestamp;

    // 1. INITIALIZE
    // accounts: maker, fundraiser, mint_to_raise, vault, system_program, token_program, associated_token_program
    let accounts = vec![
        AccountMeta::new(maker.pubkey(), true),
        AccountMeta::new_readonly(mint_to_raise, false),
        AccountMeta::new(fundraiser_pda, false),
        AccountMeta::new(vault, false),
        AccountMeta::new_readonly(system_program_id(), false),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new_readonly(associated_token_program_id(), false),
    ];

    let amount_to_raise: u64 = 30; // Min is 3
    let duration: u8 = 5; // 5 days

    let mut data = vec![0]; // Initialize discriminator (0)
    data.extend_from_slice(&amount_to_raise.to_le_bytes());
    data.extend_from_slice(&current_time.to_le_bytes());
    data.push(duration);
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
        .expect("Failed to INITIALIZE fundraiser");

    // Create 10 contributors to meet target
    let mut contributors = Vec::new();
    let mut contributor_atas = Vec::new();
    let mut contributor_pdas = Vec::new();

    for _i in 0..10 {
        let contributor = Keypair::new();
        svm.airdrop(&contributor.pubkey(), 1_000_000_000).unwrap();

        let ata =
            create_ata_with_balance(&mut svm, &payer, &contributor.pubkey(), &mint_to_raise, 3);

        let (contributor_pda, c_bump) = Pubkey::find_program_address(
            &[b"contributor", contributor.pubkey().as_ref()],
            &program_id,
        );

        contributors.push(contributor);
        contributor_atas.push(ata);
        contributor_pdas.push((contributor_pda, c_bump));
    }

    // 2. CONTRIBUTE (10 times)
    // accounts: contributor, mint_to_raise, fundraiser, contributor_account, contributor_ata, vault, system_program, token_program, associated_token_program
    for i in 0..10 {
        let contributor = &contributors[i];
        let contributor_ata = contributor_atas[i];
        let (contributor_pda, c_bump) = contributor_pdas[i];

        let accounts = vec![
            AccountMeta::new(contributor.pubkey(), true),
            AccountMeta::new_readonly(mint_to_raise, false),
            AccountMeta::new(fundraiser_pda, false),
            AccountMeta::new(contributor_pda, false),
            AccountMeta::new(contributor_ata, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program_id(), false),
            AccountMeta::new_readonly(token_program_id(), false),
            AccountMeta::new_readonly(associated_token_program_id(), false),
        ];

        let mut data = vec![1]; // Contribute discriminator (1)
        data.extend_from_slice(&3u64.to_le_bytes()); // contribute 3 tokens
        data.push(c_bump);

        let ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let tx = Transaction::new(
            &[contributor],
            Message::new(&[ix], Some(&contributor.pubkey())),
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).expect("Failed to CONTRIBUTE");
    }

    // Verify Fundraiser current amount
    let fundraiser_acc = svm.get_account(&fundraiser_pda).unwrap();
    let current_amount = u64::from_le_bytes(fundraiser_acc.data[72..80].try_into().unwrap());
    assert_eq!(current_amount, 30);

    // 3. CHECKER (Collect)
    // accounts: maker, mint_to_raise, fundraiser, vault, maker_ata, system_program, token_program
    let maker_ata = create_ata_with_balance(&mut svm, &payer, &maker.pubkey(), &mint_to_raise, 0);

    let accounts = vec![
        AccountMeta::new(maker.pubkey(), true),
        AccountMeta::new_readonly(mint_to_raise, false),
        AccountMeta::new(fundraiser_pda, false),
        AccountMeta::new(vault, false),
        AccountMeta::new(maker_ata, false),
        AccountMeta::new_readonly(system_program_id(), false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];

    let mut data = vec![2]; // Checker discriminator (2)
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
    svm.send_transaction(tx).expect("Failed to call CHECKER");

    // Verify maker received all raised tokens
    let maker_ata_acc = svm.get_account(&maker_ata).unwrap();
    let balance_maker = u64::from_le_bytes(maker_ata_acc.data[64..72].try_into().unwrap());
    assert_eq!(balance_maker, 30);

    // Verify fundraiser PDA is closed
    assert!(svm.get_account(&fundraiser_pda).is_none());
}

#[test]
fn test_fundraiser_happy_path_refund() {
    let (mut svm, program_id, payer) = setup_svm();

    let maker = Keypair::new();
    svm.airdrop(&maker.pubkey(), 2_000_000_000).unwrap();

    let mint_to_raise = create_mint(&mut svm, &payer);

    let (fundraiser_pda, bump) =
        Pubkey::find_program_address(&[b"fundraiser", maker.pubkey().as_ref()], &program_id);

    let vault = spl_associated_token_account::get_associated_token_address_with_program_id(
        &fundraiser_pda,
        &mint_to_raise,
        &token_program_id(),
    );

    // Initialize fundraiser with time_started in the past so it is already expired!
    let clock = svm.get_sysvar::<SolanaClock>();
    let current_time = clock.unix_timestamp;
    let expired_start_time = current_time - (5 * 86400) - 100; // 5 days + 100 seconds ago

    // 1. INITIALIZE
    let accounts = vec![
        AccountMeta::new(maker.pubkey(), true),
        AccountMeta::new_readonly(mint_to_raise, false),
        AccountMeta::new(fundraiser_pda, false),
        AccountMeta::new(vault, false),
        AccountMeta::new_readonly(system_program_id(), false),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new_readonly(associated_token_program_id(), false),
    ];

    let amount_to_raise: u64 = 30;
    let duration: u8 = 5;

    let mut data = vec![0];
    data.extend_from_slice(&amount_to_raise.to_le_bytes());
    data.extend_from_slice(&expired_start_time.to_le_bytes());
    data.push(duration);
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
    svm.send_transaction(tx).unwrap();

    // Create 2 contributors
    let contributor1 = Keypair::new();
    let contributor2 = Keypair::new();
    svm.airdrop(&contributor1.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&contributor2.pubkey(), 1_000_000_000).unwrap();

    let c1_ata =
        create_ata_with_balance(&mut svm, &payer, &contributor1.pubkey(), &mint_to_raise, 3);
    let c2_ata =
        create_ata_with_balance(&mut svm, &payer, &contributor2.pubkey(), &mint_to_raise, 3);

    let (c1_pda, c1_bump) = Pubkey::find_program_address(
        &[b"contributor", contributor1.pubkey().as_ref()],
        &program_id,
    );
    let (c2_pda, c2_bump) = Pubkey::find_program_address(
        &[b"contributor", contributor2.pubkey().as_ref()],
        &program_id,
    );

    // Temporarily set the Clock back to current_time (before expiration) so we can contribute
    let mut contribution_clock = clock.clone();
    contribution_clock.unix_timestamp = expired_start_time + 10;
    svm.set_sysvar::<SolanaClock>(&contribution_clock);

    // Contributor 1 contributes
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(contributor1.pubkey(), true),
            AccountMeta::new_readonly(mint_to_raise, false),
            AccountMeta::new(fundraiser_pda, false),
            AccountMeta::new(c1_pda, false),
            AccountMeta::new(c1_ata, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program_id(), false),
            AccountMeta::new_readonly(token_program_id(), false),
            AccountMeta::new_readonly(associated_token_program_id(), false),
        ],
        data: {
            let mut d = vec![1];
            d.extend_from_slice(&3u64.to_le_bytes());
            d.push(c1_bump);
            d
        },
    };
    let tx = Transaction::new(
        &[&contributor1],
        Message::new(&[ix], Some(&contributor1.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    // Contributor 2 contributes
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(contributor2.pubkey(), true),
            AccountMeta::new_readonly(mint_to_raise, false),
            AccountMeta::new(fundraiser_pda, false),
            AccountMeta::new(c2_pda, false),
            AccountMeta::new(c2_ata, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program_id(), false),
            AccountMeta::new_readonly(token_program_id(), false),
            AccountMeta::new_readonly(associated_token_program_id(), false),
        ],
        data: {
            let mut d = vec![1];
            d.extend_from_slice(&3u64.to_le_bytes());
            d.push(c2_bump);
            d
        },
    };
    let tx = Transaction::new(
        &[&contributor2],
        Message::new(&[ix], Some(&contributor2.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    // Restore Clock to current_time (expired state)
    svm.set_sysvar::<SolanaClock>(&clock);

    // 2. REFUND (Contributor 1)
    // accounts: contributor, maker, mint_to_raise, fundraiser, contributor_account, contributor_ata, vault, system_program, token_program
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(contributor1.pubkey(), true),
            AccountMeta::new(maker.pubkey(), false),
            AccountMeta::new_readonly(mint_to_raise, false),
            AccountMeta::new(fundraiser_pda, false),
            AccountMeta::new(c1_pda, false),
            AccountMeta::new(c1_ata, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program_id(), false),
            AccountMeta::new_readonly(token_program_id(), false),
        ],
        data: vec![3, bump, c1_bump], // Refund discriminator (3)
    };
    let tx = Transaction::new(
        &[&contributor1],
        Message::new(&[ix], Some(&contributor1.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx)
        .expect("Refund for contributor 1 failed");

    // Verify Contributor 1 got 3 tokens back
    let c1_ata_acc = svm.get_account(&c1_ata).unwrap();
    let balance_c1 = u64::from_le_bytes(c1_ata_acc.data[64..72].try_into().unwrap());
    assert_eq!(balance_c1, 3);

    // Verify Contributor 1 PDA is closed
    assert!(svm.get_account(&c1_pda).is_none());
}

#[test]
fn test_initialize_security_checks() {
    let (mut svm, program_id, payer) = setup_svm();
    let maker = Keypair::new();
    svm.airdrop(&maker.pubkey(), 2_000_000_000).unwrap();

    let mint_to_raise = create_mint(&mut svm, &payer);

    let (fundraiser_pda, bump) =
        Pubkey::find_program_address(&[b"fundraiser", maker.pubkey().as_ref()], &program_id);

    let vault = spl_associated_token_account::get_associated_token_address_with_program_id(
        &fundraiser_pda,
        &mint_to_raise,
        &token_program_id(),
    );

    let current_time = svm.get_sysvar::<SolanaClock>().unix_timestamp;

    let fee_payer = Keypair::new();
    svm.airdrop(&fee_payer.pubkey(), 10_000_000_000).unwrap();

    // 1. Not Enough Accounts
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(maker.pubkey(), true),
            AccountMeta::new(fundraiser_pda, false),
        ],
        data: vec![0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, bump],
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
            AccountMeta::new_readonly(mint_to_raise, false),
            AccountMeta::new(fundraiser_pda, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program_id(), false),
            AccountMeta::new_readonly(token_program_id(), false),
            AccountMeta::new_readonly(associated_token_program_id(), false),
        ],
        data: {
            let mut d = vec![0];
            d.extend_from_slice(&3u64.to_le_bytes());
            d.extend_from_slice(&current_time.to_le_bytes());
            d.push(5);
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

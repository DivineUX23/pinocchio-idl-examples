use litesvm::LiteSVM;
use solana_sdk::{
    instruction::{AccountMeta, Instruction, InstructionError},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use std::str::FromStr;

const PROGRAM_ID_STR: &str = "DM5R2269qS18hfHq54eHqZSEMkajVQDFgxn3UuYYhCJP";

fn system_program_id() -> Pubkey {
    Pubkey::default()
}

fn setup_svm() -> (LiteSVM, Pubkey, Keypair) {
    let mut svm = LiteSVM::new();
    let program_id = Pubkey::from_str(PROGRAM_ID_STR).unwrap();

    // Load SBF program
    svm.add_program_from_file(program_id, "target/deploy/pinocchio_counter.so")
        .expect("Failed to load pinocchio_counter.so");

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    (svm, program_id, authority)
}

#[test]
fn test_counter_happy_path() {
    let (mut svm, program_id, authority) = setup_svm();

    let (counter_pda, bump) =
        Pubkey::find_program_address(&[b"counter", authority.pubkey().as_ref()], &program_id);

    // 1. Initialize
    let accounts = vec![
        AccountMeta::new(authority.pubkey(), true),
        AccountMeta::new(counter_pda, false),
        AccountMeta::new_readonly(system_program_id(), false),
    ];
    let mut data = vec![0]; // Initialize discriminator (0)
    data.push(bump);

    let ix = Instruction {
        program_id,
        accounts,
        data,
    };

    let tx = Transaction::new(
        &[&authority],
        Message::new(&[ix], Some(&authority.pubkey())),
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .expect("Failed to initialize counter");

    // Verify initial state
    let account = svm
        .get_account(&counter_pda)
        .expect("Counter account not found");
    assert_eq!(account.data.len(), 41);
    let stored_authority: [u8; 32] = account.data[0..32].try_into().unwrap();
    let stored_count = u64::from_le_bytes(account.data[32..40].try_into().unwrap());
    let stored_bump = account.data[40];

    assert_eq!(stored_authority, authority.pubkey().to_bytes());
    assert_eq!(stored_count, 0);
    assert_eq!(stored_bump, bump);

    // 2. Increment
    let accounts = vec![
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new(counter_pda, false),
    ];
    let mut data = vec![1]; // Increment discriminator (1)
    data.push(bump);

    let ix = Instruction {
        program_id,
        accounts,
        data,
    };

    let tx = Transaction::new(
        &[&authority],
        Message::new(&[ix], Some(&authority.pubkey())),
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .expect("Failed to increment counter");

    // Verify count is 1
    let account = svm.get_account(&counter_pda).unwrap();
    let stored_count = u64::from_le_bytes(account.data[32..40].try_into().unwrap());
    assert_eq!(stored_count, 1);

    // 3. Decrement
    let accounts = vec![
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new(counter_pda, false),
    ];
    let mut data = vec![2]; // Decrement discriminator (2)
    data.push(bump);

    let ix = Instruction {
        program_id,
        accounts,
        data,
    };

    let tx = Transaction::new(
        &[&authority],
        Message::new(&[ix], Some(&authority.pubkey())),
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .expect("Failed to decrement counter");

    // Verify count is 0
    let account = svm.get_account(&counter_pda).unwrap();
    let stored_count = u64::from_le_bytes(account.data[32..40].try_into().unwrap());
    assert_eq!(stored_count, 0);
}

#[test]
fn test_initialize_security_checks() {
    let (mut svm, program_id, authority) = setup_svm();

    let (counter_pda, bump) =
        Pubkey::find_program_address(&[b"counter", authority.pubkey().as_ref()], &program_id);

    // Create a separate fee payer for the non-signer tests
    let fee_payer = Keypair::new();
    svm.airdrop(&fee_payer.pubkey(), 10_000_000_000).unwrap();

    // 1. Not Enough Accounts
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new(counter_pda, false),
        ],
        data: vec![0, bump],
    };
    let tx = Transaction::new(
        &[&authority],
        Message::new(&[ix], Some(&authority.pubkey())),
        svm.latest_blockhash(),
    );
    let err = svm.send_transaction(tx).unwrap_err();
    assert!(matches!(
        err.err,
        TransactionError::InstructionError(0, InstructionError::NotEnoughAccountKeys)
    ));

    // 2. Authority Not Signer
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(authority.pubkey(), false), // not signer in AccountMeta
            AccountMeta::new(counter_pda, false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: vec![0, bump],
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

    // 3. Counter Account Not Writable
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(counter_pda, false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: vec![0, bump],
    };
    let tx = Transaction::new(
        &[&authority],
        Message::new(&[ix], Some(&authority.pubkey())),
        svm.latest_blockhash(),
    );
    let err = svm.send_transaction(tx).unwrap_err();
    assert!(matches!(
        err.err,
        TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
    ));

    // 4. Invalid PDA Address (e.g. wrong seed / random pubkey)
    let random_pda = Pubkey::new_unique();
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new(random_pda, false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: vec![0, bump],
    };
    let tx = Transaction::new(
        &[&authority],
        Message::new(&[ix], Some(&authority.pubkey())),
        svm.latest_blockhash(),
    );
    let err = svm.send_transaction(tx).unwrap_err();
    assert!(matches!(
        err.err,
        TransactionError::InstructionError(0, InstructionError::InvalidArgument)
    ));

    // 5. Invalid Data (e.g. no bump)
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new(counter_pda, false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: vec![0], // no bump
    };
    let tx = Transaction::new(
        &[&authority],
        Message::new(&[ix], Some(&authority.pubkey())),
        svm.latest_blockhash(),
    );
    let err = svm.send_transaction(tx).unwrap_err();
    assert!(matches!(
        err.err,
        TransactionError::InstructionError(0, InstructionError::InvalidArgument)
    ));
}

#[test]
fn test_increment_decrement_security_checks() {
    let (mut svm, program_id, authority) = setup_svm();

    let (counter_pda, bump) =
        Pubkey::find_program_address(&[b"counter", authority.pubkey().as_ref()], &program_id);

    // Create a separate fee payer for the non-signer tests
    let fee_payer = Keypair::new();
    svm.airdrop(&fee_payer.pubkey(), 10_000_000_000).unwrap();

    // Initialize first
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new(counter_pda, false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: vec![0, bump],
    };
    let tx = Transaction::new(
        &[&authority],
        Message::new(&[ix], Some(&authority.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    // 1. Not Enough Accounts for Increment
    let ix = Instruction {
        program_id,
        accounts: vec![AccountMeta::new_readonly(authority.pubkey(), true)],
        data: vec![1, bump],
    };
    let tx = Transaction::new(
        &[&authority],
        Message::new(&[ix], Some(&authority.pubkey())),
        svm.latest_blockhash(),
    );
    let err = svm.send_transaction(tx).unwrap_err();
    assert!(matches!(
        err.err,
        TransactionError::InstructionError(0, InstructionError::NotEnoughAccountKeys)
    ));

    // 2. Authority Not Signer for Increment
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(authority.pubkey(), false), // not signer in AccountMeta
            AccountMeta::new(counter_pda, false),
        ],
        data: vec![1, bump],
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

    // 3. Counter Account Not Writable for Increment
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new_readonly(counter_pda, false),
        ],
        data: vec![1, bump],
    };
    let tx = Transaction::new(
        &[&authority],
        Message::new(&[ix], Some(&authority.pubkey())),
        svm.latest_blockhash(),
    );
    let err = svm.send_transaction(tx).unwrap_err();
    assert!(matches!(
        err.err,
        TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
    ));

    // 4. Invalid PDA Address (mismatch __expected_pda)
    let random_pda = Pubkey::new_unique();
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new(random_pda, false),
        ],
        data: vec![1, bump],
    };
    let tx = Transaction::new(
        &[&authority],
        Message::new(&[ix], Some(&authority.pubkey())),
        svm.latest_blockhash(),
    );
    let err = svm.send_transaction(tx).unwrap_err();
    assert!(matches!(
        err.err,
        TransactionError::InstructionError(0, InstructionError::InvalidArgument)
    ));

    // 5. Authority Mismatch (stored authority in state doesn't match passed authority)
    let wrong_authority = Keypair::new();
    svm.airdrop(&wrong_authority.pubkey(), 1_000_000_000)
        .unwrap();

    let (wrong_counter_pda, wrong_bump) = Pubkey::find_program_address(
        &[b"counter", wrong_authority.pubkey().as_ref()],
        &program_id,
    );

    // Initialize for wrong authority
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(wrong_authority.pubkey(), true),
            AccountMeta::new(wrong_counter_pda, false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: vec![0, wrong_bump],
    };
    let tx = Transaction::new(
        &[&wrong_authority],
        Message::new(&[ix], Some(&wrong_authority.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    // Manually overwrite the authority stored in wrong_counter_pda's account state to authority.pubkey()
    let mut wrong_acc = svm.get_account(&wrong_counter_pda).unwrap();
    wrong_acc.data[0..32].copy_from_slice(&authority.pubkey().to_bytes());
    svm.set_account(wrong_counter_pda, wrong_acc).unwrap();

    // Call increment using wrong_authority as the authority account (PDA derivation will pass),
    // but the stored authority is authority.pubkey(), causing authority check mismatch.
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(wrong_authority.pubkey(), true),
            AccountMeta::new(wrong_counter_pda, false),
        ],
        data: vec![1, wrong_bump],
    };
    let tx = Transaction::new(
        &[&wrong_authority],
        Message::new(&[ix], Some(&wrong_authority.pubkey())),
        svm.latest_blockhash(),
    );
    let err = svm.send_transaction(tx).unwrap_err();
    assert!(matches!(
        err.err,
        TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
    ));
}

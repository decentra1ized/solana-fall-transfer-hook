#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        AccountDeserialize, InstructionData, ToAccountMetas,
        solana_program::instruction::Instruction,
        system_program::ID as SYSTEM_PROGRAM_ID,
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

use helpers::{setup, initialize_mint, rate_limit_pda};

#[test]
fn test_initialize() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    initialize_mint(&mut svm, &payer, &mint, &program_id);

    let rate_limit = rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id);

    let instruction = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }.to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = solana_message::Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg), &[&payer],
    ).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Initialization failed: {:?}", res.err());

    // Challenge 2: the account records the mint it belongs to.
    let account = svm.get_account(&rate_limit).expect("rate limit account should exist");
    let state = solana_fall_transfer_hook::RateLimit::try_deserialize(
        &mut account.data.as_slice(),
    ).expect("rate limit account should deserialize");

    assert_eq!(state.authority, payer.pubkey());
    assert_eq!(state.mint, mint.pubkey());
    assert_eq!(state.max_amount, solana_fall_transfer_hook::RateLimit::MAX_AMOUNT);
    assert_eq!(state.amount_transferred, 0);
}

/// Challenge 1: `initialize` must refuse a mint that is not a Token-2022 mint.
#[test]
fn test_initialize_rejects_non_token_2022_mint() {
    use anchor_lang::Id;
    use anchor_spl::token::Token;

    let (mut svm, payer, program_id) = setup();

    // A legacy SPL Token mint, not a Token-2022 mint.
    let mint = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(82);
    let create_ix = anchor_lang::solana_program::system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        rent,
        82,
        &Token::id(),
    );
    let init_mint_ix = anchor_spl::token::spl_token::instruction::initialize_mint(
        &Token::id(),
        &mint.pubkey(),
        &payer.pubkey(),
        None,
        9,
    ).unwrap();

    let blockhash = svm.latest_blockhash();
    let msg = solana_message::Message::new_with_blockhash(
        &[create_ix, init_mint_ix], Some(&payer.pubkey()), &blockhash,
    );
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg), &[&payer, &mint],
    ).unwrap();
    svm.send_transaction(tx).expect("legacy mint creation should succeed");

    let rate_limit = rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id);

    let instruction = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }.to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = solana_message::Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg), &[&payer],
    ).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Initialize should reject a non-Token-2022 mint");

    let logs = format!("{:?}", res.err());
    assert!(
        logs.contains("0x1772"),
        "expected InvalidMint (0x1772), got: {}", logs,
    );
}

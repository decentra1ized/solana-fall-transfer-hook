#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        AccountDeserialize, InstructionData, ToAccountMetas,
        solana_program::{
            instruction::Instruction,
            program_option::COption,
            program_pack::Pack,
        },
        system_program::ID as SYSTEM_PROGRAM_ID,
    },
    anchor_spl::token::spl_token,
    solana_account::Account,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

use helpers::{setup, initialize_mint};

#[test]
fn test_initialize() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // First create the mint via the dedicated instruction
    initialize_mint(&mut svm, &payer, &mint, &program_id);

    // Then initialize the rate limit account
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit"],
        &program_id,
    ).0;

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

    let account = svm.get_account(&rate_limit).unwrap();
    let state = solana_fall_transfer_hook::RateLimit::try_deserialize(
        &mut account.data.as_slice(),
    )
    .unwrap();
    assert_eq!(state.authority, payer.pubkey());
    assert_eq!(state.mint, mint.pubkey());
}

#[test]
fn test_initialize_rejects_legacy_token_mint() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    let mint_state = spl_token::state::Mint {
        mint_authority: COption::Some(payer.pubkey()),
        supply: 0,
        decimals: 9,
        is_initialized: true,
        freeze_authority: COption::None,
    };
    let mut mint_data = vec![0; spl_token::state::Mint::LEN];
    spl_token::state::Mint::pack(mint_state, &mut mint_data).unwrap();
    svm.set_account(
        mint.pubkey(),
        Account {
            lamports: 1_000_000,
            data: mint_data,
            owner: spl_token::ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let rate_limit = Pubkey::find_program_address(&[b"rate_limit"], &program_id).0;
    let instruction = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    let blockhash = svm.latest_blockhash();
    let message = solana_message::Message::new_with_blockhash(
        &[instruction],
        Some(&payer.pubkey()),
        &blockhash,
    );
    let transaction = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(message),
        &[&payer],
    )
    .unwrap();

    let error = svm
        .send_transaction(transaction)
        .expect_err("Legacy Token mint should be rejected");
    assert!(
        error.meta.logs.iter().any(|log| log.contains("InvalidMint")),
        "Expected InvalidMint error, got logs: {:?}",
        error.meta.logs,
    );
}

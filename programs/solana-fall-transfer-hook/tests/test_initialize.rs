#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        solana_program::instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID,
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{initialize_mint, setup};

/// Confirms that a RateLimit account can be created for a particular
/// Token-2022 mint and owner.
///
/// This test also reads the finished account and proves that the mint's
/// address was saved correctly.
#[test]
fn test_initialize() {
    // Create a small local Solana blockchain and a temporary payer.
    let (mut svm, payer, program_id) = setup();

    // Create a new keypair that will become our Token-2022 mint.
    let mint = Keypair::new();

    // The mint must exist before we create its RateLimit account.
    initialize_mint(&mut svm, &payer, &mint, &program_id);

    let payer_key = payer.pubkey();
    let mint_key = mint.pubkey();

    // Derive the unique RateLimit PDA for this mint and owner.
    //
    // These seeds must match the program:
    // ["rate_limit", mint, owner]
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint_key.as_ref(), payer_key.as_ref()],
        &program_id,
    )
    .0;

    // Build the instruction that initializes the RateLimit PDA.
    let instruction = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: payer_key,
            mint: mint_key,
            rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    );

    // Place the instruction inside a signed Solana transaction.
    let blockhash = svm.latest_blockhash();

    let message = Message::new_with_blockhash(&[instruction], Some(&payer_key), &blockhash);

    let transaction =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[&payer]).unwrap();

    // The transaction should succeed because this is a real Token-2022 mint.
    let result = svm.send_transaction(transaction);

    assert!(
        result.is_ok(),
        "RateLimit initialization failed: {:?}",
        result.err()
    );

    // Load the newly created RateLimit account from LiteSVM.
    let stored_account = svm
        .get_account(&rate_limit)
        .expect("RateLimit account should exist");

    // Convert its raw account bytes back into our RateLimit Rust structure.
    let mut account_data = stored_account.data.as_slice();

    let stored_rate_limit =
        solana_fall_transfer_hook::RateLimit::try_deserialize(&mut account_data)
            .expect("RateLimit account should deserialize");

    // Prove that Challenge 2 works: the account remembers its mint.
    assert_eq!(
        stored_rate_limit.mint, mint_key,
        "RateLimit should store the correct Token-2022 mint"
    );

    // Prove that this RateLimit account belongs to the correct owner.
    assert_eq!(
        stored_rate_limit.authority, payer_key,
        "RateLimit should store the correct authority"
    );
}

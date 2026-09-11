#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    build_program_transfer_with_hook_ix, create_ata, mint_tokens, setup,
    setup_mint_and_extra_metas,
};

fn send_program_transfer(
    svm: &mut litesvm::LiteSVM,
    owner: &Keypair,
    instruction: anchor_lang::solana_program::instruction::Instruction,
) -> Result<(), litesvm::types::FailedTransactionMetadata> {
    let blockhash = svm.latest_blockhash();
    let message = Message::new_with_blockhash(&[instruction], Some(&owner.pubkey()), &blockhash);
    let transaction =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[owner]).unwrap();
    svm.send_transaction(transaction).map(|_| ())
}

#[test]
fn test_transfer_through_token_mover() {
    let (mut svm, payer, hook_program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    let source = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let destination = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source, 100);

    let instruction = build_program_transfer_with_hook_ix(
        &source,
        &destination,
        &mint.pubkey(),
        &payer.pubkey(),
        &hook_program_id,
        100,
        9,
    );

    let result = send_program_transfer(&mut svm, &payer, instruction);
    assert!(result.is_ok(), "CPI transfer failed: {:?}", result.err());
}

#[test]
fn test_token_mover_still_enforces_rate_limit() {
    let (mut svm, payer, hook_program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    let source = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let destination = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source, 1_000_001);

    let at_limit = build_program_transfer_with_hook_ix(
        &source,
        &destination,
        &mint.pubkey(),
        &payer.pubkey(),
        &hook_program_id,
        1_000_000,
        9,
    );
    let first_result = send_program_transfer(&mut svm, &payer, at_limit);
    assert!(
        first_result.is_ok(),
        "Transfer at limit failed: {:?}",
        first_result.err()
    );

    let over_limit = build_program_transfer_with_hook_ix(
        &source,
        &destination,
        &mint.pubkey(),
        &payer.pubkey(),
        &hook_program_id,
        1,
        9,
    );
    let second_result = send_program_transfer(&mut svm, &payer, over_limit);
    let error = second_result.expect_err("Transfer above limit should fail");
    assert!(
        format!("{error:?}").contains("Custom(6001)"),
        "Expected RateLimitExceeded (0x1771), got: {error:?}"
    );
}

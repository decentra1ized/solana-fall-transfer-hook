#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    build_program_transfer_with_hook_ix, create_ata, mint_tokens, send_ix, setup,
    setup_mint_and_extra_metas,
};

#[test]
fn test_transfer_through_program() {
    let (mut svm, owner, hook_program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &owner, &mint, &hook_program_id);

    let recipient = Keypair::new();
    let source = create_ata(&mut svm, &owner, &owner.pubkey(), &mint.pubkey());
    let destination = create_ata(&mut svm, &owner, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &owner, &mint.pubkey(), &source, 100);

    let transfer = build_program_transfer_with_hook_ix(
        &source,
        &destination,
        &mint.pubkey(),
        &owner.pubkey(),
        &hook_program_id,
        100,
    );

    send_ix(&mut svm, transfer, &owner, &[&owner]);
}

#[test]
fn test_program_transfer_enforces_rate_limit() {
    let (mut svm, owner, hook_program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &owner, &mint, &hook_program_id);

    let recipient = Keypair::new();
    let source = create_ata(&mut svm, &owner, &owner.pubkey(), &mint.pubkey());
    let destination = create_ata(&mut svm, &owner, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &owner, &mint.pubkey(), &source, 1_000_001);

    let at_limit = build_program_transfer_with_hook_ix(
        &source,
        &destination,
        &mint.pubkey(),
        &owner.pubkey(),
        &hook_program_id,
        1_000_000,
    );
    send_ix(&mut svm, at_limit, &owner, &[&owner]);

    let over_limit = build_program_transfer_with_hook_ix(
        &source,
        &destination,
        &mint.pubkey(),
        &owner.pubkey(),
        &hook_program_id,
        1,
    );
    let blockhash = svm.latest_blockhash();
    let message = Message::new_with_blockhash(&[over_limit], Some(&owner.pubkey()), &blockhash);
    let transaction =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[&owner]).unwrap();

    let error = svm
        .send_transaction(transaction)
        .expect_err("The transfer hook should reject a CPI transfer over the limit");
    assert!(
        error
            .meta
            .logs
            .iter()
            .any(|log| log.contains("Error Code: RateLimitExceeded")),
        "Expected RateLimitExceeded (0x1771), got: {:?}",
        error.err
    );
}

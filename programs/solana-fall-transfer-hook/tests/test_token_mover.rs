//! Challenge 4: move tokens from inside a program.
//!
//! The transfer instruction lives in the separate `token-mover` program, so the
//! call chain is token-mover -> Token-2022 -> transfer hook. Putting it in the
//! hook program itself would make Token-2022 re-enter a program that is already
//! on the call stack, which the runtime rejects.

#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_signer::Signer,
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens, build_mover_transfer_ix,
    try_send_ix,
};

#[test]
fn test_transfer_through_program() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let ix = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 100,
    );

    let res = try_send_ix(&mut svm, ix, &payer, &[&payer]);
    assert!(res.is_ok(), "Transfer through token-mover failed: {:?}", res.err());
}

#[test]
fn test_transfer_through_program_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    let ix1 = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000,
    );
    assert!(
        try_send_ix(&mut svm, ix1, &payer, &[&payer]).is_ok(),
        "Transfer at the limit should succeed",
    );

    let ix2 = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1,
    );
    let err = try_send_ix(&mut svm, ix2, &payer, &[&payer])
        .expect_err("One more base unit must exceed the rate limit");
    assert!(err.contains("0x1771"), "expected RateLimitExceeded (0x1771), got: {}", err);
}

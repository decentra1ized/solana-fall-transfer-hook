#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens, build_transfer_with_hook_ix,
    initialize_mint, initialize_rate_limit, initialize_extra_account_metas, try_send_ix,
};

#[test]
fn test_transfer_hook() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    let mint_amount = 1_000_000u64;
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, mint_amount);

    let transfer_ix = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 100, 9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[transfer_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer with hook failed: {:?}", res.err());
}

#[test]
fn test_transfer_hook_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    let ix1 = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer at limit should succeed: {:?}", res.err());

    let ix2 = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Transfer exceeding rate limit should fail");
}

/// Challenge 3: two owners each spend the full 1,000,000 cap inside the same
/// hour. With a program-wide rate limit the second wallet would be blocked.
/// With a per-(mint, owner) rate limit both succeed.
#[test]
fn test_rate_limit_is_per_owner() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    initialize_mint(&mut svm, &payer, &mint, &program_id);
    initialize_extra_account_metas(&mut svm, &payer, &mint, &program_id);

    let alice = Keypair::new();
    let bob = Keypair::new();
    let recipient = Keypair::new();
    for wallet in [&alice, &bob, &recipient] {
        svm.airdrop(&wallet.pubkey(), 1_000_000_000).unwrap();
    }

    // Each owner creates their own rate limit account for this mint.
    initialize_rate_limit(&mut svm, &alice, &mint, &program_id);
    initialize_rate_limit(&mut svm, &bob, &mint, &program_id);

    let alice_ata = create_ata(&mut svm, &payer, &alice.pubkey(), &mint.pubkey());
    let bob_ata = create_ata(&mut svm, &payer, &bob.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    // Alice gets one extra base unit so the final check fails on the rate
    // limit rather than on an empty balance.
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &alice_ata, 1_000_001);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &bob_ata, 1_000_000);

    let alice_ix = build_transfer_with_hook_ix(
        &alice_ata, &dest_ata, &mint.pubkey(), &alice.pubkey(), &program_id, 1_000_000, 9,
    );
    assert!(
        try_send_ix(&mut svm, alice_ix, &alice, &[&alice]).is_ok(),
        "Alice should be able to send her full 1,000,000 allowance",
    );

    let bob_ix = build_transfer_with_hook_ix(
        &bob_ata, &dest_ata, &mint.pubkey(), &bob.pubkey(), &program_id, 1_000_000, 9,
    );
    assert!(
        try_send_ix(&mut svm, bob_ix, &bob, &[&bob]).is_ok(),
        "Bob has his own rate limit, so he should also send 1,000,000 in the same hour",
    );

    // Each owner is still capped individually.
    let alice_over_ix = build_transfer_with_hook_ix(
        &alice_ata, &dest_ata, &mint.pubkey(), &alice.pubkey(), &program_id, 1, 9,
    );
    let err = try_send_ix(&mut svm, alice_over_ix, &alice, &[&alice])
        .expect_err("Alice is at her cap, so one more base unit must fail");
    assert!(err.contains("0x1771"), "expected RateLimitExceeded (0x1771), got: {}", err);
}

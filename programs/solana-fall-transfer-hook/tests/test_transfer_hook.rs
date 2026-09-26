#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, initialize_rate_limit, create_ata, mint_tokens,
    build_transfer_with_hook_ix,
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

    // Mint more than the rate limit so we have enough tokens
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // First transfer: exactly at the limit - should succeed
    let ix1 = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer at limit should succeed: {:?}", res.err());

    // Second transfer: 1 token more - should fail with RateLimitExceeded
    let ix2 = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Transfer exceeding rate limit should fail");
}

// Proof that the rate limit is per-owner and not program-wide: two wallets each
// move the full 1_000_000 limit within the same window. Against a single global
// bucket the second transfer would have blown the limit and failed.
#[test]
fn test_rate_limit_is_per_owner() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // Creates the mint, the extra account metas, and `payer`'s rate limit.
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // A second wallet, with its own SOL and - crucially - its own rate limit
    // account. Without this `initialize` call the hook would fail with
    // AccountNotInitialized, because nothing lives at its PDA yet.
    let other = Keypair::new();
    svm.airdrop(&other.pubkey(), 1_000_000_000).unwrap();
    initialize_rate_limit(&mut svm, &other, &mint, &program_id);

    let recipient = Keypair::new();
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let other_ata = create_ata(&mut svm, &payer, &other.pubkey(), &mint.pubkey());

    // Each wallet holds exactly the per-owner limit.
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_ata, 1_000_000);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &other_ata, 1_000_000);

    // No clock advance between the two sends, so both land in the same window.
    for (label, owner, source_ata) in [
        ("first wallet", &payer, payer_ata),
        ("second wallet", &other, other_ata),
    ] {
        let ix = build_transfer_with_hook_ix(
            &source_ata, &dest_ata, &mint.pubkey(), &owner.pubkey(), &program_id, 1_000_000, 9,
        );
        let blockhash = svm.latest_blockhash();
        let msg = Message::new_with_blockhash(&[ix], Some(&owner.pubkey()), &blockhash);
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[owner]).unwrap();

        let res = svm.send_transaction(tx);
        assert!(
            res.is_ok(),
            "{label} should be able to send its full limit in the same window: {:?}",
            res.err(),
        );
    }
}

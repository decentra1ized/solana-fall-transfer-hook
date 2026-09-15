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
    rate_limit_pda, read_rate_limit,
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

/// Challenge 3: the rate limit is per (mint, owner), not program-wide.
///
/// This is the test that actually proves the seed change. Every other test
/// transfers as a single owner and would pass unchanged against the original
/// global bucket. Here one owner exhausts their entire allowance and a second
/// owner must still be able to transfer the full amount.
#[test]
fn test_rate_limit_is_per_owner() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // Sets up the mint, the extra account metas, and the payer's own bucket.
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // A second holder, who initializes their own bucket for this mint.
    let other = Keypair::new();
    svm.airdrop(&other.pubkey(), 1_000_000_000).unwrap();
    helpers::initialize_rate_limit(&mut svm, &other, &mint, &program_id);

    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let other_ata = create_ata(&mut svm, &payer, &other.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_ata, 2_000_000);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &other_ata, 2_000_000);

    // The payer spends their whole allowance.
    let ix = build_transfer_with_hook_ix(
        &payer_ata, &other_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    assert!(svm.send_transaction(tx).is_ok(), "payer's first transfer should succeed");

    // ...and is now cut off.
    let ix = build_transfer_with_hook_ix(
        &payer_ata, &other_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    assert!(svm.send_transaction(tx).is_err(), "payer should be rate limited");

    // The other holder is unaffected: with a single global bucket this would
    // fail, because the payer already consumed the whole 1_000_000.
    let ix = build_transfer_with_hook_ix(
        &other_ata, &payer_ata, &mint.pubkey(), &other.pubkey(), &program_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&other.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&other]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "a second owner must have their own bucket: {:?}",
        res.err(),
    );

    // Confirm the two buckets are genuinely separate accounts, each holding
    // only its own owner's usage.
    let payer_bucket = rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id);
    let other_bucket = rate_limit_pda(&mint.pubkey(), &other.pubkey(), &program_id);
    assert_ne!(payer_bucket, other_bucket, "each owner needs a distinct PDA");
    assert_eq!(read_rate_limit(&svm, &payer_bucket).amount_transferred, 1_000_000);
    assert_eq!(read_rate_limit(&svm, &other_bucket).amount_transferred, 1_000_000);
}

/// Challenge 3 follow-on: because the bucket is per (mint, owner), a holder
/// who has never called `initialize` has no rate limit account, and the hook
/// cannot resolve one - so their transfers are refused rather than silently
/// unlimited.
#[test]
fn test_transfer_fails_without_an_initialized_rate_limit() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // This holder never calls `initialize`.
    let stranger = Keypair::new();
    svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();

    let stranger_ata = create_ata(&mut svm, &payer, &stranger.pubkey(), &mint.pubkey());
    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &stranger_ata, 1_000);

    let ix = build_transfer_with_hook_ix(
        &stranger_ata, &payer_ata, &mint.pubkey(), &stranger.pubkey(), &program_id, 1, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&stranger.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&stranger]).unwrap();

    assert!(
        svm.send_transaction(tx).is_err(),
        "a holder with no rate limit account must not be able to transfer",
    );
}

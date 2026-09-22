#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::AccountDeserialize,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use solana_fall_transfer_hook::RateLimit;

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens, build_transfer_with_hook_ix, initialize_rate_limit, send_ix,
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

/// Two owners, same mint, same window, each sending the full cap.
/// Under the old global bucket the second transfer would be rejected.
#[test]
fn test_rate_limit_is_per_owner() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // Creates the mint, the payer's rate limit, and the extra account metas.
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // A second wallet, with its own funds and its own rate limit account.
    let second = Keypair::new();
    svm.airdrop(&second.pubkey(), 1_000_000_000).unwrap();
    initialize_rate_limit(&mut svm, &second, &mint, &program_id);

    let recipient = Keypair::new();
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let second_ata = create_ata(&mut svm, &payer, &second.pubkey(), &mint.pubkey());

    // Each wallet holds exactly the cap.
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_ata, RateLimit::MAX_AMOUNT);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &second_ata, RateLimit::MAX_AMOUNT);

    // The payer spends its entire budget.
    let ix = build_transfer_with_hook_ix(
        &payer_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, RateLimit::MAX_AMOUNT, 9,
    );
    send_ix(&mut svm, ix, &payer, &[&payer]);

    // Same mint, same window, different owner: must still go through.
    let ix = build_transfer_with_hook_ix(
        &second_ata, &dest_ata, &mint.pubkey(), &second.pubkey(), &program_id, RateLimit::MAX_AMOUNT, 9,
    );
    send_ix(&mut svm, ix, &second, &[&second]);

    // And they really are two separate buckets, each with its own total.
    let payer_rl = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()], &program_id).0;
    let second_rl = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), second.pubkey().as_ref()], &program_id).0;

    assert_ne!(payer_rl, second_rl, "each owner should get a distinct rate limit PDA");

    for (label, addr) in [("payer", payer_rl), ("second", second_rl)] {
        let account = svm.get_account(&addr).expect("rate limit should exist");
        let state = RateLimit::try_deserialize(&mut account.data.as_slice()).unwrap();
        assert_eq!(state.amount_transferred, RateLimit::MAX_AMOUNT, "{label} bucket");
        assert_eq!(state.mint, mint.pubkey(), "{label} bucket");
    }
}

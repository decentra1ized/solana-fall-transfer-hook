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
    initialize_rate_limit,
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

/// The point of challenge 3.
///
/// Two holders of the same mint each send the full cap inside the same window.
/// Against a single program-wide `[b"rate_limit"]` account the first sender
/// would consume the cap and the second would be refused — one holder denying
/// service to every other. Scoped per (mint, owner), both succeed.
///
/// Note this test cannot pass by accident: it is the only one with two owners,
/// so it is the only one that distinguishes a shared limit from a scoped one.
#[test]
fn test_rate_limits_are_isolated_per_owner() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // `setup_mint_and_extra_metas` creates the mint, the extra-account-meta list,
    // and the payer's own rate limit.
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // A second holder, with their own rate limit account. After challenge 3 this
    // call is mandatory: every owner must initialize before their first transfer.
    let second = Keypair::new();
    svm.airdrop(&second.pubkey(), 10_000_000_000).unwrap();
    initialize_rate_limit(&mut svm, &second, &mint, &second.pubkey(), &program_id);

    let recipient = Keypair::new();
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let second_ata = create_ata(&mut svm, &payer, &second.pubkey(), &mint.pubkey());

    // Each holder starts with exactly the cap.
    let cap = 1_000_000u64;
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_ata, cap);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &second_ata, cap);

    // Holder one spends the whole cap.
    let ix1 = build_transfer_with_hook_ix(
        &payer_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, cap, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    assert!(
        svm.send_transaction(tx).is_ok(),
        "first holder should be able to spend the full cap"
    );

    // Holder two, same mint, same window, also spends the whole cap.
    // This is the assertion that fails without per-owner seeds.
    let ix2 = build_transfer_with_hook_ix(
        &second_ata, &dest_ata, &mint.pubkey(), &second.pubkey(), &program_id, cap, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&second.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&second]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "second holder must have their own limit, not share the first's: {:?}",
        res.err()
    );
}

/// A sponsored rate limit: one account pays the rent, a different account holds
/// the tokens.
///
/// `Initialize` seeds the rate limit from `owner`, while `transfer_hook` derives
/// it from the transfer's `owner`. Seeding from `payer` instead makes the two
/// agree only in the case where payer == owner, which every other test in this
/// file happens to satisfy — so this is the only test that can tell the two
/// derivations apart. Against the payer-seeded version the account is created at
/// an address the hook never resolves, and the transfer fails on a missing
/// account rather than on anything that names the real cause.
#[test]
fn test_rate_limit_can_be_sponsored_by_a_third_party() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // The holder never pays for anything: no airdrop, no lamports of their own.
    let holder = Keypair::new();
    initialize_rate_limit(&mut svm, &payer, &mint, &holder.pubkey(), &program_id);

    let holder_ata = create_ata(&mut svm, &payer, &holder.pubkey(), &mint.pubkey());
    let recipient = Keypair::new();
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &holder_ata, 1_000);

    // The holder signs the transfer; the payer still funds the transaction.
    let ix = build_transfer_with_hook_ix(
        &holder_ata, &dest_ata, &mint.pubkey(), &holder.pubkey(), &program_id, 100, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg), &[&payer, &holder],
    ).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "a sponsored rate limit must be found by the hook: {:?}",
        res.err()
    );
}

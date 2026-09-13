//! Tests for the in-program `transfer` instruction (challenge 4) and for the
//! per-mint/per-owner rate limit buckets (challenge 3).

#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    solana_instruction_error::InstructionError,
    solana_transaction_error::TransactionError,
};

use helpers::{
    setup, setup_mint_and_extra_metas, initialize_mint, initialize_rate_limit,
    initialize_extra_account_metas, create_ata, mint_tokens, send_ix,
    build_program_transfer_ix, build_transfer_with_hook_ix, rate_limit_pda,
    rate_limit_amount_transferred, token_balance,
};

/// The in-program transfer refuses to run when this program is its own mint's hook.
///
/// This is the re-entrancy case from challenge 4, and the failure is the *correct*
/// outcome rather than a defect. The call chain would be
///
///   this program (depth 1) -> Token-2022 (depth 2) -> this program's hook (depth 3)
///
/// and Solana forbids re-entering a program that is already on the invocation stack.
///
/// What matters is that it fails **loudly**, with `ReentrancyNotAllowed`, instead of
/// quietly transferring without running the hook. A transfer that skipped its own rate
/// limit would be far worse than one that refuses to execute, so this test pins the
/// specific error rather than just asserting `is_err()`.
#[test]
fn test_program_transfer_rejects_self_reentrancy() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let rate_limit = rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id);
    let before = rate_limit_amount_transferred(&svm, &rate_limit);

    let ix = build_program_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let err = svm.send_transaction(tx).expect_err(
        "transferring a mint whose hook is this program must not succeed",
    );
    assert_eq!(
        err.err,
        TransactionError::InstructionError(0, InstructionError::ReentrancyNotAllowed),
        "expected the runtime's re-entrancy refusal, got {:?}",
        err.err,
    );

    // Nothing moved and nothing was recorded: the whole transaction rolled back.
    assert_eq!(
        rate_limit_amount_transferred(&svm, &rate_limit), before,
        "a rejected transfer must not charge the rate limit"
    );
    assert_eq!(
        token_balance(&svm, &dest_ata), 0,
        "no tokens should have reached the destination"
    );
}

/// The hook still runs — and still rate limits — on the normal top-level path.
///
/// This is the counterpart to the test above: it shows the rate limit itself is
/// intact, and that the re-entrancy refusal is specifically about *who initiates*
/// the transfer, not about the hook being broken.
#[test]
fn test_top_level_transfer_still_runs_the_hook() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let rate_limit = rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id);

    // Token-2022 is at depth 1 here, so the hook runs at depth 2 and no re-entrancy
    // arises.
    let ix = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000, 9,
    );
    send_ix(&mut svm, ix, &payer, &[&payer]);

    assert_eq!(
        rate_limit_amount_transferred(&svm, &rate_limit), 1_000,
        "the hook should have charged the transfer against this owner's window"
    );
    assert_eq!(
        token_balance(&svm, &dest_ata), 1_000,
        "the tokens should have arrived"
    );
}

/// Two owners of the same mint get independent buckets.
///
/// This is the point of challenge 3. Under the original single `"rate_limit"` seed
/// this test fails: the first owner exhausting the window would block the second.
#[test]
fn test_rate_limit_is_per_owner() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // `payer` creates the mint and the extra metas, and opens their own bucket.
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // A second holder with their own funded wallet and their own bucket.
    let other = Keypair::new();
    svm.airdrop(&other.pubkey(), 1_000_000_000).unwrap();
    initialize_rate_limit(&mut svm, &other, &mint, &program_id);

    let recipient = Keypair::new();
    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let other_ata = create_ata(&mut svm, &payer, &other.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_ata, 2_000_000);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &other_ata, 2_000_000);

    // The payer burns their entire window.
    let ix = build_transfer_with_hook_ix(
        &payer_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9,
    );
    send_ix(&mut svm, ix, &payer, &[&payer]);

    // The payer is now blocked...
    let blocked = build_transfer_with_hook_ix(
        &payer_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[blocked], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    assert!(svm.send_transaction(tx).is_err(), "payer should have exhausted their own window");

    // ...but the other holder is untouched. With one global bucket this fails.
    let ix_other = build_transfer_with_hook_ix(
        &other_ata, &dest_ata, &mint.pubkey(), &other.pubkey(), &program_id, 1_000_000, 9,
    );
    send_ix(&mut svm, ix_other, &other, &[&other]);

    // Each bucket recorded only its own owner's traffic.
    let payer_bucket = rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id);
    let other_bucket = rate_limit_pda(&mint.pubkey(), &other.pubkey(), &program_id);
    assert_ne!(payer_bucket, other_bucket, "each owner must get a distinct PDA");
    assert_eq!(rate_limit_amount_transferred(&svm, &payer_bucket), 1_000_000);
    assert_eq!(rate_limit_amount_transferred(&svm, &other_bucket), 1_000_000);
}

/// The same owner gets independent buckets for different mints.
#[test]
fn test_rate_limit_is_per_mint() {
    let (mut svm, payer, program_id) = setup();
    let mint_a = Keypair::new();
    let mint_b = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint_a, &program_id);
    setup_mint_and_extra_metas(&mut svm, &payer, &mint_b, &program_id);

    let recipient = Keypair::new();
    let src_a = create_ata(&mut svm, &payer, &payer.pubkey(), &mint_a.pubkey());
    let dst_a = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint_a.pubkey());
    let src_b = create_ata(&mut svm, &payer, &payer.pubkey(), &mint_b.pubkey());
    let dst_b = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint_b.pubkey());

    mint_tokens(&mut svm, &payer, &mint_a.pubkey(), &src_a, 2_000_000);
    mint_tokens(&mut svm, &payer, &mint_b.pubkey(), &src_b, 2_000_000);

    // Exhaust the window on mint A.
    let ix_a = build_transfer_with_hook_ix(
        &src_a, &dst_a, &mint_a.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9,
    );
    send_ix(&mut svm, ix_a, &payer, &[&payer]);

    let blocked = build_transfer_with_hook_ix(
        &src_a, &dst_a, &mint_a.pubkey(), &payer.pubkey(), &program_id, 1, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[blocked], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    assert!(svm.send_transaction(tx).is_err(), "mint A's window should be exhausted");

    // Mint B is a separate bucket for the same owner and is unaffected.
    let ix_b = build_transfer_with_hook_ix(
        &src_b, &dst_b, &mint_b.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9,
    );
    send_ix(&mut svm, ix_b, &payer, &[&payer]);
}

/// `initialize` must refuse a mint that is not owned by Token-2022 (challenge 1).
#[test]
fn test_initialize_rejects_non_token_2022_mint() {
    let (mut svm, payer, program_id) = setup();

    // A legacy SPL Token mint: a real mint, but owned by the wrong program, so it
    // can never carry a transfer hook.
    let legacy_mint = Keypair::new();
    helpers::setup_legacy_mint(&mut svm, &legacy_mint, &payer.pubkey());

    let rate_limit = rate_limit_pda(&legacy_mint.pubkey(), &payer.pubkey(), &program_id);
    let ix = helpers::build_initialize_ix(
        &payer.pubkey(), &legacy_mint.pubkey(), &rate_limit, &program_id,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    assert!(
        svm.send_transaction(tx).is_err(),
        "a rate limit must not be opened against a non-Token-2022 mint"
    );
}

/// A well-formed Token-2022 mint is still accepted, so the check above is not
/// simply rejecting everything.
#[test]
fn test_initialize_accepts_token_2022_mint() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    initialize_mint(&mut svm, &payer, &mint, &program_id);
    initialize_rate_limit(&mut svm, &payer, &mint, &program_id);
    initialize_extra_account_metas(&mut svm, &payer, &mint, &program_id);

    let rate_limit = rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id);
    assert!(
        svm.get_account(&rate_limit).is_some(),
        "the rate limit account should have been created"
    );
}

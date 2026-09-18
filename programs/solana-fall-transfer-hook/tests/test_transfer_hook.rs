#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_with_mover, setup_mint_and_extra_metas, create_ata, mint_tokens,
    build_transfer_with_hook_ix, build_mover_transfer_ix, initialize_rate_limit,
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

#[test]
fn test_two_users_each_send_max_amount() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let user2 = Keypair::new();
    svm.airdrop(&user2.pubkey(), 1_000_000_000).unwrap();

    // Initialize rate limit for user 2
    initialize_rate_limit(&mut svm, &payer, &mint, &user2.pubkey(), &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let user1_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let user2_ata = create_ata(&mut svm, &payer, &user2.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    // Mint 1,000,000 to user 1 and 1,000,000 to user 2
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &user1_ata, 1_000_000);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &user2_ata, 1_000_000);

    // User 1 sends full 1,000,000 budget
    let ix1 = build_transfer_with_hook_ix(
        &user1_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg1 = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx1 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg1), &[&payer]).unwrap();
    let res1 = svm.send_transaction(tx1);
    assert!(res1.is_ok(), "User 1 sending max amount should succeed: {:?}", res1.err());

    // User 2 sends full 1,000,000 budget in the same hour - should succeed with per-owner rate limit!
    let ix2 = build_transfer_with_hook_ix(
        &user2_ata, &dest_ata, &mint.pubkey(), &user2.pubkey(), &program_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg2 = Message::new_with_blockhash(&[ix2], Some(&user2.pubkey()), &blockhash);
    let tx2 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg2), &[&user2]).unwrap();
    let res2 = svm.send_transaction(tx2);
    assert!(res2.is_ok(), "User 2 sending max amount should also succeed: {:?}", res2.err());
}

#[test]
fn test_cpi_transfer_tokens_success() {
    let (mut svm, payer, hook_program_id, mover_program_id) = setup_with_mover();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    // CPI send of 100 succeeds
    let cpi_ix = build_mover_transfer_ix(
        &mover_program_id, &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &hook_program_id, 100, 9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[cpi_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "CPI transfer of 100 tokens should succeed: {:?}", res.err());
}

#[test]
fn test_cpi_transfer_tokens_rate_limit_exceeded() {
    let (mut svm, payer, hook_program_id, mover_program_id) = setup_with_mover();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // First CPI send: 1,000,000 tokens - succeeds
    let cpi_ix1 = build_mover_transfer_ix(
        &mover_program_id, &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &hook_program_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg1 = Message::new_with_blockhash(&[cpi_ix1], Some(&payer.pubkey()), &blockhash);
    let tx1 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg1), &[&payer]).unwrap();
    let res1 = svm.send_transaction(tx1);
    assert!(res1.is_ok(), "CPI transfer of 1,000,000 tokens should succeed: {:?}", res1.err());

    // Second CPI send: 1 token - fails with RateLimitExceeded (0x1771)
    let cpi_ix2 = build_mover_transfer_ix(
        &mover_program_id, &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &hook_program_id, 1, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg2 = Message::new_with_blockhash(&[cpi_ix2], Some(&payer.pubkey()), &blockhash);
    let tx2 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg2), &[&payer]).unwrap();
    let res2 = svm.send_transaction(tx2);
    assert!(res2.is_err(), "CPI transfer of 1 token exceeding limit should fail");
}

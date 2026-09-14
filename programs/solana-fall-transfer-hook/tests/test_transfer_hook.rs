#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        solana_program::instruction::Instruction, InstructionData, ToAccountMetas,
    },
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    assert_logs_contain, build_transfer_with_hook_ix, create_ata, extra_metas_pda, fetch_rate_limit,
    initialize_extra_account_metas, initialize_mint, initialize_rate_limit_for, mint_tokens,
    rate_limit_pda, setup, setup_mint_and_extra_metas, token_amount, try_send_ix,
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
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        100,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[transfer_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer with hook failed: {:?}", res.err());
    assert_eq!(token_amount(&svm, &source_ata), mint_amount - 100);
    assert_eq!(token_amount(&svm, &dest_ata), 100);
    assert_eq!(
        fetch_rate_limit(
            &svm,
            &rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id)
        )
        .amount_transferred,
        100
    );
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
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Transfer at limit should succeed: {:?}",
        res.err()
    );

    // Second transfer: 1 token more - should fail with RateLimitExceeded
    let ix2 = build_transfer_with_hook_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1,
        9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Transfer exceeding rate limit should fail");
}

#[test]
fn test_transfer_hook_independent_owners() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    let alice = Keypair::new();
    let recipient = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);
    initialize_rate_limit_for(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &alice.pubkey(),
        &program_id,
    );

    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let alice_ata = create_ata(&mut svm, &payer, &alice.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_ata, 2_000_000);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &alice_ata, 2_000_000);

    try_send_ix(
        &mut svm,
        build_transfer_with_hook_ix(
            &payer_ata,
            &dest_ata,
            &mint.pubkey(),
            &payer.pubkey(),
            &program_id,
            1_000_000,
            9,
        ),
        &payer,
        &[&payer],
    )
    .expect("payer filling their own bucket should succeed");

    try_send_ix(
        &mut svm,
        build_transfer_with_hook_ix(
            &alice_ata,
            &dest_ata,
            &mint.pubkey(),
            &alice.pubkey(),
            &program_id,
            100,
            9,
        ),
        &payer,
        &[&payer, &alice],
    )
    .expect("alice must have an independent bucket");

    try_send_ix(
        &mut svm,
        build_transfer_with_hook_ix(
            &payer_ata,
            &dest_ata,
            &mint.pubkey(),
            &payer.pubkey(),
            &program_id,
            1,
            9,
        ),
        &payer,
        &[&payer],
    )
    .expect_err("payer should still be capped after filling their bucket");

    assert_eq!(
        fetch_rate_limit(&svm, &rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id))
            .amount_transferred,
        1_000_000
    );
    assert_eq!(
        fetch_rate_limit(&svm, &rate_limit_pda(&mint.pubkey(), &alice.pubkey(), &program_id))
            .amount_transferred,
        100
    );
}

#[test]
fn test_transfer_hook_independent_mints() {
    let (mut svm, payer, program_id) = setup();
    let mint_a = Keypair::new();
    let mint_b = Keypair::new();
    let recipient = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint_a, &program_id);
    setup_mint_and_extra_metas(&mut svm, &payer, &mint_b, &program_id);

    let src_a = create_ata(&mut svm, &payer, &payer.pubkey(), &mint_a.pubkey());
    let dst_a = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint_a.pubkey());
    let src_b = create_ata(&mut svm, &payer, &payer.pubkey(), &mint_b.pubkey());
    let dst_b = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint_b.pubkey());

    mint_tokens(&mut svm, &payer, &mint_a.pubkey(), &src_a, 2_000_000);
    mint_tokens(&mut svm, &payer, &mint_b.pubkey(), &src_b, 2_000_000);

    try_send_ix(
        &mut svm,
        build_transfer_with_hook_ix(
            &src_a,
            &dst_a,
            &mint_a.pubkey(),
            &payer.pubkey(),
            &program_id,
            1_000_000,
            9,
        ),
        &payer,
        &[&payer],
    )
    .expect("filling mint A bucket should succeed");

    try_send_ix(
        &mut svm,
        build_transfer_with_hook_ix(
            &src_b,
            &dst_b,
            &mint_b.pubkey(),
            &payer.pubkey(),
            &program_id,
            250,
            9,
        ),
        &payer,
        &[&payer],
    )
    .expect("mint B must have an independent bucket");

    try_send_ix(
        &mut svm,
        build_transfer_with_hook_ix(
            &src_a,
            &dst_a,
            &mint_a.pubkey(),
            &payer.pubkey(),
            &program_id,
            1,
            9,
        ),
        &payer,
        &[&payer],
    )
    .expect_err("mint A should still be capped");
}

#[test]
fn test_transfer_hook_missing_rate_limit() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    let recipient = Keypair::new();

    initialize_mint(&mut svm, &payer, &mint, &program_id);
    initialize_extra_account_metas(&mut svm, &payer, &mint, &program_id);

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000);

    try_send_ix(
        &mut svm,
        build_transfer_with_hook_ix(
            &source_ata,
            &dest_ata,
            &mint.pubkey(),
            &payer.pubkey(),
            &program_id,
            100,
            9,
        ),
        &payer,
        &[&payer],
    )
    .expect_err("transfer without an initialized rate limit must fail");
}

#[test]
fn test_transfer_hook_direct_invoke_rejected() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    let recipient = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000);

    let ix = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::TransferHook { amount: 100 }.data(),
        solana_fall_transfer_hook::accounts::TransferHook {
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: dest_ata,
            owner: payer.pubkey(),
            extra_account_meta_list: extra_metas_pda(&mint.pubkey(), &program_id),
            rate_limit: rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id),
        }
        .to_account_metas(None),
    );

    let err = try_send_ix(&mut svm, ix, &payer, &[&payer])
        .expect_err("direct hook invoke must fail");
    assert_logs_contain(&err, "Transfer hook invoked outside of an active transfer");
    assert_eq!(token_amount(&svm, &source_ata), 1_000);
}

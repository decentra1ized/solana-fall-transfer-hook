#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens, send_ix, build_transfer_with_hook_ix, build_token_mover_ix, initialize_rate_limit
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
fn test_rate_limit_is_per_owner() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(
        &mut svm,
        &payer,
        &mint,
        &program_id,
    );

    let second_owner = Keypair::new();

    svm.airdrop(
        &second_owner.pubkey(),
        1_000_000_000,
    )
    .unwrap();

    initialize_rate_limit(
        &mut svm,
        &second_owner,
        &mint,
        &program_id,
    );

    let recipient = Keypair::new();

    svm.airdrop(
        &recipient.pubkey(),
        1_000_000_000,
    )
    .unwrap();

    let payer_ata = create_ata(
        &mut svm,
        &payer,
        &payer.pubkey(),
        &mint.pubkey(),
    );

    let second_owner_ata = create_ata(
        &mut svm,
        &payer,
        &second_owner.pubkey(),
        &mint.pubkey(),
    );

    let recipient_ata = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );

    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &payer_ata,
        1_000_000,
    );

    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &second_owner_ata,
        1_000_000,
    );

    //transfering at the exact limit
    let payer_transfer_ix = build_transfer_with_hook_ix(
        &payer_ata,
        &recipient_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        9,
    );

    let blockhash = svm.latest_blockhash();

    let msg = Message::new_with_blockhash(
        &[payer_transfer_ix],
        Some(&payer.pubkey()),
        &blockhash,
    );

    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&payer],
    )
    .unwrap();

    let res = svm.send_transaction(tx);

    assert!(
        res.is_ok(),
        "First owner's transfer should succeed: {:?}",
        res.err()
    );

    //2nd transfer within 1st hour which will suceed due to per owner pda
    let second_owner_transfer_ix = build_transfer_with_hook_ix(
        &second_owner_ata,
        &recipient_ata,
        &mint.pubkey(),
        &second_owner.pubkey(),
        &program_id,
        1_000_000,
        9,
    );

    let blockhash = svm.latest_blockhash();

    let msg = Message::new_with_blockhash(
        &[second_owner_transfer_ix],
        Some(&second_owner.pubkey()),
        &blockhash,
    );

    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&second_owner],
    )
    .unwrap();

    let res = svm.send_transaction(tx);

    assert!(
        res.is_ok(),
        "Second owner's independent rate-limit transfer should succeed: {:?}",
        res.err()
    );
}


#[test]
fn test_token_mover_within_limit() {

    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata,
        1_000_000u64
    );


    let transfer_ix = build_token_mover_ix(&source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &token_mover::id(), &program_id, 100);


    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[transfer_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer with hook failed: {:?}", res.err());

}


#[test]
fn test_token_mover_exceed_limit() {

    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);


    //transfering at exact limit
    let ix1 = build_token_mover_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),  &token_mover::id(), &program_id, 1_000_000,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer at limit should succeed: {:?}", res.err());

    //transfering one more token
    //must throw error RateLimitExceeded
    let ix2 = build_token_mover_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),  &token_mover::id(), &program_id, 1,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Transfer exceeding rate limit should fail");

}
#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{InstructionData, ToAccountMetas},
    anchor_lang::solana_program::instruction::Instruction,
    anchor_lang::solana_program::system_program,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, initialize_rate_limit, create_ata, mint_tokens, build_transfer_with_hook_ix,
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
fn test_transfer_hook_per_user_limit() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // payer gets its own rate limit account via setup
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // a second, independent owner for the same mint. The mint already
    // exists (created above), so build the Initialize instruction directly
    // instead of using initialize_rate_limit, which would try to recreate it.
    let user2 = Keypair::new();
    svm.airdrop(&user2.pubkey(), 1_000_000_000).unwrap();
    {
        let rate_limit_2 = Pubkey::find_program_address(
            &[b"rate_limit", mint.pubkey().as_ref(), user2.pubkey().as_ref()],
            &program_id,
        ).0;
        let ix = Instruction::new_with_bytes(
            program_id,
            &solana_fall_transfer_hook::instruction::Initialize {}.data(),
            solana_fall_transfer_hook::accounts::Initialize {
                payer: user2.pubkey(),
                mint: mint.pubkey(),
                rate_limit: rate_limit_2,
                system_program: system_program::ID,
            }.to_account_metas(None),
        );
        let blockhash = svm.latest_blockhash();
        let msg = Message::new_with_blockhash(&[ix], Some(&user2.pubkey()), &blockhash);
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user2]).unwrap();
        let res = svm.send_transaction(tx);
        assert!(res.is_ok(), "user2 initialize should succeed: {:?}", res.err());
    }

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata_1 = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let source_ata_2 = create_ata(&mut svm, &user2, &user2.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata_1, 2_000_000);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata_2, 2_000_000);

    // payer sends 1,000,000 - at their own limit
    let ix1 = build_transfer_with_hook_ix(
        &source_ata_1, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Payer transfer at their own limit should succeed: {:?}", res.err());

    // user2 sends 1,000,000 too - proves they have a SEPARATE rate limit, not shared
    let ix2 = build_transfer_with_hook_ix(
        &source_ata_2, &dest_ata, &mint.pubkey(), &user2.pubkey(), &program_id, 1_000_000, 9,
    );
    let blockhash2 = svm.latest_blockhash();
    let msg2 = Message::new_with_blockhash(&[ix2], Some(&user2.pubkey()), &blockhash2);
    let tx2 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg2), &[&user2]).unwrap();
    let res2 = svm.send_transaction(tx2);
    assert!(res2.is_ok(), "User2 transfer at their own limit should succeed: {:?}", res2.err());
}

#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
        solana_program::instruction::{AccountMeta, Instruction},
    },
    anchor_spl::token_2022::spl_token_2022,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
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

#[test]
fn test_rate_limit_is_per_user() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // Set up the mint, the first owner's rate limit, and the extra account meta list.
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // Create a second owner and fund it so it can pay for its own transactions.
    let second_owner = Keypair::new();
    svm.airdrop(&second_owner.pubkey(), 1_000_000_000).unwrap();

    // The second owner gets a separate rate-limit PDA for the same mint.
    initialize_rate_limit(&mut svm, &second_owner, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    // Create token accounts for both owners and one shared recipient.
    let source_ata_1 = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let source_ata_2 = create_ata(
        &mut svm,
        &second_owner,
        &second_owner.pubkey(),
        &mint.pubkey(),
    );
    let dest_ata = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );

    // Give both owners exactly one full rate-limit worth of tokens.
    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata_1,
        1_000_000,
    );
    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata_2,
        1_000_000,
    );

    // Owner 1 uses their entire limit.
    let ix1 = build_transfer_with_hook_ix(
        &source_ata_1,
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
        "first owner's transfer should succeed: {:?}",
        res.err()
    );

    // Owner 2 should still have their own untouched 1,000,000 limit.
    let ix2 = build_transfer_with_hook_ix(
        &source_ata_2,
        &dest_ata,
        &mint.pubkey(),
        &second_owner.pubkey(),
        &program_id,
        1_000_000,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[ix2],
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
        "second owner should have an independent rate limit: {:?}",
        res.err()
    );
}

#[test]
fn test_transfer_through_program() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(
        &mut svm,
        &payer,
        &payer.pubkey(),
        &mint.pubkey(),
    );

    let destination_ata = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );

    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata,
        1_000_000,
    );

    let rate_limit = Pubkey::find_program_address(
        &[
            b"rate_limit",
            mint.pubkey().as_ref(),
            payer.pubkey().as_ref(),
        ],
        &program_id,
    )
    .0;

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &program_id,
    )
    .0;

    let mut ix = Instruction::new_with_bytes(
        token_mover::id(),
        &token_mover::instruction::TransferWithHook {
            amount: 100,
        }
        .data(),
        token_mover::accounts::TransferWithHook {
            owner: payer.pubkey(),
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: destination_ata,
            token_program: spl_token_2022::id(),
        }
        .to_account_metas(None),
    );

    ix.accounts
        .push(AccountMeta::new_readonly(program_id, false));
    ix.accounts
        .push(AccountMeta::new_readonly(extra_account_meta_list, false));
    ix.accounts
        .push(AccountMeta::new(rate_limit, false));

    let blockhash = svm.latest_blockhash();

    let msg = Message::new_with_blockhash(
        &[ix],
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
        "transfer through token-mover should succeed: {:?}",
        res.err()
    );
}

#[test]
fn test_rate_limit_through_program() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(
        &mut svm,
        &payer,
        &payer.pubkey(),
        &mint.pubkey(),
    );

    let destination_ata = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );

    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata,
        2_000_000,
    );

    let rate_limit = Pubkey::find_program_address(
        &[
            b"rate_limit",
            mint.pubkey().as_ref(),
            payer.pubkey().as_ref(),
        ],
        &program_id,
    )
    .0;

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &program_id,
    )
    .0;

    let build_ix = |amount: u64| {
        let mut ix = Instruction::new_with_bytes(
            token_mover::id(),
            &token_mover::instruction::TransferWithHook { amount }.data(),
            token_mover::accounts::TransferWithHook {
                owner: payer.pubkey(),
                source_token: source_ata,
                mint: mint.pubkey(),
                destination_token: destination_ata,
                token_program: spl_token_2022::id(),
            }
            .to_account_metas(None),
        );

        ix.accounts
            .push(AccountMeta::new_readonly(program_id, false));
        ix.accounts
            .push(AccountMeta::new_readonly(extra_account_meta_list, false));
        ix.accounts
            .push(AccountMeta::new(rate_limit, false));

        ix
    };

    // First transfer reaches the full hourly limit.
    let ix = build_ix(1_000_000);

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[ix],
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
        "first transfer through token-mover should succeed: {:?}",
        res.err()
    );

    // One more token should be rejected by the hook.
    let ix = build_ix(1);

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[ix],
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
        res.is_err(),
        "second transfer should fail because the rate limit was exceeded"
    );
}
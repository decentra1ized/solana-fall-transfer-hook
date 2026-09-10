#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{InstructionData, ToAccountMetas},
    anchor_lang::solana_program::instruction::Instruction,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens,
    build_transfer_with_hook_ix, initialize_rate_limit,
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
fn test_transfer_hook_per_owner_rate_limit() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // Create a second owner and give them SOL
    let second_owner = Keypair::new();
    svm.airdrop(&second_owner.pubkey(), 1_000_000_000).unwrap();

    // Initialize a separate rate limit for the second owner
    initialize_rate_limit(
        &mut svm,
        &second_owner,
        &mint,
        &program_id,
    );

    // Create token accounts for both owners
    let payer_source = create_ata(
        &mut svm,
        &payer,
        &payer.pubkey(),
        &mint.pubkey(),
    );

    let second_owner_source = create_ata(
        &mut svm,
        &payer,
        &second_owner.pubkey(),
        &mint.pubkey(),
    );

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let payer_dest = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );

    // Give both owners enough tokens to transfer the full limit
    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &payer_source,
        1_000_000,
    );

    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &second_owner_source,
        1_000_000,
    );

    // Owner 1 transfers exactly 1,000,000 - should succeed
    let ix1 = build_transfer_with_hook_ix(
        &payer_source,
        &payer_dest,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[ix1],
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
        "Owner 1 transfer should succeed: {:?}",
        res.err()
    );

    // Owner 2 transfers exactly 1,000,000 - should ALSO succeed
    // because they have a separate rate limit account.
    let ix2 = build_transfer_with_hook_ix(
        &second_owner_source,
        &payer_dest,
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
        "Owner 2 transfer should succeed: {:?}",
        res.err()
    );
}

#[test]
fn test_transfer_through_token_mover() {
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

    let dest_ata = create_ata(
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
        1_000_101,
    );

    // Build the normal Token-2022 transfer instruction first.
    // We use it only to obtain the hook's remaining accounts.
    let hook_ix = build_transfer_with_hook_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        100,
        9,
    );

    // Build the instruction that calls the token-mover program.
    let mut transfer_ix = Instruction {
        program_id: token_mover::id(),
        accounts: token_mover::accounts::TransferWithHook {
            owner: payer.pubkey(),
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: dest_ata,
            token_program: anchor_spl::token_2022::ID,
        }
        .to_account_metas(None),
        data: token_mover::instruction::TransferWithHook { amount: 100 }.data(),
    };

    // The token-mover program needs the hook program and
    // the hook's extra accounts.
    transfer_ix.accounts.push(hook_ix.accounts[4].clone());
    transfer_ix.accounts.push(hook_ix.accounts[5].clone());
    transfer_ix.accounts.push(hook_ix.accounts[6].clone());

    let blockhash = svm.latest_blockhash();

    let msg = Message::new_with_blockhash(
        &[transfer_ix],
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
        "Transfer through token-mover should succeed: {:?}",
        res.err()
    );
}

#[test]
fn test_transfer_through_token_mover_rate_limit() {
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

    let dest_ata = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );

    // Mint enough tokens for both transfers.
    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata,
        1_000_001,
    );

    // First transfer: exactly at the rate limit.
    let hook_ix = build_transfer_with_hook_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        9,
    );

    let mut transfer_ix = Instruction {
        program_id: token_mover::id(),
        accounts: token_mover::accounts::TransferWithHook {
            owner: payer.pubkey(),
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: dest_ata,
            token_program: anchor_spl::token_2022::ID,
        }
        .to_account_metas(None),
        data: token_mover::instruction::TransferWithHook {
            amount: 1_000_000,
        }
        .data(),
    };

    // Hook program first, followed by the hook's extra accounts.
    transfer_ix.accounts.push(hook_ix.accounts[4].clone());
    transfer_ix.accounts.push(hook_ix.accounts[5].clone());
    transfer_ix.accounts.push(hook_ix.accounts[6].clone());

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[transfer_ix],
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
        "Transfer at limit through token-mover should succeed: {:?}",
        res.err()
    );

    // Second transfer: one more base unit.
    let hook_ix = build_transfer_with_hook_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1,
        9,
    );

    let mut transfer_ix = Instruction {
        program_id: token_mover::id(),
        accounts: token_mover::accounts::TransferWithHook {
            owner: payer.pubkey(),
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: dest_ata,
            token_program: anchor_spl::token_2022::ID,
        }
        .to_account_metas(None),
        data: token_mover::instruction::TransferWithHook { amount: 1 }.data(),
    };

    transfer_ix.accounts.push(hook_ix.accounts[4].clone());
    transfer_ix.accounts.push(hook_ix.accounts[5].clone());
    transfer_ix.accounts.push(hook_ix.accounts[6].clone());

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[transfer_ix],
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
        "Transfer exceeding rate limit through token-mover should fail"
    );
}
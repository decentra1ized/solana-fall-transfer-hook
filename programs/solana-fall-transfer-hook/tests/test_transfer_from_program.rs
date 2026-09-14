#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{Id, InstructionData, ToAccountMetas},
    solana_keypair::Keypair,
    solana_message::Message,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    token_mover,
};

use helpers::{setup, setup_mint_and_extra_metas, create_ata, mint_tokens, build_transfer_with_hook_ix};

#[test]
fn test_transfer_from_program_success() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);
    
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &Keypair::new().pubkey(), &mint.pubkey());
    
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let amount = 100;
    let decimals = 9;

    let ix = build_transfer_with_hook_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        amount,
        decimals,
    );

    let mut mover_ix = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        token_mover::id(),
        &token_mover::instruction::TransferWithHook { amount, decimals }.data(),
        token_mover::accounts::TransferWithHook {
            owner: payer.pubkey(),
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: dest_ata,
            token_program: anchor_spl::token_2022::Token2022::id(),
        }.to_account_metas(None),
    );

    for meta in ix.accounts.iter().skip(4) {
        mover_ix.accounts.push(meta.clone());
    }

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[mover_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg),
        &[&payer],
    ).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer from program failed: {:?}", res.err());
}

#[test]
fn test_transfer_from_program_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);
    
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &Keypair::new().pubkey(), &mint.pubkey());
    
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_001);

    let decimals = 9;

    let ix1 = build_transfer_with_hook_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        decimals,
    );

    let mut mover_ix1 = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        token_mover::id(),
        &token_mover::instruction::TransferWithHook { amount: 1_000_000, decimals }.data(),
        token_mover::accounts::TransferWithHook {
            owner: payer.pubkey(),
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: dest_ata,
            token_program: anchor_spl::token_2022::Token2022::id(),
        }.to_account_metas(None),
    );

    for meta in ix1.accounts.iter().skip(4) {
        mover_ix1.accounts.push(meta.clone());
    }

    let blockhash = svm.latest_blockhash();
    let msg1 = Message::new_with_blockhash(&[mover_ix1], Some(&payer.pubkey()), &blockhash);
    let tx1 = VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg1),
        &[&payer],
    ).unwrap();

    let res1 = svm.send_transaction(tx1);
    assert!(res1.is_ok(), "First transfer should succeed: {:?}", res1.err());

    let ix2 = build_transfer_with_hook_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1,
        decimals,
    );

    let mut mover_ix2 = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        token_mover::id(),
        &token_mover::instruction::TransferWithHook { amount: 1, decimals }.data(),
        token_mover::accounts::TransferWithHook {
            owner: payer.pubkey(),
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: dest_ata,
            token_program: anchor_spl::token_2022::Token2022::id(),
        }.to_account_metas(None),
    );

    for meta in ix2.accounts.iter().skip(4) {
        mover_ix2.accounts.push(meta.clone());
    }

    let msg2 = Message::new_with_blockhash(&[mover_ix2], Some(&payer.pubkey()), &blockhash);
    let tx2 = VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg2),
        &[&payer],
    ).unwrap();

    let res2 = svm.send_transaction(tx2);
    assert!(res2.is_err(), "Second transfer should fail");
}

#[allow(dead_code)]
mod helpers;
use anchor_lang::{prelude::*, InstructionData, ToAccountMetas,solana_program::instruction::{AccountMeta, Instruction}};
use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens, build_transfer_with_hook_ix,
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


fn build_token_mover_transfer_ix(
    source_ata: &solana_pubkey::Pubkey,
    dest_ata: &solana_pubkey::Pubkey,
    mint: &solana_pubkey::Pubkey,
    owner: &solana_pubkey::Pubkey,
    hook_program_id: &solana_pubkey::Pubkey,
    extra_account_meta_list: &solana_pubkey::Pubkey,
    rate_limit: &solana_pubkey::Pubkey,
    amount: u64,
) -> Instruction {
    let mut accounts = token_mover::accounts::TransferWithHook {
        owner: *owner,
        source_token: *source_ata,
        mint: *mint,
        destination_token: *dest_ata,
        token_program: anchor_spl::token_2022::Token2022::id(),
    }
    .to_account_metas(None);

    // The hook program must be first in remaining_accounts.
    accounts.push(AccountMeta::new_readonly(*hook_program_id, false));

    // Extra accounts required by the transfer hook.
    accounts.push(AccountMeta::new_readonly(
        *extra_account_meta_list,
        false,
    ));

    accounts.push(AccountMeta::new(*rate_limit, false));

    Instruction {
        program_id: token_mover::ID,
        accounts,
        data: token_mover::instruction::Transfer { amount }.data(),
    }
}

#[test]
fn test_transfer_through_token_mover() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());

    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000,);

    let extra_account_meta_list =
        solana_pubkey::Pubkey::find_program_address(
            &[b"extra-account-metas", mint.pubkey().as_ref()],
            &program_id,
        )
        .0;

    let rate_limit =
        solana_pubkey::Pubkey::find_program_address(
            &[ b"rate_limit",  mint.pubkey().as_ref(),  payer.pubkey().as_ref(),],
            &program_id,
        )
        .0;

    let ix = build_token_mover_transfer_ix( &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, &extra_account_meta_list,&rate_limit,100,);
    let blockhash = svm.latest_blockhash();

    let msg =
        Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash,);

    let tx =
        VersionedTransaction::try_new( VersionedMessage::Legacy(msg),&[&payer],).unwrap();

    let result = svm.send_transaction(tx);

    assert!(result.is_ok(), "Transfer through token-mover failed: {:?}",result.err());
}

#[test]
fn test_transfer_through_token_mover_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());

    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens( &mut svm,&payer,&mint.pubkey(), &source_ata,2_000_000,);

    let extra_account_meta_list =
        solana_pubkey::Pubkey::find_program_address(
            &[b"extra-account-metas", mint.pubkey().as_ref()],
            &program_id,
        ).0;

    let rate_limit =
        solana_pubkey::Pubkey::find_program_address(
            &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref(),],
            &program_id,
        )
        .0;

    // First transfer: 1,000,000 should succeed.
    let ix1 = build_token_mover_transfer_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        &extra_account_meta_list,
        &rate_limit,
        1_000_000,
    );

    let blockhash = svm.latest_blockhash();

    let msg =Message::new_with_blockhash(&[ix1],Some(&payer.pubkey()), &blockhash,);

    let tx =VersionedTransaction::try_new(VersionedMessage::Legacy(msg),&[&payer],).unwrap();

    let result = svm.send_transaction(tx);

    assert!(result.is_ok(),"First transfer should succeed: {:?}",result.err());

    // Second transfer: 1 more should hit RateLimitExceeded (0x1771).
    let ix2 = build_token_mover_transfer_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        &extra_account_meta_list,
        &rate_limit,
        1,
    );

    let blockhash = svm.latest_blockhash();

    let msg =Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash,);
    let tx =VersionedTransaction::try_new( VersionedMessage::Legacy(msg),&[&payer],).unwrap();
    let result = svm.send_transaction(tx);

    assert!(result.is_err(),"Second transfer should fail with RateLimitExceeded");
}
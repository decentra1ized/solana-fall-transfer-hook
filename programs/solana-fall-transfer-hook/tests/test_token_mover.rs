#![allow(dead_code)]
mod helpers;

use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use anchor_spl::token_2022::ID as TOKEN_2022_ID;

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens,
};

#[test]
fn test_cpi_transfer_success() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 10_000);

    let mut ix = Instruction {
        program_id: token_mover::id(),
        accounts: token_mover::accounts::TransferWithHook {
            owner: payer.pubkey(),
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: dest_ata,
            token_program: TOKEN_2022_ID,
        }
        .to_account_metas(None),
        data: token_mover::instruction::TransferWithHook { amount: 100 }.data(),
    };

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &program_id,
    ).0;
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        &program_id,
    ).0;

    ix.accounts.push(AccountMeta::new_readonly(program_id, false));
    ix.accounts.push(AccountMeta::new_readonly(extra_account_meta_list, false));
    ix.accounts.push(AccountMeta::new(rate_limit, false));

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "CPI transfer of 100 should succeed: {:?}", res.err());
}

#[test]
fn test_cpi_transfer_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &program_id,
    ).0;
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        &program_id,
    ).0;

    // First transfer: 1_000_000
    let mut ix1 = Instruction {
        program_id: token_mover::id(),
        accounts: token_mover::accounts::TransferWithHook {
            owner: payer.pubkey(),
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: dest_ata,
            token_program: TOKEN_2022_ID,
        }
        .to_account_metas(None),
        data: token_mover::instruction::TransferWithHook { amount: 1_000_000 }.data(),
    };
    ix1.accounts.push(AccountMeta::new_readonly(program_id, false));
    ix1.accounts.push(AccountMeta::new_readonly(extra_account_meta_list, false));
    ix1.accounts.push(AccountMeta::new(rate_limit, false));

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "CPI of 1_000_000 should succeed: {:?}", res.err());

    // Second transfer: 1 → must fail
    let mut ix2 = Instruction {
        program_id: token_mover::id(),
        accounts: token_mover::accounts::TransferWithHook {
            owner: payer.pubkey(),
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: dest_ata,
            token_program: TOKEN_2022_ID,
        }
        .to_account_metas(None),
        data: token_mover::instruction::TransferWithHook { amount: 1 }.data(),
    };
    ix2.accounts.push(AccountMeta::new_readonly(program_id, false));
    ix2.accounts.push(AccountMeta::new_readonly(extra_account_meta_list, false));
    ix2.accounts.push(AccountMeta::new(rate_limit, false));

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Second CPI should fail with rate limit exceeded");
}

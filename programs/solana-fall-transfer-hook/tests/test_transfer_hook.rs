#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{Id, InstructionData, ToAccountMetas},
    anchor_spl::token_2022::Token2022,
    anchor_lang::solana_program::instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
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


#[test]
fn test_rate_limit_is_per_user() {
    let (mut svm, payer, program_id) = setup();
    let second_wallet = Keypair::new();
    svm.airdrop(&second_wallet.pubkey(), 1_000_000_000).unwrap();

    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    helpers::initialize_rate_limit(&mut svm, &second_wallet, &mint, &program_id);

    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let second_wallet_ata = create_ata(
        &mut svm,
        &payer,
        &second_wallet.pubkey(),
        &mint.pubkey(),
    );
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_ata, 1_000_000);
    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &second_wallet_ata,
        1_000_000,
    );

    let payer_transfer = build_transfer_with_hook_ix(
        &payer_ata,
        &second_wallet_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[payer_transfer], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "First wallet transfer failed: {:?}", result.err());

    let second_wallet_transfer = build_transfer_with_hook_ix(
        &second_wallet_ata,
        &payer_ata,
        &mint.pubkey(),
        &second_wallet.pubkey(),
        &program_id,
        1_000_000,
        9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[second_wallet_transfer],
        Some(&second_wallet.pubkey()),
        &blockhash,
    );
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&second_wallet],
    )
    .unwrap();
    let result = svm.send_transaction(tx);
    assert!(
        result.is_ok(),
        "Second wallet transfer failed: {:?}",
        result.err()
    );
}

fn build_transfer_with_mover_ix(
    source_ata: &Pubkey,
    dest_ata: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    hook_program_id: &Pubkey,
    amount: u64,
    decimals: u8,
) -> Instruction {
    let mut ix = Instruction::new_with_bytes(
        token_mover::id(),
        &token_mover::instruction::TransferWithHook { amount, decimals }.data(),
        token_mover::accounts::TransferWithHook {
            owner: *owner,
            source_token: *source_ata,
            mint: *mint,
            destination_token: *dest_ata,
            token_program: Token2022::id(),
        }
        .to_account_metas(None),
    );

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.as_ref()],
        hook_program_id,
    )
    .0;
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.as_ref(), owner.as_ref()],
        hook_program_id,
    )
    .0;

    ix.accounts.push(AccountMeta::new_readonly(*hook_program_id, false));
    ix.accounts.push(AccountMeta::new_readonly(extra_account_meta_list, false));
    ix.accounts.push(AccountMeta::new(rate_limit, false));

    ix
}






#[test]
fn test_transfer_through_token_mover() {
    let (mut svm, payer, hook_program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let destination_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 100);

    let ix = build_transfer_with_mover_ix(
        &source_ata,
        &destination_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &hook_program_id,
        100,
        9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Mover transfer failed: {:?}", result.err());
}

#[test]
fn test_transfer_through_token_mover_rate_limit() {
    let (mut svm, payer, hook_program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let destination_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_001);

    let first_ix = build_transfer_with_mover_ix(
        &source_ata,
        &destination_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &hook_program_id,
        1_000_000,
        9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[first_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    assert!(svm.send_transaction(tx).is_ok());

    let second_ix = build_transfer_with_mover_ix(
        &source_ata,
        &destination_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &hook_program_id,
        1,
        9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[second_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Transfer over the limit should fail");
    assert!(format!("{:?}", result.err()).contains("0x1771"));
}
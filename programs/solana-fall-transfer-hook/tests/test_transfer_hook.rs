#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        InstructionData, ToAccountMetas,
        prelude::Clock,
        solana_program::instruction::Instruction,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
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
fn test_rate_limit_is_separate_for_each_owner() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let second_owner = Keypair::new();
    svm.airdrop(&second_owner.pubkey(), 1_000_000_000).unwrap();
    initialize_rate_limit(&mut svm, &second_owner, &mint, &program_id);

    let recipient = Keypair::new();
    let payer_source = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let second_source = create_ata(
        &mut svm,
        &payer,
        &second_owner.pubkey(),
        &mint.pubkey(),
    );
    let destination = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_source, 1_000_000);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &second_source, 1);

    let payer_transfer = build_transfer_with_hook_ix(
        &payer_source,
        &destination,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        9,
    );
    assert!(send_transfer(&mut svm, payer_transfer, &payer));

    let second_owner_transfer = build_transfer_with_hook_ix(
        &second_source,
        &destination,
        &mint.pubkey(),
        &second_owner.pubkey(),
        &program_id,
        1,
        9,
    );
    assert!(
        send_transfer(&mut svm, second_owner_transfer, &second_owner),
        "One owner's transfers should not consume another owner's limit",
    );
}

#[test]
fn test_rate_limit_is_separate_for_each_mint() {
    let (mut svm, payer, program_id) = setup();
    let first_mint = Keypair::new();
    let second_mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &first_mint, &program_id);
    setup_mint_and_extra_metas(&mut svm, &payer, &second_mint, &program_id);

    let recipient = Keypair::new();
    let first_source = create_ata(
        &mut svm,
        &payer,
        &payer.pubkey(),
        &first_mint.pubkey(),
    );
    let first_destination = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &first_mint.pubkey(),
    );
    let second_source = create_ata(
        &mut svm,
        &payer,
        &payer.pubkey(),
        &second_mint.pubkey(),
    );
    let second_destination = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &second_mint.pubkey(),
    );
    mint_tokens(
        &mut svm,
        &payer,
        &first_mint.pubkey(),
        &first_source,
        1_000_000,
    );
    mint_tokens(
        &mut svm,
        &payer,
        &second_mint.pubkey(),
        &second_source,
        1,
    );

    let first_transfer = build_transfer_with_hook_ix(
        &first_source,
        &first_destination,
        &first_mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        9,
    );
    assert!(send_transfer(&mut svm, first_transfer, &payer));

    let second_transfer = build_transfer_with_hook_ix(
        &second_source,
        &second_destination,
        &second_mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1,
        9,
    );
    assert!(
        send_transfer(&mut svm, second_transfer, &payer),
        "One mint's transfers should not consume another mint's limit",
    );
}

#[test]
fn test_rate_limit_resets_after_window_expires() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let destination = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source, 1_000_001);

    let first_transfer = build_transfer_with_hook_ix(
        &source,
        &destination,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        9,
    );
    assert!(send_transfer(&mut svm, first_transfer, &payer));

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += solana_fall_transfer_hook::ONE_HOUR + 1;
    svm.set_sysvar::<Clock>(&clock);
    svm.expire_blockhash();

    let transfer_after_reset = build_transfer_with_hook_ix(
        &source,
        &destination,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1,
        9,
    );
    assert!(
        send_transfer(&mut svm, transfer_after_reset, &payer),
        "Transfer should succeed after the rate-limit window resets",
    );
}

#[test]
fn test_direct_transfer_hook_call_is_rejected() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let destination = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );
    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &program_id,
    )
    .0;
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        &program_id,
    )
    .0;
    let direct_hook_call = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::TransferHook { amount: 1 }.data(),
        solana_fall_transfer_hook::accounts::TransferHook {
            source_token: source,
            mint: mint.pubkey(),
            destination_token: destination,
            owner: payer.pubkey(),
            extra_account_meta_list,
            rate_limit,
        }
        .to_account_metas(None),
    );

    let error = submit_instruction(&mut svm, direct_hook_call, &payer)
        .expect_err("The hook must only run during a Token-2022 transfer");
    assert!(
        error
            .meta
            .logs
            .iter()
            .any(|log| log.contains("NotTransferring")),
        "Expected NotTransferring error, got logs: {:?}",
        error.meta.logs,
    );
}

fn send_transfer(svm: &mut LiteSVM, instruction: Instruction, payer: &Keypair) -> bool {
    submit_instruction(svm, instruction, payer).is_ok()
}

fn submit_instruction(
    svm: &mut LiteSVM,
    instruction: Instruction,
    payer: &Keypair,
) -> Result<
    litesvm::types::TransactionMetadata,
    Box<litesvm::types::FailedTransactionMetadata>,
> {
    let blockhash = svm.latest_blockhash();
    let message = Message::new_with_blockhash(
        &[instruction],
        Some(&payer.pubkey()),
        &blockhash,
    );
    let transaction = VersionedTransaction::try_new(
        VersionedMessage::Legacy(message),
        &[payer],
    )
    .unwrap();
    svm.send_transaction(transaction).map_err(Box::new)
}

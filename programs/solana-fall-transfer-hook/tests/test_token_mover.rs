#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::AccountDeserialize,
    anchor_spl::token_2022::spl_token_2022::{
        extension::StateWithExtensions,
        state::Account as TokenAccount,
    },
    litesvm::LiteSVM,
    solana_fall_transfer_hook::RateLimit,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens, build_token_mover_transfer_ix,
};

fn token_balance(svm: &LiteSVM, ata: &Pubkey) -> u64 {
    let account = svm.get_account(ata).unwrap();
    StateWithExtensions::<TokenAccount>::unpack(&account.data).unwrap().base.amount
}

#[test]
fn test_token_mover_transfer_with_hook() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    // Wallet -> token-mover -> Token-2022 transfer_checked -> transfer hook
    let transfer_ix = build_token_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 100,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[transfer_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer through token-mover failed: {:?}", res.err());

    // The hook ran as the third CPI level: token-mover [1] -> Token-2022 [2] -> hook [3]
    let logs = res.unwrap().logs;
    assert!(
        logs.iter().any(|l| l == &format!("Program {} invoke [3]", program_id)),
        "Transfer hook was not invoked through Token-2022: {:#?}", logs
    );

    assert_eq!(token_balance(&svm, &source_ata), 999_900);
    assert_eq!(token_balance(&svm, &dest_ata), 100);

    // The hook recorded the transfer against the rate limit
    let rate_limit = Pubkey::find_program_address(&[b"rate_limit"], &program_id).0;
    let rate_limit_account = svm.get_account(&rate_limit).unwrap();
    let rate_limit_state = RateLimit::try_deserialize(&mut rate_limit_account.data.as_slice()).unwrap();
    assert_eq!(rate_limit_state.amount_transferred, 100);
}

#[test]
fn test_token_mover_rate_limit_exceeded() {
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
    let ix1 = build_token_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer at limit should succeed: {:?}", res.err());

    // Second transfer in the same hour: 1 base unit more - should fail with RateLimitExceeded (0x1771)
    let ix2 = build_token_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Transfer exceeding rate limit should fail");

    // The error must come from the transfer hook itself, reached through token-mover and Token-2022
    let failure = res.unwrap_err();
    let hook_failed_log = format!("Program {} failed: custom program error: 0x1771", program_id);
    assert!(
        failure.meta.logs.iter().any(|l| l == &hook_failed_log),
        "Expected RateLimitExceeded (0x1771) from the hook, got {:?}: {:#?}", failure.err, failure.meta.logs
    );

    // Nothing moved on the failed transfer
    assert_eq!(token_balance(&svm, &source_ata), 1_000_000);
    assert_eq!(token_balance(&svm, &dest_ata), 1_000_000);
}

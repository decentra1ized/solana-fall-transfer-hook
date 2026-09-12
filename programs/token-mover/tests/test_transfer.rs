#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_program_pack::Pack,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens, build_mover_transfer_ix,
};

// CHALLENGE 4: a transfer initiated by a *program* (token-mover), rather
// than a direct client instruction, must still trigger the rate-limiting
// transfer hook.
#[test]
fn program_initiated_transfer_triggers_the_hook() {
    let (mut svm, payer, mover_program_id, hook_program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let transfer_ix = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),
        &mover_program_id, &hook_program_id, 100,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[transfer_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Program-initiated transfer failed: {:?}", res.err());

    // Confirm the tokens actually moved, not just that the transaction
    // didn't error out.
    let dest_account = svm.get_account(&dest_ata).unwrap();
    let dest_state = anchor_spl::token_2022::spl_token_2022::state::Account::unpack_from_slice(
        &dest_account.data[..anchor_spl::token_2022::spl_token_2022::state::Account::LEN],
    ).unwrap();
    assert_eq!(dest_state.amount, 100);
}

// The hook must be applying the SAME rate limit to a program-initiated
// transfer as it does to a direct one - the mover can't be used to bypass
// the limit just because Token-2022 was invoked one hop further away.
#[test]
fn program_initiated_transfer_is_still_rate_limited() {
    let (mut svm, payer, mover_program_id, hook_program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    // Mint more than the rate limit so we have enough tokens to try.
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // First transfer: exactly at the limit - should succeed.
    let ix1 = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),
        &mover_program_id, &hook_program_id, 1_000_000,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Program-initiated transfer at limit should succeed: {:?}", res.err());

    // Second transfer: 1 token more - should fail with RateLimitExceeded,
    // surfaced from inside the hook, through Token-2022, through the mover.
    let ix2 = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),
        &mover_program_id, &hook_program_id, 1,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Program-initiated transfer exceeding the rate limit should fail");
}

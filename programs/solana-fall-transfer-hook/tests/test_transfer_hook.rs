#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::solana_program::instruction::Instruction,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, initialize_rate_limit_for,
    create_ata, mint_tokens, build_transfer_with_hook_ix,
};

fn send_and_expect(svm: &mut litesvm::LiteSVM, ix: Instruction, payer: &Keypair, signer: &Keypair, ok: bool) {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[signer]).unwrap();
    let res = svm.send_transaction(tx);
    assert_eq!(res.is_ok(), ok, "unexpected transaction result: {:?}", res.err());
}

#[test]
fn test_transfer_hook_within_limit_succeeds() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let ix = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 500_000, 9,
    );
    send_and_expect(&mut svm, ix, &payer, &payer, true);
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
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    let ix1 = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9,
    );
    send_and_expect(&mut svm, ix1, &payer, &payer, true);

    let ix2 = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1, 9,
    );
    send_and_expect(&mut svm, ix2, &payer, &payer, false);
}

/// Challenge 3: before the per-(mint, owner) reseed, this scenario failed -
/// both wallets shared one global rate_limit account, so the second
/// wallet's transfer would hit the first wallet's already-maxed counter.
#[test]
fn test_transfer_hook_rate_limit_is_per_owner() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let second_owner = Keypair::new();
    svm.airdrop(&second_owner.pubkey(), 1_000_000_000).unwrap();
    initialize_rate_limit_for(&mut svm, &second_owner, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let payer_source = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let second_source = create_ata(&mut svm, &payer, &second_owner.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_source, 1_000_000);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &second_source, 1_000_000);

    let ix1 = build_transfer_with_hook_ix(
        &payer_source, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9,
    );
    send_and_expect(&mut svm, ix1, &payer, &payer, true);

    let ix2 = build_transfer_with_hook_ix(
        &second_source, &dest_ata, &mint.pubkey(), &second_owner.pubkey(), &program_id, 1_000_000, 9,
    );
    send_and_expect(&mut svm, ix2, &second_owner, &second_owner, true);
}
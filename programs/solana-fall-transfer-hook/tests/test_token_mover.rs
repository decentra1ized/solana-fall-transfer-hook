#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_signer::Signer,
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens, build_mover_transfer_ix,
    try_send_ix,
};

/// Challenge 4: the transfer is initiated by another on-chain program rather
#[test]
fn test_token_mover_transfers() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let ix = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 100,
    );
    let res = try_send_ix(&mut svm, ix, &payer, &[&payer]);
    assert!(res.is_ok(), "Transfer through the mover failed: {:?}", res.err());
}


#[test]
fn test_token_mover_respects_rate_limit() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // Exactly at the cap - allowed.
    let ix = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000,
    );
    let res = try_send_ix(&mut svm, ix, &payer, &[&payer]);
    assert!(res.is_ok(), "Transfer at the limit should succeed: {:?}", res.err());

    // One base unit over - the hook rejects it, and the CPI unwinds with it.
    let ix = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1,
    );
    let res = try_send_ix(&mut svm, ix, &payer, &[&payer]);
    assert!(res.is_err(), "The hook must still run inside the mover's CPI");
}
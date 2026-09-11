// Proves challenge 4: a separate program can move tokens through the hook
// without tripping Solana's reentrancy guard, and the rate limit still
// applies inside that CPI path.

#[allow(dead_code)]
mod helpers;

use solana_keypair::Keypair;
use solana_signer::Signer;

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens,
    build_transfer_via_mover_ix, send_ix,
};

#[test]
fn transfer_via_mover_succeeds() {
    let (mut svm, payer, hook_program_id, mover_program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_ata, 2_000_000);

    let dest = Keypair::new();
    svm.airdrop(&dest.pubkey(), 1_000_000_000).unwrap();
    let dest_ata = create_ata(&mut svm, &dest, &dest.pubkey(), &mint.pubkey());

    // A well-under-the-cap transfer, routed through token-mover instead of
    // a direct wallet-to-wallet transfer_checked.
    let ix = build_transfer_via_mover_ix(
        &payer_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),
        &hook_program_id, &mover_program_id, 100,
    );

    // send_ix unwraps internally, so reaching this point proves the CPI
    // chain (mover -> Token-2022 -> hook) worked without a reentrancy error.
    send_ix(&mut svm, ix, &payer, &[&payer]);
}

#[test]
fn transfer_via_mover_still_enforces_the_rate_limit() {
    let (mut svm, payer, hook_program_id, mover_program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_ata, 2_000_000);

    let dest = Keypair::new();
    svm.airdrop(&dest.pubkey(), 1_000_000_000).unwrap();
    let dest_ata = create_ata(&mut svm, &dest, &dest.pubkey(), &mint.pubkey());

    // Spend the full 1,000,000 budget through the mover.
    let ix1 = build_transfer_via_mover_ix(
        &payer_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),
        &hook_program_id, &mover_program_id, 1_000_000,
    );
    send_ix(&mut svm, ix1, &payer, &[&payer]);

    // One more lamport over the cap, still via the mover, must fail.
    // A transfer that skipped the hook entirely would incorrectly succeed
    // here too, so this is the test that actually proves the hook runs.
    let blockhash = svm.latest_blockhash();
    let ix2 = build_transfer_via_mover_ix(
        &payer_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),
        &hook_program_id, &mover_program_id, 1,
    );
    let msg = solana_message::Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg), &[&payer],
    ).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "transfer over the rate limit via the mover must fail");
}

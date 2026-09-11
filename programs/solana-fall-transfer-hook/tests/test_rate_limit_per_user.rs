// Proves challenge 3: rate limits are now per mint AND per owner, not one
// shared bucket for the whole program.
//
// Before this challenge, the second wallet's transfer below would have
// failed - the first wallet's transfer would have already used up the
// single shared budget.

#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_signer::Signer,
};

use helpers::{
    setup, setup_mint_and_extra_metas, initialize_rate_limit, create_ata,
    mint_tokens, build_transfer_with_hook_ix, send_ix,
};

#[test]
fn each_owner_gets_their_own_rate_limit_budget() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // payer is the first owner; sets up the mint, its own rate limit, and
    // the shared extra-account-meta list for this mint.
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_ata, 2_000_000);

    // A second, independent wallet needs its own rate limit account too -
    // this is the "new rule after this challenge" the tutorial calls out.
    let second_owner = Keypair::new();
    svm.airdrop(&second_owner.pubkey(), 1_000_000_000).unwrap();
    initialize_rate_limit(&mut svm, &second_owner, &mint, &program_id);

    let second_owner_ata = create_ata(&mut svm, &second_owner, &second_owner.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &second_owner_ata, 2_000_000);

    let dest = Keypair::new();
    svm.airdrop(&dest.pubkey(), 1_000_000_000).unwrap();
    let dest_ata = create_ata(&mut svm, &dest, &dest.pubkey(), &mint.pubkey());

    // payer spends its full 1,000,000 budget.
    let ix1 = build_transfer_with_hook_ix(
        &payer_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id,
        1_000_000, 9,
    );
    send_ix(&mut svm, ix1, &payer, &[&payer]);

    // Before challenge 3, this would fail: it would land on the same
    // shared account payer just filled up. Now it is a fresh budget.
    let ix2 = build_transfer_with_hook_ix(
        &second_owner_ata, &dest_ata, &mint.pubkey(), &second_owner.pubkey(), &program_id,
        1_000_000, 9,
    );
    send_ix(&mut svm, ix2, &second_owner, &[&second_owner]);

    // Both should have succeeded (send_ix unwraps internally), so if we get
    // here, both owners exhausted their own independent budgets.
}

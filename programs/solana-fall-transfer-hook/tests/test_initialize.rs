#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        solana_program::instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID,
        InstructionData, ToAccountMetas,
    },
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

use helpers::{
    assert_logs_contain, create_legacy_mint, fetch_rate_limit, initialize_mint,
    initialize_rate_limit, initialize_rate_limit_for, initialize_rate_limit_ix, rate_limit_pda,
    setup, try_send_ix,
};

#[test]
fn test_initialize_persists_mint_and_authority() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    initialize_mint(&mut svm, &payer, &mint, &program_id);
    initialize_rate_limit(&mut svm, &payer, &mint, &program_id);

    let pda = rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id);
    let rate_limit = fetch_rate_limit(&svm, &pda);

    assert_eq!(rate_limit.authority, payer.pubkey());
    assert_eq!(rate_limit.mint, mint.pubkey());
    assert_eq!(rate_limit.max_amount, solana_fall_transfer_hook::RateLimit::MAX_AMOUNT);
    assert_eq!(rate_limit.amount_transferred, 0);
}

#[test]
fn test_initialize_rejects_legacy_token_mint() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    create_legacy_mint(&mut svm, &payer, &mint);

    let err = try_send_ix(
        &mut svm,
        initialize_rate_limit_ix(&payer.pubkey(), &mint.pubkey(), &payer.pubkey(), &program_id),
        &payer,
        &[&payer],
    )
    .expect_err("legacy SPL Token mint must be rejected");

    assert_logs_contain(&err, "Invalid mint account");
}

#[test]
fn test_initialize_rejects_non_mint_account() {
    let (mut svm, payer, program_id) = setup();

    let err = try_send_ix(
        &mut svm,
        initialize_rate_limit_ix(
            &payer.pubkey(),
            &payer.pubkey(),
            &payer.pubkey(),
            &program_id,
        ),
        &payer,
        &[&payer],
    )
    .expect_err("system account must not pass as mint");

    assert!(!err.meta.logs.is_empty());
}

#[test]
fn test_initialize_rejects_wrong_pda_seeds() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    initialize_mint(&mut svm, &payer, &mint, &program_id);

    let stale_global_pda = Pubkey::find_program_address(&[b"rate_limit"], &program_id).0;

    let ix = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            owner: payer.pubkey(),
            rate_limit: stale_global_pda,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    );

    try_send_ix(&mut svm, ix, &payer, &[&payer])
        .expect_err("old global rate_limit PDA must fail seed constraint");
}

#[test]
fn test_initialize_is_unique_per_owner_and_mint() {
    let (mut svm, payer, program_id) = setup();
    let mint_a = Keypair::new();
    let mint_b = Keypair::new();
    let alice = Keypair::new();

    initialize_mint(&mut svm, &payer, &mint_a, &program_id);
    initialize_mint(&mut svm, &payer, &mint_b, &program_id);

    initialize_rate_limit_for(&mut svm, &payer, &mint_a.pubkey(), &payer.pubkey(), &program_id);
    initialize_rate_limit_for(&mut svm, &payer, &mint_a.pubkey(), &alice.pubkey(), &program_id);
    initialize_rate_limit_for(&mut svm, &payer, &mint_b.pubkey(), &payer.pubkey(), &program_id);

    let payer_a = rate_limit_pda(&mint_a.pubkey(), &payer.pubkey(), &program_id);
    let alice_a = rate_limit_pda(&mint_a.pubkey(), &alice.pubkey(), &program_id);
    let payer_b = rate_limit_pda(&mint_b.pubkey(), &payer.pubkey(), &program_id);

    assert_ne!(payer_a, alice_a);
    assert_ne!(payer_a, payer_b);
    assert_eq!(fetch_rate_limit(&svm, &payer_a).mint, mint_a.pubkey());
    assert_eq!(fetch_rate_limit(&svm, &alice_a).mint, mint_a.pubkey());
    assert_eq!(fetch_rate_limit(&svm, &payer_b).mint, mint_b.pubkey());
}

#[test]
fn test_initialize_rejects_duplicate() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    initialize_mint(&mut svm, &payer, &mint, &program_id);
    initialize_rate_limit(&mut svm, &payer, &mint, &program_id);

    try_send_ix(
        &mut svm,
        initialize_rate_limit_ix(&payer.pubkey(), &mint.pubkey(), &payer.pubkey(), &program_id),
        &payer,
        &[&payer],
    )
    .expect_err("second init for the same (mint, owner) must fail");
}

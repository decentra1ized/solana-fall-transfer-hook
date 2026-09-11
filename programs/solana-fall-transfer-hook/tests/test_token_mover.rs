//! Integration test for the separate token-mover program.
//!
//! Why do we need a second program?
//!
//! The transfer-hook program cannot call Token-2022 and then have
//! Token-2022 call back into the same transfer-hook program. Solana
//! blocks that circular call as reentrancy.
//!
//! The safe call path is:
//!
//! token-mover -> Token-2022 -> transfer-hook
//!
//! This test proves that the separate token-mover can perform the
//! transfer while the rate-limit hook still runs.

mod helpers;

use {
    anchor_lang::{
        solana_program::instruction::{AccountMeta, Instruction},
        AccountDeserialize, Id, InstructionData, ToAccountMetas,
    },
    anchor_spl::token_2022::{
        spl_token_2022::{extension::StateWithExtensions, state::Account as SplTokenAccount},
        Token2022,
    },
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

use helpers::*;

/// Reads the token balance stored inside a Token-2022 account.
///
/// LiteSVM gives us the raw account data. Token-2022 then
/// deserializes that data so we can inspect its token amount.
fn token_amount(svm: &litesvm::LiteSVM, token_account: &Pubkey) -> u64 {
    let account = svm
        .get_account(token_account)
        .expect("The token account should exist");

    let token_state = StateWithExtensions::<SplTokenAccount>::unpack(&account.data)
        .expect("The account should contain valid Token-2022 data");

    token_state.base.amount
}

/// Reads our custom RateLimit account from LiteSVM.
///
/// This lets the test confirm that the transfer hook actually ran
/// and recorded the transferred amount.
fn read_rate_limit(
    svm: &litesvm::LiteSVM,
    rate_limit_address: &Pubkey,
) -> solana_fall_transfer_hook::RateLimit {
    let account = svm
        .get_account(rate_limit_address)
        .expect("The RateLimit account should exist");

    let mut account_data: &[u8] = &account.data;

    solana_fall_transfer_hook::RateLimit::try_deserialize(&mut account_data)
        .expect("The RateLimit account should deserialize")
}

/// Proves that the token-mover can transfer tokens and that the
/// Token-2022 transfer hook still checks and updates the rate limit.
#[test]
fn token_mover_executes_transfer_hook_without_reentrancy() {
    // Create the local test blockchain and load the transfer-hook program.
    let (mut svm, payer, transfer_hook_program_id) = setup();

    // Load the separate token-mover program into LiteSVM.
    //
    // This program begins the transfer so Token-2022 can safely call
    // the transfer-hook program without calling back into the same
    // program that started the transfer.
    let token_mover_program_id = token_mover::id();
    let token_mover_bytes = include_bytes!("../../../target/deploy/token_mover.so");

    svm.add_program(token_mover_program_id, token_mover_bytes)
        .expect("The token-mover program should load");

    // Create a new Token-2022 mint for this test.
    let mint = Keypair::new();

    // Create another wallet that will receive the transferred tokens.
    let recipient = Keypair::new();

    // Initialize:
    //
    // 1. The Token-2022 mint.
    // 2. The payer's RateLimit PDA.
    // 3. The mint's Extra Account Meta List.
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &transfer_hook_program_id);

    // Create the payer's token account.
    let source_token = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());

    // Create the recipient's token account.
    let destination_token = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    // Give the payer 500,000 test tokens.
    let starting_balance = 500_000_u64;

    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_token,
        starting_balance,
    );

    // Transfer an amount that is below the 1,000,000-token limit.
    let transfer_amount = 400_000_u64;

    // Find the Extra Account Meta List belonging to this mint.
    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &transfer_hook_program_id,
    )
    .0;

    // Find the RateLimit PDA belonging to this exact mint and owner.
    //
    // These seeds must match initialize.rs, transfer_hook.rs,
    // init_extra_account_meta.rs, and the helper functions.
    let rate_limit = Pubkey::find_program_address(
        &[
            b"rate_limit",
            mint.pubkey().as_ref(),
            payer.pubkey().as_ref(),
        ],
        &transfer_hook_program_id,
    )
    .0;

    // Build the instruction for the separate token-mover program.
    let mut transfer_instruction = Instruction::new_with_bytes(
        token_mover_program_id,
        &token_mover::instruction::TransferWithHook {
            amount: transfer_amount,
        }
        .data(),
        token_mover::accounts::TransferWithHook {
            owner: payer.pubkey(),
            source_token,
            mint: mint.pubkey(),
            destination_token,
            token_program: Token2022::id(),
        }
        .to_account_metas(None),
    );

    // Pass the accounts needed by Token-2022 and the transfer hook.
    //
    // These are remaining accounts because they are not part of the
    // token-mover program's normal Anchor account structure.
    //
    // Their order is important:
    //
    // 1. Transfer-hook program.
    // 2. Extra Account Meta List.
    // 3. Writable RateLimit PDA.
    transfer_instruction
        .accounts
        .push(AccountMeta::new_readonly(transfer_hook_program_id, false));

    transfer_instruction
        .accounts
        .push(AccountMeta::new_readonly(extra_account_meta_list, false));

    transfer_instruction
        .accounts
        .push(AccountMeta::new(rate_limit, false));

    // Send the transfer through the token-mover program.
    //
    // Before adding token-mover, this kind of transfer failed with
    // ReentrancyNotAllowed. It should now succeed.
    send_ix(&mut svm, transfer_instruction, &payer, &[&payer]);

    // Confirm that the tokens left the payer's account.
    assert_eq!(
        token_amount(&svm, &source_token),
        starting_balance - transfer_amount,
        "The sender should have 100,000 tokens remaining",
    );

    // Confirm that the recipient received the tokens.
    assert_eq!(
        token_amount(&svm, &destination_token),
        transfer_amount,
        "The recipient should receive 400,000 tokens",
    );

    // Confirm that the transfer hook ran and recorded the transfer.
    let rate_limit_state = read_rate_limit(&svm, &rate_limit);

    assert_eq!(
        rate_limit_state.amount_transferred, transfer_amount,
        "The transfer hook should record the transferred amount",
    );

    // Confirm that this rate-limit account belongs to the correct mint.
    assert_eq!(
        rate_limit_state.mint,
        mint.pubkey(),
        "The RateLimit account should belong to this mint",
    );

    // Confirm that this rate-limit account belongs to the payer.
    assert_eq!(
        rate_limit_state.authority,
        payer.pubkey(),
        "The RateLimit account should belong to the token owner",
    );
}

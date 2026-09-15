#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        InstructionData, ToAccountMetas,
        solana_program::instruction::{AccountMeta, Instruction},
    },
    anchor_spl::token_2022::Token2022,
    anchor_lang::Id,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{setup, setup_mint_and_extra_metas, create_ata, mint_tokens, rate_limit_pda};

/// Challenge 4, the part that motivates the separate program.
///
/// `transfer` performs exactly the same transfer_checked CPI as token-mover,
/// but from inside the hook program. Token-2022 then invokes the hook, which
/// is this same program - already on the call stack - and Solana rejects the
/// re-entrant call.
#[test]
fn test_in_program_transfer_is_rejected_as_reentrant() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &program_id,
    ).0;
    let rate_limit = rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id);

    let mut accounts = solana_fall_transfer_hook::accounts::Transfer {
        source: source_ata,
        mint: mint.pubkey(),
        destination: dest_ata,
        authority: payer.pubkey(),
        token_program: Token2022::id(),
    }.to_account_metas(None);
    accounts.push(AccountMeta::new_readonly(program_id, false));
    accounts.push(AccountMeta::new_readonly(extra_account_meta_list, false));
    accounts.push(AccountMeta::new(rate_limit, false));

    let ix = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::Transfer { amount: 500, decimals: 9 }.data(),
        accounts,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let err = svm.send_transaction(tx)
        .expect_err("the hook program must not be able to transfer its own hooked mint");

    // The runtime rejects it outright: InstructionError(0, ReentrancyNotAllowed).
    // The logs show the stack: hook [1] -> Token-2022 [2] -> hook again.
    let logs = err.meta.logs.join("\n");
    assert!(
        logs.contains("reentrancy not allowed"),
        "expected a re-entrancy rejection, got:\n{}", logs,
    );
}

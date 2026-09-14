//! Challenge 4: moving hooked tokens from inside a program.
//!
//! The hook program cannot CPI `transfer_checked` itself — the chain would be
//! hook -> Token-2022 -> hook, and Solana rejects direct re-entrancy. A second
//! program breaks the cycle: vault_transfer -> Token-2022 -> hook.

#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        solana_program::instruction::{AccountMeta, Instruction},
        InstructionData, ToAccountMetas,
    },
    anchor_spl::token_2022::{spl_token_2022, Token2022},
    anchor_lang::Id,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{create_ata, initialize_rate_limit_for, rate_limit_pda, setup, initialize_mint,
              initialize_extra_account_metas};

fn add_vault_program(svm: &mut LiteSVM) {
    let bytes = include_bytes!("../../../target/deploy/vault_transfer.so");
    svm.add_program(vault_transfer::id(), bytes).unwrap();
}

/// The vault PDA that owns the tokens and signs the CPI.
fn vault_pda(mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"vault", mint.as_ref()], &vault_transfer::id()).0
}

fn send_from_vault(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    vault: &Pubkey,
    vault_ata: &Pubkey,
    destination: &Pubkey,
    hook_program: &Pubkey,
    amount: u64,
) -> Result<litesvm::types::TransactionMetadata, litesvm::types::FailedTransactionMetadata> {
    let mut ix = Instruction::new_with_bytes(
        vault_transfer::id(),
        &vault_transfer::instruction::Send { amount }.data(),
        vault_transfer::accounts::Send {
            payer: payer.pubkey(),
            vault: *vault,
            mint: *mint,
            vault_ata: *vault_ata,
            destination: *destination,
            token_program: Token2022::id(),
        }
        .to_account_metas(None),
    );

    // The hook's extra accounts, resolved exactly as a wallet client would.
    // The owner here is the vault PDA, so it gets its own rate limit bucket.
    let meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.as_ref()],
        hook_program,
    )
    .0;
    ix.accounts.push(AccountMeta::new_readonly(*hook_program, false));
    ix.accounts.push(AccountMeta::new_readonly(meta_list, false));
    ix.accounts.push(AccountMeta::new(rate_limit_pda(mint, vault, hook_program), false));

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx)
}

#[test]
fn a_program_can_move_hooked_tokens_and_the_hook_still_runs() {
    let (mut svm, payer, program_id) = setup();
    add_vault_program(&mut svm);

    let mint = Keypair::new();
    initialize_mint(&mut svm, &payer, &mint, &program_id);
    initialize_extra_account_metas(&mut svm, &payer, &mint, &program_id);

    let mint_pk = mint.pubkey();
    let vault = vault_pda(&mint_pk);

    // The vault PDA is the token owner, so the bucket belongs to the PDA.
    initialize_rate_limit_for(&mut svm, &payer, &mint, &vault, &program_id);

    let vault_ata = create_ata(&mut svm, &payer, &vault, &mint_pk);
    let recipient = Keypair::new();
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint_pk);

    helpers::mint_tokens(&mut svm, &payer, &mint_pk, &vault_ata, 2_000_000);

    // On-chain code moves the tokens. Three distinct programs, no re-entrancy.
    let res = send_from_vault(
        &mut svm, &payer, &mint_pk, &vault, &vault_ata, &dest_ata, &program_id, 1_000_000,
    );
    assert!(res.is_ok(), "program-initiated transfer failed: {:?}", res.err());

    // And the hook really ran: one more lamport of token is over the cap.
    let res = send_from_vault(
        &mut svm, &payer, &mint_pk, &vault, &vault_ata, &dest_ata, &program_id, 1,
    );
    assert!(res.is_err(), "the hook should have rejected this: the cap is spent");
}

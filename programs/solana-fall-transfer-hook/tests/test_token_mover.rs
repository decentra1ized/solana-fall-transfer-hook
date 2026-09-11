#[allow(dead_code)]
mod helpers;

use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::{Id, InstructionData, ToAccountMetas};
use anchor_spl::token_2022::Token2022;
use helpers::{create_ata, mint_tokens, setup, setup_mint_and_extra_metas};
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

fn setup_mover() -> (LiteSVM, Keypair, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey) {
    let (mut svm, payer, hook_program_id) = setup();
    svm.add_program(
        token_mover::id(),
        include_bytes!("../../../target/deploy/token_mover.so"),
    )
    .unwrap();

    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);
    let recipient = Keypair::new();
    let source = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let destination = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source, 1_000_001);

    (
        svm,
        payer,
        hook_program_id,
        token_mover::id(),
        mint.pubkey(),
        source,
        destination,
    )
}

fn mover_instruction(
    mover_id: Pubkey,
    hook_id: Pubkey,
    mint: Pubkey,
    source: Pubkey,
    destination: Pubkey,
    owner: Pubkey,
    amount: u64,
) -> Instruction {
    let extra_account_meta_list =
        Pubkey::find_program_address(&[b"extra-account-metas", mint.as_ref()], &hook_id).0;
    let rate_limit =
        Pubkey::find_program_address(&[b"rate_limit", mint.as_ref(), owner.as_ref()], &hook_id).0;
    let mut accounts = token_mover::accounts::TransferWithHook {
        owner,
        source_token: source,
        mint,
        destination_token: destination,
        token_program: Token2022::id(),
    }
    .to_account_metas(None);
    accounts.push(AccountMeta::new_readonly(hook_id, false));
    accounts.push(AccountMeta::new_readonly(extra_account_meta_list, false));
    accounts.push(AccountMeta::new(rate_limit, false));

    Instruction::new_with_bytes(
        mover_id,
        &token_mover::instruction::TransferWithHook {
            amount,
            decimals: 9,
        }
        .data(),
        accounts,
    )
}

fn send(svm: &mut LiteSVM, payer: &Keypair, instruction: Instruction) -> Result<(), String> {
    let message = Message::new_with_blockhash(
        &[instruction],
        Some(&payer.pubkey()),
        &svm.latest_blockhash(),
    );
    let transaction = VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[payer])
        .map_err(|error| error.to_string())?;
    svm.send_transaction(transaction)
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn program_transfer_runs_the_hook() {
    let (mut svm, payer, hook_id, mover_id, mint, source, destination) = setup_mover();
    send(
        &mut svm,
        &payer,
        mover_instruction(
            mover_id,
            hook_id,
            mint,
            source,
            destination,
            payer.pubkey(),
            100,
        ),
    )
    .expect("program transfer should run the hook");
}

#[test]
fn program_transfer_still_enforces_the_rate_limit() {
    let (mut svm, payer, hook_id, mover_id, mint, source, destination) = setup_mover();
    send(
        &mut svm,
        &payer,
        mover_instruction(
            mover_id,
            hook_id,
            mint,
            source,
            destination,
            payer.pubkey(),
            1_000_000,
        ),
    )
    .expect("transfer at the limit should succeed");
    let error = send(
        &mut svm,
        &payer,
        mover_instruction(
            mover_id,
            hook_id,
            mint,
            source,
            destination,
            payer.pubkey(),
            1,
        ),
    )
    .expect_err("transfer over the limit should fail");
    assert!(
        error.contains("0x1771"),
        "expected rate-limit error, got {error}"
    );
}

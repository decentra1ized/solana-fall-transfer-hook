#[allow(dead_code)]
mod helpers;

use {
    std::{
        future::Future,
        task::{Context, Poll, Waker},
    },
    anchor_lang::{
        Id, InstructionData, ToAccountMetas,
        solana_program::instruction::Instruction,
        system_program::ID as SYSTEM_PROGRAM_ID,
    },
    anchor_spl::token_2022::{spl_token_2022, Token2022},
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_transfer_hook_interface::offchain::{
        add_extra_account_metas_for_execute, AccountFetchError,
    },
};

use helpers::{
    setup, send_ix, initialize_mint, initialize_rate_limit,
    initialize_extra_account_metas,
};

#[test]
fn test_initialize_extra_account_meta_list() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    initialize_mint(&mut svm, &payer, &mint, &program_id);
    initialize_rate_limit(&mut svm, &payer, &mint, &program_id);

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &program_id,
    ).0;

    let init_extra_ix = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::InitializeExtraAccountMetaList {}.data(),
        solana_fall_transfer_hook::accounts::InitializeExtraAccountMetaList {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            extra_account_meta_list,
            system_program: SYSTEM_PROGRAM_ID,
        }.to_account_metas(None),
    );
    send_ix(&mut svm, init_extra_ix, &payer, &[&payer]);

    let account = svm.get_account(&extra_account_meta_list);
    assert!(account.is_some(), "ExtraAccountMetaList account should exist");
}

#[test]
fn test_extra_account_meta_list_resolves_rate_limit() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    initialize_mint(&mut svm, &payer, &mint, &program_id);
    initialize_rate_limit(&mut svm, &payer, &mint, &program_id);
    initialize_extra_account_metas(&mut svm, &payer, &mint, &program_id);

    let source = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let mut transfer_instruction = spl_token_2022::instruction::transfer_checked(
        &Token2022::id(),
        &source,
        &mint.pubkey(),
        &destination,
        &payer.pubkey(),
        &[],
        100,
        9,
    )
    .unwrap();
    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &program_id,
    )
    .0;
    let validation_data = svm
        .get_account(&extra_account_meta_list)
        .unwrap()
        .data;

    block_on(add_extra_account_metas_for_execute(
        &mut transfer_instruction,
        &program_id,
        &source,
        &mint.pubkey(),
        &destination,
        &payer.pubkey(),
        100,
        move |address| {
            let data = (address == extra_account_meta_list)
                .then(|| validation_data.clone());
            async move { Ok::<Option<Vec<u8>>, AccountFetchError>(data) }
        },
    ))
    .unwrap();

    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        &program_id,
    )
    .0;
    assert_eq!(transfer_instruction.accounts[4].pubkey, rate_limit);
    assert!(transfer_instruction.accounts[4].is_writable);
    assert_eq!(transfer_instruction.accounts[5].pubkey, program_id);
    assert_eq!(transfer_instruction.accounts[6].pubkey, extra_account_meta_list);
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

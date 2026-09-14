use {
    anchor_lang::{
        solana_program::instruction::{AccountMeta, Instruction},
        system_program::ID as SYSTEM_PROGRAM_ID,
        AccountDeserialize, Id, InstructionData, ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{
            get_associated_token_address_with_program_id, spl_associated_token_account,
        },
        token_2022::{
            spl_token_2022::{self, extension::StateWithExtensions, state::Account as TokenAccount},
            Token2022,
        },
    },
    litesvm::{
        types::{FailedTransactionMetadata, TransactionMetadata},
        LiteSVM,
    },
    solana_keypair::{Address, Keypair},
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    solana_fall_transfer_hook::RateLimit,
};

pub fn setup() -> (LiteSVM, Keypair, Address) {
    let program_id = solana_fall_transfer_hook::id();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../../target/deploy/solana_fall_transfer_hook.so");
    svm.add_program(program_id, bytes).unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    (svm, payer, program_id)
}

pub fn rate_limit_pda(mint: &Pubkey, owner: &Pubkey, program_id: &Address) -> Pubkey {
    Pubkey::find_program_address(&[b"rate_limit", mint.as_ref(), owner.as_ref()], program_id).0
}

pub fn extra_metas_pda(mint: &Pubkey, program_id: &Address) -> Pubkey {
    Pubkey::find_program_address(&[b"extra-account-metas", mint.as_ref()], program_id).0
}

pub fn try_send_ix(
    svm: &mut LiteSVM,
    ix: Instruction,
    payer: &Keypair,
    signers: &[&Keypair],
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

pub fn send_ix(svm: &mut LiteSVM, ix: Instruction, payer: &Keypair, signers: &[&Keypair]) {
    try_send_ix(svm, ix, payer, signers).unwrap();
}

pub fn assert_logs_contain(err: &FailedTransactionMetadata, needle: &str) {
    let logs = err.meta.logs.join("\n");
    assert!(
        logs.contains(needle),
        "expected logs to contain {needle:?}, got:\n{logs}"
    );
}

pub fn fetch_rate_limit(svm: &LiteSVM, pda: &Pubkey) -> RateLimit {
    let acc = svm.get_account(pda).expect("rate limit account missing");
    let mut data: &[u8] = &acc.data;
    RateLimit::try_deserialize(&mut data).expect("failed to deserialize RateLimit")
}

pub fn token_amount(svm: &LiteSVM, ata: &Pubkey) -> u64 {
    let acc = svm.get_account(ata).expect("token account missing");
    StateWithExtensions::<TokenAccount>::unpack(&acc.data)
        .expect("failed to unpack token account")
        .base
        .amount
}

pub fn initialize_mint(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, program_id: &Address) {
    let ix = Instruction::new_with_bytes(
        *program_id,
        &solana_fall_transfer_hook::instruction::InitializeMint {}.data(),
        solana_fall_transfer_hook::accounts::InitializeMint {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            system_program: SYSTEM_PROGRAM_ID,
            token_program: Token2022::id(),
        }
        .to_account_metas(None),
    );
    send_ix(svm, ix, payer, &[payer, mint]);
}

pub fn initialize_rate_limit_ix(
    payer: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    program_id: &Address,
) -> Instruction {
    Instruction::new_with_bytes(
        *program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: *payer,
            mint: *mint,
            owner: *owner,
            rate_limit: rate_limit_pda(mint, owner, program_id),
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}

pub fn initialize_rate_limit_for(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    owner: &Pubkey,
    program_id: &Address,
) {
    send_ix(
        svm,
        initialize_rate_limit_ix(&payer.pubkey(), mint, owner, program_id),
        payer,
        &[payer],
    );
}

pub fn initialize_rate_limit(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    program_id: &Address,
) {
    initialize_rate_limit_for(svm, payer, &mint.pubkey(), &payer.pubkey(), program_id);
}

pub fn create_legacy_mint(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair) {
    use anchor_lang::solana_program::{program_pack::Pack, system_instruction};
    use anchor_spl::token::spl_token;

    let create = system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        10_000_000,
        spl_token::state::Mint::LEN as u64,
        &spl_token::ID,
    );
    let init = spl_token::instruction::initialize_mint(
        &spl_token::ID,
        &mint.pubkey(),
        &payer.pubkey(),
        None,
        9,
    )
    .unwrap();

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[create, init], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, mint]).unwrap();
    svm.send_transaction(tx).unwrap();
}

pub fn initialize_extra_account_metas(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    program_id: &Address,
) {
    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        program_id,
    )
    .0;

    let ix = Instruction::new_with_bytes(
        *program_id,
        &solana_fall_transfer_hook::instruction::InitializeExtraAccountMetaList {}.data(),
        solana_fall_transfer_hook::accounts::InitializeExtraAccountMetaList {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            extra_account_meta_list,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    send_ix(svm, ix, payer, &[payer]);
}

pub fn setup_mint_and_extra_metas(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    program_id: &Address,
) {
    initialize_mint(svm, payer, mint, program_id);
    initialize_rate_limit(svm, payer, mint, program_id);
    initialize_extra_account_metas(svm, payer, mint, program_id);
}

pub fn create_ata(svm: &mut LiteSVM, payer: &Keypair, wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    let ata = get_associated_token_address_with_program_id(wallet, mint, &Token2022::id());
    let ix = spl_associated_token_account::instruction::create_associated_token_account(
        &payer.pubkey(),
        wallet,
        mint,
        &Token2022::id(),
    );
    send_ix(svm, ix, payer, &[payer]);
    ata
}

pub fn mint_tokens(svm: &mut LiteSVM, payer: &Keypair, mint: &Pubkey, dest: &Pubkey, amount: u64) {
    let ix = spl_token_2022::instruction::mint_to(
        &Token2022::id(),
        mint,
        dest,
        &payer.pubkey(),
        &[],
        amount,
    )
    .unwrap();
    send_ix(svm, ix, payer, &[payer]);
}

pub fn build_transfer_with_hook_ix(
    source_ata: &Pubkey,
    dest_ata: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    program_id: &Address,
    amount: u64,
    decimals: u8,
) -> Instruction {
    let mut ix = spl_token_2022::instruction::transfer_checked(
        &Token2022::id(),
        source_ata,
        mint,
        dest_ata,
        owner,
        &[],
        amount,
        decimals,
    )
    .unwrap();

    ix.accounts
        .push(AccountMeta::new_readonly(*program_id, false));
    ix.accounts
        .push(AccountMeta::new_readonly(extra_metas_pda(mint, program_id), false));
    ix.accounts
        .push(AccountMeta::new(rate_limit_pda(mint, owner, program_id), false));

    ix
}

pub fn build_program_transfer_ix(
    source_ata: &Pubkey,
    dest_ata: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    program_id: &Address,
    amount: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        *program_id,
        &solana_fall_transfer_hook::instruction::TransferChecked { amount }.data(),
        solana_fall_transfer_hook::accounts::ProgramTransfer {
            owner: *owner,
            source_token: *source_ata,
            mint: *mint,
            destination_token: *dest_ata,
            extra_account_meta_list: extra_metas_pda(mint, program_id),
            rate_limit: rate_limit_pda(mint, owner, program_id),
            hook_program: *program_id,
            token_program: Token2022::id(),
        }
        .to_account_metas(None),
    )
}

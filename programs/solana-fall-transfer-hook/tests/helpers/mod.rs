use {
    anchor_lang::{
        solana_program::instruction::{AccountMeta, Instruction},
        system_program::ID as SYSTEM_PROGRAM_ID,
        Id, InstructionData, ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{
            get_associated_token_address_with_program_id, spl_associated_token_account,
        },
        token_2022::{spl_token_2022, Token2022},
    },
    litesvm::LiteSVM,
    solana_keypair::{Address, Keypair},
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

/// Creates a fresh local Solana test environment.
///
/// LiteSVM behaves like a small Solana blockchain running in memory.
/// It lets us test the program without using real SOL or deploying online.
pub fn setup() -> (LiteSVM, Keypair, Address) {
    let program_id = solana_fall_transfer_hook::id();
    let mut svm = LiteSVM::new();

    // Load the compiled Solana program into the local test blockchain.
    let bytes = include_bytes!("../../../../target/deploy/solana_fall_transfer_hook.so");
    svm.add_program(program_id, bytes).unwrap();

    // Create a temporary test wallet and give it fake SOL.
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    (svm, payer, program_id)
}

/// Sends one instruction to the local LiteSVM blockchain.
///
/// The payer pays the transaction fee, while `signers` contains every
/// wallet that must authorize the instruction.
pub fn send_ix(svm: &mut LiteSVM, ix: Instruction, payer: &Keypair, signers: &[&Keypair]) {
    let blockhash = svm.latest_blockhash();

    let message = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);

    let transaction =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), signers).unwrap();

    svm.send_transaction(transaction).unwrap();
}

/// Creates a Token-2022 mint with our transfer-hook extension enabled.
///
/// The mint represents the token itself. Every transfer of this token
/// will automatically call our transfer-hook program.
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

    // Both the payer and the new mint must sign its creation.
    send_ix(svm, ix, payer, &[payer, mint]);
}

/// Creates one RateLimit PDA for a specific Token-2022 mint and owner.
///
/// The payer is also the token owner in these tests. Including both the
/// mint and owner in the seeds prevents unrelated users and tokens from
/// sharing one global rate-limit counter.
pub fn initialize_rate_limit(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    program_id: &Address,
) {
    // Save the public keys so they can be reused safely below.
    let payer_key = payer.pubkey();
    let mint_key = mint.pubkey();

    // These seeds must exactly match initialize.rs, transfer_hook.rs,
    // and init_extra_account_meta.rs.
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint_key.as_ref(), payer_key.as_ref()],
        program_id,
    )
    .0;

    // Build the instruction that creates and fills the RateLimit PDA.
    let ix = Instruction::new_with_bytes(
        *program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: payer_key,
            mint: mint_key,
            rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    );

    send_ix(svm, ix, payer, &[payer]);
}

/// Creates the Extra Account Meta List for a particular mint.
///
/// Token-2022 reads this list to learn which additional accounts must
/// be included when it calls our transfer hook.
pub fn initialize_extra_account_metas(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    program_id: &Address,
) {
    // Every mint receives its own Extra Account Meta List PDA.
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

/// Performs all the setup required before testing token transfers.
///
/// This creates:
/// 1. The Token-2022 mint.
/// 2. The owner's RateLimit PDA.
/// 3. The mint's Extra Account Meta List.
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

/// Creates a Token-2022 associated token account.
///
/// An ATA is the account that actually holds a wallet's tokens.
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

/// Creates test tokens and deposits them into a token account.
pub fn mint_tokens(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    destination: &Pubkey,
    amount: u64,
) {
    let ix = spl_token_2022::instruction::mint_to(
        &Token2022::id(),
        mint,
        destination,
        &payer.pubkey(),
        &[],
        amount,
    )
    .unwrap();

    send_ix(svm, ix, payer, &[payer]);
}

/// Builds a Token-2022 transfer that automatically calls our hook.
///
/// Token-2022 requires three extra accounts:
/// 1. Our transfer-hook program.
/// 2. The mint's Extra Account Meta List.
/// 3. The owner's writable RateLimit PDA.
pub fn build_transfer_with_hook_ix(
    source_ata: &Pubkey,
    destination_ata: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    program_id: &Address,
    amount: u64,
    decimals: u8,
) -> Instruction {
    // Start with a normal Token-2022 checked-transfer instruction.
    let mut ix = spl_token_2022::instruction::transfer_checked(
        &Token2022::id(),
        source_ata,
        mint,
        destination_ata,
        owner,
        &[],
        amount,
        decimals,
    )
    .unwrap();

    // Find the metadata list associated with this mint.
    let extra_account_meta_list =
        Pubkey::find_program_address(&[b"extra-account-metas", mint.as_ref()], program_id).0;

    // Find the rate-limit account belonging to this mint and owner.
    //
    // The seed order must always be:
    // ["rate_limit", mint, owner]
    let rate_limit =
        Pubkey::find_program_address(&[b"rate_limit", mint.as_ref(), owner.as_ref()], program_id).0;

    // Add the accounts Token-2022 needs to execute our transfer hook.
    ix.accounts
        .push(AccountMeta::new_readonly(*program_id, false));

    ix.accounts
        .push(AccountMeta::new_readonly(extra_account_meta_list, false));

    // The hook updates amount_transferred, so this account is writable.
    ix.accounts.push(AccountMeta::new(rate_limit, false));

    ix
}

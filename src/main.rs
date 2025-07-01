use apl_token::state::{Mint, Account};
use arch_program::{program_pack::Pack, sanitized::ArchMessage};
use arch_sdk::{build_and_sign_transaction, generate_new_keypair, ArchRpcClient, Status};
use arch_test_sdk::{
    constants::{ BITCOIN_NETWORK,NODE1_ADDRESS},
    helper::{create_and_fund_account_with_faucet, read_account_info, send_transactions_and_wait},
};
use log::info;
// const BITCOIN_NETWORK: Network = Network::Testnet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    info!("🚀 Starting test_token program...");

    let client = ArchRpcClient::new(NODE1_ADDRESS);
    println!("📡 Connected to node: {}", NODE1_ADDRESS);
    println!("🌐 Using network: {:?}", BITCOIN_NETWORK);

    // Run the complete token lifecycle
    run_token_lifecycle(&client)?;

    println!("🎉 Token lifecycle completed successfully!");
    Ok(())
}

pub fn run_token_lifecycle(client: &ArchRpcClient) -> Result<(), Box<dyn std::error::Error>> {
    // Step 1: Create token mint
    println!("\n📋 Step 1: Creating token mint...");
    let (authority_keypair, token_mint_pubkey) = create_token_mint(client)?;
    
    // Step 2: Create user accounts
    println!("\n👥 Step 2: Creating user accounts...");
    let (user1_keypair, user1_pubkey, _) = generate_new_keypair(BITCOIN_NETWORK);
    let (user2_keypair, user2_pubkey, _) = generate_new_keypair(BITCOIN_NETWORK);
    
    create_and_fund_account_with_faucet(&user1_keypair, BITCOIN_NETWORK);
    create_and_fund_account_with_faucet(&user2_keypair, BITCOIN_NETWORK);

    // Step 3: Create token accounts
    println!("\n💳 Step 3: Creating token accounts...");
    let user1_token_account = create_token_account(client, token_mint_pubkey, user1_keypair)?;
    let user2_token_account = create_token_account(client, token_mint_pubkey, user2_keypair)?;

    // Step 4: Mint initial tokens
    println!("\n🪙 Step 4: Minting initial supply...");
    let authority_pubkey = arch_program::pubkey::Pubkey::from_slice(
        &authority_keypair.x_only_public_key().0.serialize()
    );
    mint_tokens(client, &token_mint_pubkey, &user1_token_account, &authority_pubkey, authority_keypair, 1_000_000_000)?; // 1,000 tokens (9 decimals)

    // Step 5: Check balance
    println!("\n💰 Step 5: Checking balances...");
    let user1_balance = get_token_balance(user1_token_account)?;
    println!("User1 balance: {} tokens", user1_balance as f64 / 1_000_000_000.0);

    // Step 6: Transfer tokens
    println!("\n📤 Step 6: Transferring tokens...");
    transfer_tokens(client, &user1_token_account, &user2_token_account, &user1_pubkey, user1_keypair, 500_000_000)?; // 500 tokens

    // Step 7: Check final balances
    println!("\n🏁 Step 7: Final balances...");
    let user1_final = get_token_balance(user1_token_account)?;
    let user2_final = get_token_balance(user2_token_account)?;
    
    println!("User1 final balance: {} tokens", user1_final as f64 / 1_000_000_000.0);
    println!("User2 final balance: {} tokens", user2_final as f64 / 1_000_000_000.0);

    // Step 8: Demonstrate burning tokens
    println!("\n🔥 Step 8: Burning some tokens...");
    burn_tokens(client, &user2_token_account, &token_mint_pubkey, &user2_pubkey, user2_keypair, 100_000_000)?; // Burn 100 tokens
    
    let user2_after_burn = get_token_balance(user2_token_account)?;
    println!("User2 balance after burn: {} tokens", user2_after_burn as f64 / 1_000_000_000.0);

    Ok(())
}

pub fn create_token_mint(client: &ArchRpcClient) -> Result<(bitcoin::key::Keypair, arch_program::pubkey::Pubkey), Box<dyn std::error::Error>> {
    // 1. Create mint authority (you control the token supply)
    let (authority_keypair, authority_pubkey, _) = generate_new_keypair(BITCOIN_NETWORK);
    create_and_fund_account_with_faucet(&authority_keypair, BITCOIN_NETWORK);

    // 2. Create mint account
    let (token_mint_keypair, token_mint_pubkey, _) = generate_new_keypair(BITCOIN_NETWORK);

    // 3. Create the mint account on-chain
    let create_account_ix = arch_program::system_instruction::create_account(
        &authority_pubkey,       // Payer
        &token_mint_pubkey,      // New account
        arch_program::account::MIN_ACCOUNT_LAMPORTS, // Rent
        Mint::LEN as u64,        // Space needed
        &apl_token::id(),        // Owner program
    );

    // 4. Initialize the mint with your token parameters
    let initialize_mint_ix = apl_token::instruction::initialize_mint(
        &apl_token::id(),
        &token_mint_pubkey,
        &authority_pubkey,       // Mint authority (can create tokens)
        None,                   // No freeze authority (optional)
        9,                      // Decimals (9 = like USDC, 0 = whole numbers only)
    )?;

    // 5. Send transaction
    let transaction = build_and_sign_transaction(
        ArchMessage::new(
            &[create_account_ix, initialize_mint_ix],
            Some(authority_pubkey),
            client.get_best_block_hash()?,
        ),
        vec![authority_keypair, token_mint_keypair],
        BITCOIN_NETWORK,
    );

    let processed_txs = send_transactions_and_wait(vec![transaction]);
    if processed_txs[0].status != Status::Processed {
        return Err("Failed to create token mint".into());
    }

    println!("🎉 Token mint created: {}", token_mint_pubkey);
    
    Ok((authority_keypair, token_mint_pubkey))
}

pub fn create_token_account(
    client: &ArchRpcClient,
    token_mint_pubkey: arch_program::pubkey::Pubkey,
    owner_keypair: bitcoin::key::Keypair,
) -> Result<arch_program::pubkey::Pubkey, Box<dyn std::error::Error>> {
    
    let owner_pubkey = arch_program::pubkey::Pubkey::from_slice(
        &owner_keypair.x_only_public_key().0.serialize()
    );

    // 1. Create account keypair
    let (token_account_keypair, token_account_pubkey, _) = generate_new_keypair(BITCOIN_NETWORK);

    // 2. Create account on-chain
    let create_account_ix = arch_program::system_instruction::create_account(
        &owner_pubkey,
        &token_account_pubkey,
        arch_program::account::MIN_ACCOUNT_LAMPORTS,
        apl_token::state::Account::LEN as u64,
        &apl_token::id(),
    );

    // 3. Initialize token account
    let initialize_account_ix = apl_token::instruction::initialize_account(
        &apl_token::id(),
        &token_account_pubkey,
        &token_mint_pubkey,      // Which token this account holds
        &owner_pubkey,           // Who owns this account
    )?;

    // 4. Send transaction
    let transaction = build_and_sign_transaction(
        ArchMessage::new(
            &[create_account_ix, initialize_account_ix],
            Some(owner_pubkey),
            client.get_best_block_hash()?,
        ),
        vec![owner_keypair, token_account_keypair],
        BITCOIN_NETWORK,
    );

    let processed_txs = send_transactions_and_wait(vec![transaction]);
    if processed_txs[0].status != Status::Processed {
        return Err("Failed to create token account".into());
    }

    println!("💳 Token account created: {}", token_account_pubkey);
    Ok(token_account_pubkey)
}

pub fn mint_tokens(
    client: &ArchRpcClient,
    mint_pubkey: &arch_program::pubkey::Pubkey,
    account_pubkey: &arch_program::pubkey::Pubkey,
    authority_pubkey: &arch_program::pubkey::Pubkey,
    authority_keypair: bitcoin::key::Keypair,
    amount: u64,
) -> Result<(), Box<dyn std::error::Error>> {

    // Create mint instruction
    let mint_ix = apl_token::instruction::mint_to(
        &apl_token::id(),
        mint_pubkey,
        account_pubkey,
        authority_pubkey,
        &[],                    // No additional signers for single authority
        amount,                 // Amount to mint (in smallest units)
    )?;

    // Send transaction
    let transaction = build_and_sign_transaction(
        ArchMessage::new(
            &[mint_ix],
            Some(*authority_pubkey),
            client.get_best_block_hash()?,
        ),
        vec![authority_keypair],
        BITCOIN_NETWORK,
    );

    let processed_txs = send_transactions_and_wait(vec![transaction]);
    if processed_txs[0].status != Status::Processed {
        return Err("Failed to mint tokens".into());
    }

    println!("🪙 Minted {} tokens", amount);
    Ok(())
}

pub fn transfer_tokens(
    client: &ArchRpcClient,
    from_account: &arch_program::pubkey::Pubkey,
    to_account: &arch_program::pubkey::Pubkey,
    owner_pubkey: &arch_program::pubkey::Pubkey,
    owner_keypair: bitcoin::key::Keypair,
    amount: u64,
) -> Result<(), Box<dyn std::error::Error>> {

    // Create transfer instruction
    let transfer_ix = apl_token::instruction::transfer(
        &apl_token::id(),
        from_account,
        to_account,
        owner_pubkey,
        &[],                    // No additional signers
        amount,
    )?;

    // Send transaction
    let transaction = build_and_sign_transaction(
        ArchMessage::new(
            &[transfer_ix],
            Some(*owner_pubkey),
            client.get_best_block_hash()?,
        ),
        vec![owner_keypair],
        BITCOIN_NETWORK,
    );

    let processed_txs = send_transactions_and_wait(vec![transaction]);
    if processed_txs[0].status != Status::Processed {
        return Err("Failed to transfer tokens".into());
    }

    println!("📤 Transferred {} tokens", amount);
    Ok(())
}

pub fn burn_tokens(
    client: &ArchRpcClient,
    token_account: &arch_program::pubkey::Pubkey,
    mint_pubkey: &arch_program::pubkey::Pubkey,
    owner_pubkey: &arch_program::pubkey::Pubkey,
    owner_keypair: bitcoin::key::Keypair,
    amount: u64,
) -> Result<(), Box<dyn std::error::Error>> {

    let burn_ix = apl_token::instruction::burn(
        &apl_token::id(),
        token_account,
        mint_pubkey,
        owner_pubkey,
        &[],
        amount,
    )?;

    let transaction = build_and_sign_transaction(
        ArchMessage::new(
            &[burn_ix],
            Some(*owner_pubkey),
            client.get_best_block_hash()?,
        ),
        vec![owner_keypair],
        BITCOIN_NETWORK,
    );

    let processed_txs = send_transactions_and_wait(vec![transaction]);
    if processed_txs[0].status != Status::Processed {
        return Err("Failed to burn tokens".into());
    }

    println!("🔥 Burned {} tokens", amount);
    Ok(())
}

pub fn get_token_balance(token_account: arch_program::pubkey::Pubkey) -> Result<u64, Box<dyn std::error::Error>> {
    let account_info = read_account_info(token_account);
    let account_data = Account::unpack(&account_info.data)?;
    Ok(account_data.amount)
}

// Include the test module
#[cfg(test)]
mod test;
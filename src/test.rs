#[cfg(test)]
mod tests {
    use crate::*;
    use apl_token::state::{Mint, Account};
    use arch_program::program_pack::Pack;
    use arch_sdk::{generate_new_keypair, ArchRpcClient};
    use arch_test_sdk::{
        constants::{BITCOIN_NETWORK, NODE1_ADDRESS},
        helper::{create_and_fund_account_with_faucet, read_account_info},
    };

    fn setup_test_client() -> ArchRpcClient {
        ArchRpcClient::new(NODE1_ADDRESS)
    }

    #[test]
    fn test_create_token_mint() {
        let client = setup_test_client();
        let result = create_token_mint(&client);
        
        assert!(result.is_ok(), "Failed to create token mint: {:?}", result.err());
        
        let (authority_keypair, token_mint_pubkey) = result.unwrap();
        
        // Verify the mint account exists and has correct state
        let mint_account_info = read_account_info(token_mint_pubkey);
        assert!(!mint_account_info.data.is_empty(), "Mint account data should not be empty");
        
        let mint_data = Mint::unpack(&mint_account_info.data).unwrap();
        assert_eq!(mint_data.decimals, 9, "Mint should have 9 decimals");
        assert_eq!(mint_data.supply, 0, "Initial supply should be 0");
        
        let expected_authority = arch_program::pubkey::Pubkey::from_slice(
            &authority_keypair.x_only_public_key().0.serialize()
        );
        assert_eq!(mint_data.mint_authority, Some(expected_authority).into(), "Mint authority should match");
    }

    #[test]
    fn test_create_token_account() {
        let client = setup_test_client();
        
        // First create a mint
        let (_, token_mint_pubkey) = create_token_mint(&client).unwrap();
        
        // Create a user keypair
        let (user_keypair, _, _) = generate_new_keypair(BITCOIN_NETWORK);
        create_and_fund_account_with_faucet(&user_keypair, BITCOIN_NETWORK);
        
        // Create token account
        let result = create_token_account(&client, token_mint_pubkey, user_keypair);
        assert!(result.is_ok(), "Failed to create token account: {:?}", result.err());
        
        let token_account_pubkey = result.unwrap();
        
        // Verify the token account exists and has correct state
        let account_info = read_account_info(token_account_pubkey);
        assert!(!account_info.data.is_empty(), "Token account data should not be empty");
        
        let account_data = Account::unpack(&account_info.data).unwrap();
        assert_eq!(account_data.mint, token_mint_pubkey, "Token account should reference correct mint");
        assert_eq!(account_data.amount, 0, "Initial token balance should be 0");
        
        let expected_owner = arch_program::pubkey::Pubkey::from_slice(
            &user_keypair.x_only_public_key().0.serialize()
        );
        assert_eq!(account_data.owner, expected_owner, "Token account owner should match");
    }

    #[test]
    fn test_mint_tokens() {
        let client = setup_test_client();
        
        // Setup: create mint and token account
        let (authority_keypair, token_mint_pubkey) = create_token_mint(&client).unwrap();
        let (user_keypair, _, _) = generate_new_keypair(BITCOIN_NETWORK);
        create_and_fund_account_with_faucet(&user_keypair, BITCOIN_NETWORK);
        let token_account_pubkey = create_token_account(&client, token_mint_pubkey, user_keypair).unwrap();
        
        // Mint tokens
        let authority_pubkey = arch_program::pubkey::Pubkey::from_slice(
            &authority_keypair.x_only_public_key().0.serialize()
        );
        let mint_amount = 1_000_000_000; // 1000 tokens with 9 decimals
        
        let result = mint_tokens(
            &client,
            &token_mint_pubkey,
            &token_account_pubkey,
            &authority_pubkey,
            authority_keypair,
            mint_amount,
        );
        
        assert!(result.is_ok(), "Failed to mint tokens: {:?}", result.err());
        
        // Verify the token account balance
        let balance = get_token_balance(token_account_pubkey).unwrap();
        assert_eq!(balance, mint_amount, "Token balance should equal minted amount");
        
        // Verify mint supply increased
        let mint_account_info = read_account_info(token_mint_pubkey);
        let mint_data = Mint::unpack(&mint_account_info.data).unwrap();
        assert_eq!(mint_data.supply, mint_amount, "Mint supply should equal minted amount");
    }

    #[test]
    fn test_transfer_tokens() {
        let client = setup_test_client();
        
        // Setup: create mint, two users, and their token accounts
        let (authority_keypair, token_mint_pubkey) = create_token_mint(&client).unwrap();
        
        let (user1_keypair, user1_pubkey, _) = generate_new_keypair(BITCOIN_NETWORK);
        let (user2_keypair, _, _) = generate_new_keypair(BITCOIN_NETWORK);
        
        create_and_fund_account_with_faucet(&user1_keypair, BITCOIN_NETWORK);
        create_and_fund_account_with_faucet(&user2_keypair, BITCOIN_NETWORK);
        
        let user1_token_account = create_token_account(&client, token_mint_pubkey, user1_keypair).unwrap();
        let user2_token_account = create_token_account(&client, token_mint_pubkey, user2_keypair).unwrap();
        
        // Mint tokens to user1
        let authority_pubkey = arch_program::pubkey::Pubkey::from_slice(
            &authority_keypair.x_only_public_key().0.serialize()
        );
        let initial_amount = 1_000_000_000; // 1000 tokens
        mint_tokens(
            &client,
            &token_mint_pubkey,
            &user1_token_account,
            &authority_pubkey,
            authority_keypair,
            initial_amount,
        ).unwrap();
        
        // Transfer tokens from user1 to user2
        let transfer_amount = 500_000_000; // 500 tokens
        let result = transfer_tokens(
            &client,
            &user1_token_account,
            &user2_token_account,
            &user1_pubkey,
            user1_keypair,
            transfer_amount,
        );
        
        assert!(result.is_ok(), "Failed to transfer tokens: {:?}", result.err());
        
        // Verify balances
        let user1_balance = get_token_balance(user1_token_account).unwrap();
        let user2_balance = get_token_balance(user2_token_account).unwrap();
        
        assert_eq!(user1_balance, initial_amount - transfer_amount, "User1 balance should be reduced by transfer amount");
        assert_eq!(user2_balance, transfer_amount, "User2 balance should equal transfer amount");
    }

    #[test]
    fn test_burn_tokens() {
        let client = setup_test_client();
        
        // Setup: create mint, user, and token account with tokens
        let (authority_keypair, token_mint_pubkey) = create_token_mint(&client).unwrap();
        let (user_keypair, user_pubkey, _) = generate_new_keypair(BITCOIN_NETWORK);
        create_and_fund_account_with_faucet(&user_keypair, BITCOIN_NETWORK);
        let token_account_pubkey = create_token_account(&client, token_mint_pubkey, user_keypair).unwrap();
        
        // Mint tokens
        let authority_pubkey = arch_program::pubkey::Pubkey::from_slice(
            &authority_keypair.x_only_public_key().0.serialize()
        );
        let initial_amount = 1_000_000_000; // 1000 tokens
        mint_tokens(
            &client,
            &token_mint_pubkey,
            &token_account_pubkey,
            &authority_pubkey,
            authority_keypair,
            initial_amount,
        ).unwrap();
        
        // Burn tokens
        let burn_amount = 100_000_000; // 100 tokens
        let result = burn_tokens(
            &client,
            &token_account_pubkey,
            &token_mint_pubkey,
            &user_pubkey,
            user_keypair,
            burn_amount,
        );
        
        assert!(result.is_ok(), "Failed to burn tokens: {:?}", result.err());
        
        // Verify balance reduced
        let balance = get_token_balance(token_account_pubkey).unwrap();
        assert_eq!(balance, initial_amount - burn_amount, "Balance should be reduced by burn amount");
        
        // Verify mint supply reduced
        let mint_account_info = read_account_info(token_mint_pubkey);
        let mint_data = Mint::unpack(&mint_account_info.data).unwrap();
        assert_eq!(mint_data.supply, initial_amount - burn_amount, "Mint supply should be reduced by burn amount");
    }

    #[test]
    fn test_get_token_balance() {
        let client = setup_test_client();
        
        // Setup: create mint, user, and token account
        let (authority_keypair, token_mint_pubkey) = create_token_mint(&client).unwrap();
        let (user_keypair, _, _) = generate_new_keypair(BITCOIN_NETWORK);
        create_and_fund_account_with_faucet(&user_keypair, BITCOIN_NETWORK);
        let token_account_pubkey = create_token_account(&client, token_mint_pubkey, user_keypair).unwrap();
        
        // Initial balance should be 0
        let initial_balance = get_token_balance(token_account_pubkey).unwrap();
        assert_eq!(initial_balance, 0, "Initial balance should be 0");
        
        // Mint some tokens
        let authority_pubkey = arch_program::pubkey::Pubkey::from_slice(
            &authority_keypair.x_only_public_key().0.serialize()
        );
        let mint_amount = 500_000_000; // 500 tokens
        mint_tokens(
            &client,
            &token_mint_pubkey,
            &token_account_pubkey,
            &authority_pubkey,
            authority_keypair,
            mint_amount,
        ).unwrap();
        
        // Balance should now equal minted amount
        let balance = get_token_balance(token_account_pubkey).unwrap();
        assert_eq!(balance, mint_amount, "Balance should equal minted amount");
    }

    #[test]
    fn test_complete_token_lifecycle() {
        let client = setup_test_client();
        let result = run_token_lifecycle(&client);
        
        assert!(result.is_ok(), "Token lifecycle should complete successfully: {:?}", result.err());
    }

    #[test]
    fn test_transfer_insufficient_balance() {
        let client = setup_test_client();
        
        // Setup: create mint, two users, and their token accounts
        let (_, token_mint_pubkey) = create_token_mint(&client).unwrap();
        let (user1_keypair, user1_pubkey, _) = generate_new_keypair(BITCOIN_NETWORK);
        let (user2_keypair, _, _) = generate_new_keypair(BITCOIN_NETWORK);
        
        create_and_fund_account_with_faucet(&user1_keypair, BITCOIN_NETWORK);
        create_and_fund_account_with_faucet(&user2_keypair, BITCOIN_NETWORK);
        
        let user1_token_account = create_token_account(&client, token_mint_pubkey, user1_keypair).unwrap();
        let user2_token_account = create_token_account(&client, token_mint_pubkey, user2_keypair).unwrap();
        
        // Try to transfer tokens when user1 has 0 balance
        let transfer_amount = 100_000_000; // 100 tokens
        let result = transfer_tokens(
            &client,
            &user1_token_account,
            &user2_token_account,
            &user1_pubkey,
            user1_keypair,
            transfer_amount,
        );
        
        // This should fail due to insufficient balance
        assert!(result.is_err(), "Transfer should fail with insufficient balance");
    }

    #[test]
    fn test_burn_insufficient_balance() {
        let client = setup_test_client();
        
        // Setup: create mint, user, and token account with no tokens
        let (_, token_mint_pubkey) = create_token_mint(&client).unwrap();
        let (user_keypair, user_pubkey, _) = generate_new_keypair(BITCOIN_NETWORK);
        create_and_fund_account_with_faucet(&user_keypair, BITCOIN_NETWORK);
        let token_account_pubkey = create_token_account(&client, token_mint_pubkey, user_keypair).unwrap();
        
        // Try to burn tokens when account has 0 balance
        let burn_amount = 100_000_000; // 100 tokens
        let result = burn_tokens(
            &client,
            &token_account_pubkey,
            &token_mint_pubkey,
            &user_pubkey,
            user_keypair,
            burn_amount,
        );
        
        // This should fail due to insufficient balance
        assert!(result.is_err(), "Burn should fail with insufficient balance");
    }
} 
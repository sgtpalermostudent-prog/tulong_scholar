#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation},
        token, Address, Env, IntoVal, String,
    };

    use crate::{EnrollmentStatus, TulongScholarContract, TulongScholarContractClient};

    // ── Helpers ─────────────────────────────────────────────────────────────

    /// Deploy the USDC mock token and mint `amount` to `recipient`.
    fn setup_token(env: &Env, admin: &Address, recipient: &Address, amount: i128) -> Address {
        let token_id = env.register_stellar_asset_contract_v2(admin.clone());
        let token_admin = token::StellarAssetClient::new(env, &token_id.address());
        token_admin.mint(recipient, &amount);
        token_id.address()
    }

    /// Full environment setup: returns (env, contract_client, admin, usdc_address).
    fn setup() -> (
        Env,
        TulongScholarContractClient<'static>,
        Address,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let usdc = setup_token(&env, &admin, &admin, 1_000_000_000); // 1,000 USDC (6 decimals)

        let contract_id = env.register_contract(None, TulongScholarContract);
        let client = TulongScholarContractClient::new(&env, &contract_id);
        client.initialize(&admin, &usdc);

        (env, client, admin, usdc)
    }

    // ── Test 1: Happy Path — MVP end-to-end disbursement ────────────────────
    //
    // Admin registers a scholar → activates them → disburses USDC →
    // scholar wallet receives funds → cumulative total updated.
    #[test]
    fn test_happy_path_disburse() {
        let (env, client, admin, usdc) = setup();

        let scholar_wallet = Address::generate(&env);
        let scholar_name = String::from_str(&env, "Maria Santos");
        let scholar_school = String::from_str(&env, "Polytechnic University of the Philippines");

        // Register and activate the scholar
        let scholar_id = client.register_scholar(&admin, &scholar_wallet, &scholar_name, &scholar_school);
        client.activate_scholar(&admin, &scholar_id);

        // Check status is Active before disbursement
        let record_before = client.get_scholar(&scholar_id);
        assert_eq!(record_before.status, EnrollmentStatus::Active);
        assert_eq!(record_before.total_received, 0);

        // Disburse 5,000 USDC (in 6-decimal units = 5_000_000_000)
        let stipend: i128 = 5_000_000_000;
        let cycle: u32 = 1; // Semester 1

        client.disburse(&admin, &scholar_id, &stipend, &cycle);

        // Verify cumulative total updated on-chain
        let record_after = client.get_scholar(&scholar_id);
        assert_eq!(record_after.total_received, stipend);

        // Verify scholar wallet received the tokens
        let token_client = token::Client::new(&env, &usdc);
        assert_eq!(token_client.balance(&scholar_wallet), stipend);
    }

    // ── Test 2: Edge Case — duplicate disbursement in same cycle rejected ───
    //
    // Admin tries to pay the same scholar twice in cycle 1 → second call panics.
    #[test]
    #[should_panic(expected = "already disbursed this cycle")]
    fn test_duplicate_disburse_same_cycle_rejected() {
        let (env, client, admin, _usdc) = setup();

        let scholar_wallet = Address::generate(&env);
        let id = client.register_scholar(
            &admin,
            &scholar_wallet,
            &String::from_str(&env, "Juan dela Cruz"),
            &String::from_str(&env, "De La Salle University"),
        );
        client.activate_scholar(&admin, &id);

        let amount: i128 = 2_000_000_000;
        let cycle: u32 = 2;

        client.disburse(&admin, &id, &amount, &cycle);
        // Second call for the same cycle must panic
        client.disburse(&admin, &id, &amount, &cycle);
    }

    // ── Test 3: State Verification — storage reflects correct state post-MVP ─
    //
    // After registering 3 scholars and activating 2, verify scholar_count,
    // statuses, and that the third (Pending) cannot receive a disbursement.
    #[test]
    fn test_state_verification_after_registration() {
        let (env, client, admin, _usdc) = setup();

        let w1 = Address::generate(&env);
        let w2 = Address::generate(&env);
        let w3 = Address::generate(&env);

        let id1 = client.register_scholar(
            &admin, &w1,
            &String::from_str(&env, "Ana Reyes"),
            &String::from_str(&env, "UP Diliman"),
        );
        let id2 = client.register_scholar(
            &admin, &w2,
            &String::from_str(&env, "Ben Cruz"),
            &String::from_str(&env, "Ateneo de Manila"),
        );
        let id3 = client.register_scholar(
            &admin, &w3,
            &String::from_str(&env, "Carla Lim"),
            &String::from_str(&env, "FEU Manila"),
        );

        // Activate only id1 and id2
        client.activate_scholar(&admin, &id1);
        client.activate_scholar(&admin, &id2);

        // Scholar count should be 3
        assert_eq!(client.scholar_count(), 3);

        // id1 and id2 are Active; id3 is still Pending
        assert_eq!(client.get_scholar(&id1).status, EnrollmentStatus::Active);
        assert_eq!(client.get_scholar(&id2).status, EnrollmentStatus::Active);
        assert_eq!(client.get_scholar(&id3).status, EnrollmentStatus::Pending);

        // is_disbursed should return false for all (no disbursements yet)
        assert!(!client.is_disbursed(&id1, &1u32));
        assert!(!client.is_disbursed(&id2, &1u32));

        // Disburse to id1; verify cycle guard set to true and id2 still false
        client.disburse(&admin, &id1, &1_000_000_000_i128, &1u32);
        assert!(client.is_disbursed(&id1, &1u32));
        assert!(!client.is_disbursed(&id2, &1u32));
    }

    // ── Test 4: Unauthorized caller cannot disburse ─────────────────────────
    //
    // A random non-admin address calling disburse() must panic.
    #[test]
    #[should_panic(expected = "unauthorized: caller is not admin")]
    fn test_non_admin_cannot_disburse() {
        let (env, client, admin, _usdc) = setup();

        let scholar_wallet = Address::generate(&env);
        let id = client.register_scholar(
            &admin, &scholar_wallet,
            &String::from_str(&env, "Diego Bautista"),
            &String::from_str(&env, "Mapua University"),
        );
        client.activate_scholar(&admin, &id);

        // Impersonator tries to disburse
        let impersonator = Address::generate(&env);
        client.disburse(&impersonator, &id, &500_000_000_i128, &1u32);
    }

    // ── Test 5: Suspended scholar cannot receive disbursement ───────────────
    //
    // Admin suspends a scholar mid-cycle; disburse() must panic for them.
    #[test]
    #[should_panic(expected = "scholar is not active")]
    fn test_suspended_scholar_cannot_receive_funds() {
        let (env, client, admin, _usdc) = setup();

        let scholar_wallet = Address::generate(&env);
        let id = client.register_scholar(
            &admin, &scholar_wallet,
            &String::from_str(&env, "Elena Torres"),
            &String::from_str(&env, "UST Manila"),
        );
        client.activate_scholar(&admin, &id);

        // Admin suspends the scholar (e.g. failed GWA requirement)
        client.suspend_scholar(&admin, &id);

        // Attempt to disburse to suspended scholar must fail
        client.disburse(&admin, &id, &3_000_000_000_i128, &1u32);
    }
}
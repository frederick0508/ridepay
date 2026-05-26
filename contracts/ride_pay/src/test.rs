#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::{Address as _, Ledger, LedgerInfo},
        Address, Env, String,
    };

    use crate::{RidePayContract, RidePayContractClient};

    // ── Shared test setup ─────────────────────────────────────────────────────

    /// Deploys RidePay and both mock token contracts, funds the rider with
    /// USDC and seeds the contract with a loyalty token reserve.
    /// Returns (env, client, admin, operator, rider).
    fn setup() -> (
        Env,
        RidePayContractClient<'static>,
        Address,
        Address,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths(); // All require_auth() calls pass in test context

        let admin = Address::generate(&env);
        let operator = Address::generate(&env);
        let rider = Address::generate(&env);

        // Deploy the RidePay contract
        let contract_id = env.register(RidePayContract, ());
        let client = RidePayContractClient::new(&env, &contract_id);

        // Deploy mock USDC and loyalty token contracts
        let usdc_asset = env.register_stellar_asset_contract_v2(admin.clone());
        let loyalty_asset = env.register_stellar_asset_contract_v2(admin.clone());

        let usdc_addr = usdc_asset.address();
        let loyalty_addr = loyalty_asset.address();

        // Fund rider with USDC (enough for multiple rides)
        let usdc_admin = soroban_sdk::token::StellarAssetClient::new(&env, &usdc_addr);
        usdc_admin.mint(&rider, &1_000_000_i128); // 0.1 USDC

        // Seed the RidePay contract with loyalty tokens to distribute as rewards
        let loyalty_admin =
            soroban_sdk::token::StellarAssetClient::new(&env, &loyalty_addr);
        loyalty_admin.mint(&contract_id, &1_000_i128);

        // Initialise RidePay
        client.initialize(&admin, &usdc_addr, &loyalty_addr);

        (env, client, admin, operator, rider)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 1 — Happy path: end-to-end MVP transaction executes successfully
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_happy_path_settle_ride() {
        let (env, client, _admin, operator, rider) = setup();

        // Retrieve token addresses to inspect balances after settlement
        let usdc_addr: Address = env
            .storage()
            .instance()
            .get(&crate::DataKey::UsdcToken)
            .unwrap();
        let loyalty_addr: Address = env
            .storage()
            .instance()
            .get(&crate::DataKey::LoyaltyToken)
            .unwrap();

        let usdc = soroban_sdk::token::Client::new(&env, &usdc_addr);
        let loyalty = soroban_sdk::token::Client::new(&env, &loyalty_addr);

        let fare: i128 = 100_000; // generic micro-fare amount in USDC stroops
        let ride_id = String::from_str(&env, "ride-001");

        let record = client.settle_ride(&ride_id, &rider, &operator, &fare);

        // Rider's USDC balance decreased by exact fare
        assert_eq!(usdc.balance(&rider), 1_000_000 - fare);
        // Operator received the USDC fare
        assert_eq!(usdc.balance(&operator), fare);
        // Operator received 1 loyalty token
        assert_eq!(loyalty.balance(&operator), 1);
        // Returned record captures correct fare amount
        assert_eq!(record.usdc_amount, fare);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 2 — Edge case: duplicate ride_id is rejected (idempotency guard)
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    #[should_panic(expected = "ride already settled")]
    fn test_duplicate_ride_id_rejected() {
        let (env, client, _admin, operator, rider) = setup();

        let fare: i128 = 100_000;
        let ride_id = String::from_str(&env, "ride-dup");

        // First call must succeed
        client.settle_ride(&ride_id, &rider, &operator, &fare);
        // Second call with the same ride_id must panic
        client.settle_ride(&ride_id, &rider, &operator, &fare);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 3 — State verification: stored record matches all input values
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_ride_record_stored_correctly() {
        let (env, client, _admin, operator, rider) = setup();

        // Fix ledger timestamp for deterministic assertion
        env.ledger().set(LedgerInfo {
            timestamp: 1_700_000_000,
            protocol_version: 20,
            sequence_number: 100,
            network_id: Default::default(),
            base_reserve: 5_000_000,
            min_temp_entry_ttl: 0,
            min_persistent_entry_ttl: 0,
            max_entry_ttl: 1_000_000,
        });

        let fare: i128 = 100_000;
        let ride_id = String::from_str(&env, "ride-state-check");

        client.settle_ride(&ride_id, &rider, &operator, &fare);

        // Read the record back and verify every field
        let record = client.get_ride(&ride_id);
        assert_eq!(record.operator, operator);
        assert_eq!(record.rider, rider);
        assert_eq!(record.usdc_amount, fare);
        assert_eq!(record.settled_at, 1_700_000_000);

        // Ride counter must reflect exactly one settled ride
        assert_eq!(client.ride_count(), 1);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 4 — Edge case: zero-amount fare is rejected before any transfer
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    #[should_panic(expected = "fare amount must be greater than zero")]
    fn test_zero_fare_rejected() {
        let (env, client, _admin, operator, rider) = setup();

        let ride_id = String::from_str(&env, "ride-zero");
        // Must panic before any token transfer occurs
        client.settle_ride(&ride_id, &rider, &operator, &0_i128);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 5 — Ride counter increments correctly across multiple settlements
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_ride_counter_increments() {
        let (env, client, _admin, operator, rider) = setup();

        for i in 0..3_u32 {
            let id = String::from_str(&env, &format!("ride-{i}"));
            client.settle_ride(&id, &rider, &operator, &100_000_i128);
        }

        // Counter must equal the number of distinct rides settled
        assert_eq!(client.ride_count(), 3);
    }
}

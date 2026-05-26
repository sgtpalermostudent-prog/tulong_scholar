#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, String, Symbol, Vec,
};

// ─────────────────────────────────────────────
// Storage Keys
// ─────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    UsdcToken,
    ScholarCount,
    Scholar(u32),    // scholar id → ScholarRecord
    Disbursed(u32),  // scholar id → bool (has been paid this cycle)
}

// ─────────────────────────────────────────────
// Data Types
// ─────────────────────────────────────────────

/// Enrollment status of a scholarship recipient
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum EnrollmentStatus {
    Pending,   // registered but not yet verified
    Active,    // verified; eligible to receive disbursements
    Suspended, // temporarily blocked from receiving funds
}

/// A single scholarship recipient record
#[contracttype]
#[derive(Clone)]
pub struct ScholarRecord {
    pub id: u32,
    pub wallet: Address,
    pub name: String,
    pub school: String,
    pub status: EnrollmentStatus,
    pub total_received: i128, // cumulative USDC received in stroops-equivalent
}

// ─────────────────────────────────────────────
// Contract
// ─────────────────────────────────────────────

#[contract]
pub struct TulongScholarContract;

#[contractimpl]
impl TulongScholarContract {
    // ── Setup ──────────────────────────────────────────────────────────────

    /// Initialize the contract with an admin address and the USDC token address.
    /// Admin is typically the sponsoring foundation or university bursar account.
    pub fn initialize(env: Env, admin: Address, usdc_token: Address) {
        // Prevent double-initialization
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        // Require admin's auth so only they can bootstrap the contract
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::UsdcToken, &usdc_token);
        // Start scholar count at 0
        env.storage().instance().set(&DataKey::ScholarCount, &0u32);
    }

    // ── Scholar Registry ────────────────────────────────────────────────────

    /// Register a new scholarship recipient.
    /// Only the admin (foundation) can call this.
    /// Returns the newly assigned scholar ID.
    pub fn register_scholar(
        env: Env,
        admin: Address,
        wallet: Address,
        name: String,
        school: String,
    ) -> u32 {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        // Fetch and increment scholar counter
        let count: u32 = env.storage().instance().get(&DataKey::ScholarCount).unwrap_or(0);
        let new_id = count + 1;

        let record = ScholarRecord {
            id: new_id,
            wallet,
            name,
            school,
            status: EnrollmentStatus::Pending,
            total_received: 0,
        };

        env.storage().persistent().set(&DataKey::Scholar(new_id), &record);
        env.storage().instance().set(&DataKey::ScholarCount, &new_id);

        // Emit event for off-chain indexers / frontend to track
        env.events().publish(
            (Symbol::new(&env, "scholar_registered"),),
            new_id,
        );

        new_id
    }

    /// Activate a scholar's status so they can receive disbursements.
    /// Admin only — called after manual KYC / enrollment verification.
    pub fn activate_scholar(env: Env, admin: Address, scholar_id: u32) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut record: ScholarRecord = Self::get_scholar_record(&env, scholar_id);
        record.status = EnrollmentStatus::Active;
        env.storage().persistent().set(&DataKey::Scholar(scholar_id), &record);
    }

    /// Suspend a scholar (e.g., dropped enrollment, grade threshold not met).
    /// Admin only.
    pub fn suspend_scholar(env: Env, admin: Address, scholar_id: u32) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut record: ScholarRecord = Self::get_scholar_record(&env, scholar_id);
        record.status = EnrollmentStatus::Suspended;
        env.storage().persistent().set(&DataKey::Scholar(scholar_id), &record);
    }

    // ── Disbursement ────────────────────────────────────────────────────────

    /// Disburse USDC stipend to a single active scholar.
    ///
    /// Flow:
    ///   Admin calls disburse → contract checks Active status + no duplicate in cycle
    ///   → token.transfer from admin's wallet to scholar's wallet
    ///   → marks scholar as disbursed for this cycle
    ///   → updates cumulative total
    ///
    /// `cycle_id` is an integer the admin supplies (e.g. semester number) to
    /// prevent double-payment within the same cycle.
    pub fn disburse(
        env: Env,
        admin: Address,
        scholar_id: u32,
        amount: i128,
        cycle_id: u32,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if amount <= 0 {
            panic!("amount must be positive");
        }

        let mut record: ScholarRecord = Self::get_scholar_record(&env, scholar_id);

        // Enforce Active status
        if record.status != EnrollmentStatus::Active {
            panic!("scholar is not active");
        }

        // Build a per-cycle disbursement guard key
        // Key = (scholar_id * 1_000_000) + cycle_id  (unique per scholar+cycle combo)
        let cycle_key = DataKey::Disbursed(scholar_id * 1_000_000 + cycle_id);
        if env.storage().persistent().has(&cycle_key) {
            panic!("already disbursed this cycle");
        }

        // Pull USDC token contract and transfer from admin → scholar
        let usdc: Address = env
            .storage()
            .instance()
            .get(&DataKey::UsdcToken)
            .unwrap();
        let token_client = token::Client::new(&env, &usdc);
        token_client.transfer(&admin, &record.wallet, &amount);

        // Mark disbursed for this cycle
        env.storage().persistent().set(&cycle_key, &true);

        // Update cumulative received
        record.total_received += amount;
        env.storage().persistent().set(&DataKey::Scholar(scholar_id), &record);

        env.events().publish(
            (Symbol::new(&env, "disbursed"),),
            (scholar_id, amount, cycle_id),
        );
    }

    /// Batch-disburse the same amount to a list of scholar IDs in one transaction.
    /// All scholars must be Active. Any failure halts the entire batch (atomicity).
    pub fn batch_disburse(
        env: Env,
        admin: Address,
        scholar_ids: Vec<u32>,
        amount_each: i128,
        cycle_id: u32,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if amount_each <= 0 {
            panic!("amount must be positive");
        }

        for scholar_id in scholar_ids.iter() {
            // Reuse single-disburse logic via direct storage + token call
            let mut record: ScholarRecord = Self::get_scholar_record(&env, scholar_id);
            if record.status != EnrollmentStatus::Active {
                panic!("scholar not active");
            }
            let cycle_key = DataKey::Disbursed(scholar_id * 1_000_000 + cycle_id);
            if env.storage().persistent().has(&cycle_key) {
                panic!("already disbursed this cycle");
            }

            let usdc: Address = env
                .storage()
                .instance()
                .get(&DataKey::UsdcToken)
                .unwrap();
            let token_client = token::Client::new(&env, &usdc);
            token_client.transfer(&admin, &record.wallet, &amount_each);

            env.storage().persistent().set(&cycle_key, &true);
            record.total_received += amount_each;
            env.storage().persistent().set(&DataKey::Scholar(scholar_id), &record);
        }

        env.events().publish(
            (Symbol::new(&env, "batch_disbursed"),),
            (scholar_ids.len(), amount_each, cycle_id),
        );
    }

    // ── Queries ─────────────────────────────────────────────────────────────

    /// Return the full record for a given scholar ID.
    pub fn get_scholar(env: Env, scholar_id: u32) -> ScholarRecord {
        Self::get_scholar_record(&env, scholar_id)
    }

    /// Return the total number of registered scholars.
    pub fn scholar_count(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::ScholarCount).unwrap_or(0)
    }

    /// Check whether a scholar has already been paid in a given cycle.
    pub fn is_disbursed(env: Env, scholar_id: u32, cycle_id: u32) -> bool {
        let cycle_key = DataKey::Disbursed(scholar_id * 1_000_000 + cycle_id);
        env.storage().persistent().has(&cycle_key)
    }

    /// Return the admin address (foundation/bursar).
    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    // ── Internal Helpers ────────────────────────────────────────────────────

    fn assert_admin(env: &Env, caller: &Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if *caller != admin {
            panic!("unauthorized: caller is not admin");
        }
    }

    fn get_scholar_record(env: &Env, scholar_id: u32) -> ScholarRecord {
        env.storage()
            .persistent()
            .get(&DataKey::Scholar(scholar_id))
            .unwrap_or_else(|| panic!("scholar not found"))
    }
}

mod test;
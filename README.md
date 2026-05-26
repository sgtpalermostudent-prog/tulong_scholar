# TulongScholar 🎓

![TulongScholar](contracts/tulong_scholar/STELLAR.png)

> **On-chain scholarship disbursement for Filipino university students — no bank account required.**

---

## Problem

A CHED scholar at Polytechnic University of the Philippines (PUP) Sta. Mesa loses ₱2,000–₱5,000 per semester to bank transfer delays, intermediary cuts, and manual processing errors when foundations disburse monthly stipends through traditional bank channels — with some students waiting 6–8 weeks past the semester start to receive funds they need for tuition and board.

## Solution

TulongScholar lets a sponsoring foundation register student wallets directly on-chain. When disbursement day arrives, the admin calls a single `disburse` (or `batch_disburse`) transaction that atomically transfers USDC from the foundation's Stellar wallet to each scholar's wallet in under 5 seconds — no bank, no cut, no waiting — with an immutable on-chain record of every peso sent.

---

## Stellar Features Used

| Feature | Why |
|---|---|
| **USDC transfers** | Stablecoin pegged to USD/PHP anchor; scholars receive real value without volatility |
| **Soroban smart contracts** | Enforces enrollment status, prevents duplicate disbursements per cycle, and provides auditable on-chain records |
| **Trustlines** | Scholar wallets establish USDC trustlines before receiving funds |
| **Custom token (optional)** | Foundation can issue a non-transferable "ScholarToken" for credential verification |

---

## Target Users

| | Detail |
|---|---|
| **Who** | 18–25 year old university students receiving government or private scholarships; middle-income Filipino families dependent on stipends |
| **Where** | Philippines — Metro Manila (PUP, TUP, PLM), Cebu, Davao |
| **Why they care** | Late stipends = missed tuition deadlines, borrowed money for boarding, dropped units |
| **Secondary users** | CHED, DOST-SEI, private foundations, school registrars |

---

## Core Feature (MVP)

```
Admin (Foundation) calls:
  register_scholar(wallet, name, school)   → scholar_id = 1
  activate_scholar(scholar_id = 1)         → status = Active

Admin calls:
  disburse(scholar_id=1, amount=5_000_000_000, cycle_id=1)

On-chain:
  → Checks status == Active
  → Checks no prior payment in cycle 1
  → token.transfer(admin → scholar.wallet, 5000 USDC)
  → Marks Disbursed[scholar_id=1][cycle_id=1] = true
  → Updates total_received += 5000 USDC

Result:
  Scholar's Freighter wallet shows +5,000 USDC within 5 seconds.
  Admin dashboard shows immutable audit log of all disbursements.
```

**Demo time: ~90 seconds** ✅

---

## Why This Wins

TulongScholar solves a real, named problem affecting millions of Filipino students — CHED alone administers 400,000+ scholarship slots annually. It uses Stellar's USDC anchor rails to bypass slow interbank transfers, and the Soroban contract enforces cycle-level disbursement guards that eliminate the double-payment errors common in manual payroll systems. Judges see real users, real money, and a government-scalable use case.

---

## Optional Edge: AI Integration

Integrate an AI eligibility screener that reads a student's uploaded enrollment certificate (PDF/image) via OCR and auto-calls `activate_scholar` once enrollment is verified — removing the manual verification bottleneck entirely.

---

## Project Structure

```
tulong_scholar/
├── src/
│   ├── lib.rs       ← Soroban contract (register, activate, disburse)
│   └── test.rs      ← 5 unit tests
├── Cargo.toml
└── README.md
```

---

## Timeline

| Phase | Duration |
|---|---|
| Contract + tests | Day 1 (4 hrs) |
| Deploy to testnet | Day 1 (1 hr) |
| Frontend (Freighter + React) | Day 2 (6 hrs) |
| Demo polish | Day 2 (2 hrs) |

---

## Prerequisites

- [Rust](https://rustup.rs/) ≥ 1.75
- `rustup target add wasm32-unknown-unknown`
- Stellar CLI ≥ 21.0.0 — `cargo install --locked stellar-cli --features opt`
- [Freighter Wallet](https://freighter.app) set to **Testnet**

---

## Build

```bash
cargo build --target wasm32-unknown-unknown --release
```

Output: `target/wasm32-unknown-unknown/release/tulong_scholar.wasm`

---

## Test

```bash
cargo test
```

Expected:
```
running 5 tests
test tests::test_happy_path_disburse ... ok
test tests::test_duplicate_disburse_same_cycle_rejected ... ok
test tests::test_state_verification_after_registration ... ok
test tests::test_non_admin_cannot_disburse ... ok
test tests::test_suspended_scholar_cannot_receive_funds ... ok
test result: ok. 5 passed; 0 failed
```

---

## Deploy to Testnet

```bash
# 1. Generate & fund identity (first time only)
stellar keys generate --global my-key --network testnet
stellar keys fund my-key --network testnet

# 2. Deploy contract
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/tulong_scholar.wasm \
  --source my-key \
  --network testnet

# Copy the Contract ID (starts with C...)
```

Verify on Stellar Expert:
```
https://stellar.expert/explorer/testnet/contract/<YOUR_CONTRACT_ID>
```

---

## Sample CLI Invocations

```bash
# Initialize
stellar contract invoke --id <CONTRACT_ID> --source my-key --network testnet \
  -- initialize \
  --admin <ADMIN_ADDRESS> \
  --usdc_token <USDC_TOKEN_ADDRESS>

# Register a scholar
stellar contract invoke --id <CONTRACT_ID> --source my-key --network testnet \
  -- register_scholar \
  --admin <ADMIN_ADDRESS> \
  --wallet <STUDENT_WALLET_ADDRESS> \
  --name "Maria Santos" \
  --school "Polytechnic University of the Philippines"

# Activate scholar (id = 1)
stellar contract invoke --id <CONTRACT_ID> --source my-key --network testnet \
  -- activate_scholar \
  --admin <ADMIN_ADDRESS> \
  --scholar_id 1

# Disburse 5000 USDC to scholar 1, Semester 1
stellar contract invoke --id <CONTRACT_ID> --source my-key --network testnet \
  -- disburse \
  --admin <ADMIN_ADDRESS> \
  --scholar_id 1 \
  --amount 5000000000 \
  --cycle_id 1

# Check disbursement status
stellar contract invoke --id <CONTRACT_ID> --source my-key --network testnet \
  -- is_disbursed \
  --scholar_id 1 \
  --cycle_id 1
```

---

## Vision & Purpose

The Philippines has one of the largest scholarship ecosystems in Southeast Asia — yet most disbursement infrastructure relies on G-Cash forwards, provincial bank transfers, or physical checks. TulongScholar proves that a Soroban contract + Freighter wallet can replace the entire disbursement pipeline with one auditable, near-zero-fee, 5-second transaction — giving scholars their money when they need it, not 6 weeks later.

---

## Resources

| Resource | Link |
|---|---|
| Stellar Docs | https://developers.stellar.org |
| Soroban SDK | https://docs.rs/soroban-sdk |
| Stellar CLI | https://developers.stellar.org/docs/tools/stellar-cli |
| Freighter Wallet | https://freighter.app |
| Stellar Expert (Testnet) | https://stellar.expert/explorer/testnet |
| Rise In Program | https://www.risein.com/programs/stellar-philippines-unitour-university-of-east-caloocan |
| Bootcamp Reference | https://github.com/armlynobinguar/Stellar-Bootcamp-2026 |

---

## License

MIT

# RidePay

> USDC micropayment settlement for informal transport operators — built on Stellar Soroban.

---

## Problem

Informal transport operators (tuk-tuks, mototaxis, jeepneys, boda bodas) lose 15–30% of daily income because riders cannot produce exact small-denomination change and no existing digital payment option is economically viable at sub-$1 fare sizes. Traditional payment processors charge flat fees that exceed the fare margin entirely.

## Solution

RidePay lets an operator display a dynamic QR code encoding the exact fare. The rider scans it with any Stellar-compatible wallet and sends USDC. A Soroban smart contract settles the transfer in under 5 seconds at a fee below $0.00001. At day's end the operator swaps USDC for local fiat through a regional anchor via Stellar's built-in DEX.

---

## How the settlement works

```
Rider wallet                 RidePay contract              Operator wallet
     │                             │                              │
     │── approve(contract, fare) ──▶│  (done once in mobile app)  │
     │                             │                              │
     │── settle_ride(ride_id) ────▶│                              │
     │                             │── transfer_from(rider) ─────▶│  USDC fare
     │                             │── transfer(contract) ────────▶│  1 loyalty token
     │                             │── persist RideRecord          │
     │                             │── emit RideSettled event      │
     │◀─────────────────────────── confirmed ─────────────────────▶│
```

Key design decisions:
- `transfer_from` is used for USDC because the rider holds the tokens. The rider calls `approve` on the USDC contract once (via wallet deep-link) authorising this contract as spender.
- `transfer` is used for loyalty tokens because the contract itself holds its own reserve, minted by the admin at deploy time.
- Both the rider and operator must sign `settle_ride` — prevents either party from settling without the other's consent.
- The `ride_id` idempotency guard ensures a network retry can never double-charge a rider.

---

## Timeline

| Week | Milestone |
|------|-----------|
| 1 | Soroban contract deployed to testnet; CLI settle flow verified |
| 2 | Mobile-first PWA: QR generation, wallet deep-link, settlement confirmation screen |
| 3 | Loyalty token integration; anchor swap UI with DEX routing |
| 4 | End-to-end demo recorded; operator and rider onboarding flow complete |

---

## Stellar Features Used

| Feature | Purpose |
|---------|---------|
| USDC transfers (`transfer_from`) | Pull exact fare from rider with prior approval |
| Soroban smart contract | On-chain settlement, idempotency, events |
| Custom loyalty token (`transfer`) | 1 reward token per ride from contract reserve |
| Trustlines | Rider and operator establish USDC trustline before first use |
| Built-in DEX | End-of-day USDC → local fiat via regional anchor |

---

## Vision and Purpose

Informal transport workers are among the most cash-dependent workers in the global urban economy. RidePay gives any operator a bank-account-free way to accept exact digital payment, build a verifiable on-chain earnings history, and access savings and DeFi products — starting with a single printed QR code. The contract is region-agnostic: any local USDC anchor and any Stellar-compatible wallet work out of the box.

---

## Prerequisites

```bash
# 1. Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Wasm compilation target
rustup target add wasm32-unknown-unknown

# 3. Stellar CLI (must match SDK major version — use v22)
cargo install --locked stellar-cli --version 22.0.1
```

---

## Build

```bash
stellar contract build
# Output: target/wasm32-unknown-unknown/release/ride_pay.wasm
```

---

## Test

```bash
cargo test
```

Expected:

```
running 5 tests
test test::test_happy_path_settle_ride         ... ok
test test::test_duplicate_ride_id_rejected     ... ok
test test::test_ride_record_stored_correctly   ... ok
test test::test_zero_fare_rejected             ... ok
test test::test_ride_counter_increments        ... ok

test result: ok. 5 passed; 0 failed
```

---

## Deploy to Testnet

```bash
# 1. Generate and fund a deployer key
stellar keys generate deployer --network testnet
stellar keys fund deployer --network testnet

# 2. Upload the Wasm binary to the ledger
stellar contract upload \
  --source deployer \
  --network testnet \
  --wasm target/wasm32-unknown-unknown/release/ride_pay.wasm
# → prints a 64-char hex WASM_HASH

# 3. Deploy a contract instance from that hash
stellar contract deploy \
  --source deployer \
  --network testnet \
  --wasm-hash <WASM_HASH>
# → prints CONTRACT_ID (starts with C...)

# 4. Initialise
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source deployer \
  --network testnet \
  -- initialize \
  --admin         <ADMIN_ADDRESS> \
  --usdc_token    <USDC_CONTRACT_ADDRESS> \
  --loyalty_token <LOYALTY_CONTRACT_ADDRESS>
```

> Get the USDC contract address for testnet:
> ```bash
> stellar contract id asset \
>   --asset USDC:GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5 \
>   --network testnet
> ```

---

## Rider: approve spending before first ride

The mobile app handles this via wallet deep-link. Manually via CLI:

```bash
stellar contract invoke \
  --id <USDC_CONTRACT_ADDRESS> \
  --source rider_key \
  --network testnet \
  -- approve \
  --from              <RIDER_ADDRESS> \
  --spender           <CONTRACT_ID> \
  --amount            10000000 \
  --expiration_ledger 9999999
```

---

## Sample CLI Invocation — Settle a Ride

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source rider_key \
  --network testnet \
  -- settle_ride \
  --ride_id     "ride-20240101-001" \
  --rider       <RIDER_ADDRESS> \
  --operator    <OPERATOR_ADDRESS> \
  --usdc_amount 100000
```

Expected response:

```json
{
  "operator":    "G...",
  "rider":       "G...",
  "usdc_amount": 100000,
  "settled_at":  1717977600
}
```

> `usdc_amount` is in USDC stroops (7 decimal places). `100000 = 0.01 USDC`.
> Adjust to match the local fare converted to USD at today's rate.

---

## Adapting to your region

| Parameter | How to adapt |
|-----------|-------------|
| `usdc_amount` | Convert local fare → USD → multiply by 10,000,000 for stroops |
| `loyalty_token` | Issue any custom Stellar asset; admin mints reserve to contract at deploy |
| USDC anchor | Integrate any regional anchor for fiat off-ramp via Stellar DEX |
| QR format | Encode `stellar:<OPERATOR>?amount=<FARE>&asset=USDC` per SEP-7 |

---

## Project structure

```
ridepay/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs    ← Soroban contract
    └── test.rs   ← 5 unit tests
```

---

## License

MIT © 2024 RidePay contributors

## Contract Details

- Contract Address: CBLU4IUASQ4WUMOXBFLZRSBBLILGOH33GS4LUPKFBCCCMJCDQNMF7G2M
  <img width="1920" height="950" alt="image" src="https://github.com/user-attachments/assets/b8e1eb87-bdb3-4ce3-8872-a71b5c117ba4" />
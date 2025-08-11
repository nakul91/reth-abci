# reth-abci (Tiny Skeleton)

A minimal ABCI app to drive an Ethereum execution layer (Reth) from CometBFT.
This repository is intentionally tiny and compile-friendly **without** Reth;
enable the `with-reth` feature to wire real execution once you pin matching
Reth crate versions.

## Layout
```
reth-abci/
├─ Cargo.toml                # workspace
└─ crates/abci-node/
   ├─ Cargo.toml             # features + deps
   └─ src/
      ├─ main.rs             # boots ABCI server (tcp://127.0.0.1:26658)
      ├─ app.rs              # ABCI methods
      ├─ exec.rs             # RethCtx + block execution (feature-gated)
      └─ wire.rs             # tx decoding, apphash util (feature-gated)
```

## Build & run (stub, no Reth)
This mode compiles everywhere so you can test CometBFT plumbing first.

```bash
cargo build -p abci-node
cargo run -p abci-node
```

Then point CometBFT at it:
```toml
# ~/.cometbft/config/config.toml
proxy_app = "tcp://127.0.0.1:26658"
create_empty_blocks = false
timeout_propose = "1s"
timeout_precommit = "1s"
```
Run CometBFT:
```bash
cometbft init
cometbft start
```

## Enable real Reth integration
1. Check your Reth version:
   ```bash
   reth --version
   ```
2. Edit `crates/abci-node/Cargo.toml` to pin all `reth*` crates to the same minor.
3. Build with feature:
   ```bash
   cargo run -p abci-node --features with-reth
   ```
4. Fill TODOs in:
   - `RethCtx::open()` – open MDBX, load ChainSpec, init txpool
   - `validate_tx_basic()` – sig/nonce/balance checks
   - `propose_block()` – policy + pre-sim (optional)
   - `apply_tx()` – execute via `reth-evm`, update overlay/receipts
   - `commit()` – flush overlay, compute real `stateRoot` & `receiptsRoot`

## Milestones
- ✅ ABCI plumbing with stubbed execution
- ☐ Real EVM execution via Reth
- ☐ EIP-1559 basefee at `end_block`
- ☐ Deterministic AppHash = keccak(stateRoot || receiptsRoot)
- ☐ Minimal eth JSON-RPC facade (balance, block, receipt)

## License
MIT

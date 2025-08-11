use anyhow::{Result};
use tendermint::abci::types::Header as TmHeader;

// --- Reth imports are behind a feature flag so the skeleton compiles without them ---
#[cfg(feature = "with-reth")]
use {
    reth_db::{mdbx::DatabaseArguments},
    reth_primitives::{ChainSpec, TxEnvelope, U256},
    reth_transaction_pool::{Pool},
    reth_evm::execute::{BlockEnv, CfgEnv, TxEnv},
};

// Public context available to the ABCI app
pub struct RethCtx {
    #[allow(dead_code)]
    pub db_path: String,

    // When feature is on, wire actual Reth components
    #[cfg(feature = "with-reth")]
    pub chain: ChainSpec,
    #[cfg(feature = "with-reth")]
    pub pool: Pool<reth_primitives::TxEnvelope>,
}

impl RethCtx {
    pub fn open(path: &str) -> Result<Self> {
        #[cfg(feature = "with-reth")]
        {
            // TODO: open MDBX, load/specify ChainSpec, init txpool
            let chain = ChainSpec::builder().chain_id(777u64).build();
            let pool = Pool::default();
            return Ok(Self { db_path: path.into(), chain, pool });
        }
        #[cfg(not(feature = "with-reth"))]
        {
            // Stub context (compiles without reth). Enable `--features with-reth` later.
            Ok(Self { db_path: path.into() })
        }
    }

    pub fn validate_tx_basic(&self, _tx: &crate::wire::TxEnvelopeAny) -> Result<()> {
        // TODO: When with-reth, verify sig/nonce/balance/intrinsic gas by querying state
        Ok(())
    }

    pub fn propose_block(&self, _max_bytes: usize) -> Proposed {
        // TODO: pull from pool by priority; here we return empty for the stub
        Proposed { txs: vec![] }
    }

    pub fn quick_validate_proposal(&self, _txs: &[bytes::Bytes]) -> bool {
        // TODO: lightweight stateless checks
        true
    }
}

pub struct Proposed { pub txs: Vec<Vec<u8>>; }

pub struct BlockExec {
    header: TmHeader,
    receipts: Vec<Receipt>,
    gas_used: u64,
    state_root: [u8; 32],
}

impl BlockExec {
    pub fn new(_reth: &RethCtx, header: TmHeader) -> Self {
        Self {
            header,
            receipts: vec![],
            gas_used: 0,
            state_root: [0u8; 32],
        }
    }

    pub fn apply_tx(&mut self, _reth: &RethCtx, tx: crate::wire::TxEnvelopeAny) -> Result<Receipt> {
        // TODO: when with-reth, build EVM envs and execute tx; update overlay + receipts
        let _ = tx;
        Ok(Receipt::ok())
    }

    pub fn commit(self) -> Result<([u8;32],[u8;32],u64,u64)> {
        // TODO: flush overlay to MDBX and compute final roots
        let state_root = [0u8; 32];
        let receipts_root = [0u8; 32];
        let ts = self.header.time.unix_timestamp() as u64;
        Ok((state_root, receipts_root, self.gas_used, ts))
    }
}

pub struct Receipt { /* fill with fields as needed */ }
impl Receipt {
    pub fn ok() -> Self { Self{} }
    pub fn into_abci_events(self) -> Vec<tendermint_abci::Event> { vec![] }
}

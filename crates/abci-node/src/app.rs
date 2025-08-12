use std::sync::{Arc, Mutex};
use anyhow::Result;
use tracing::info;

// Import the Application trait
use tendermint_abci::Application;

// Import the types from tendermint_proto that the Application trait expects
use tendermint_proto::abci;
use tendermint::block::Header as TmHeader;

use crate::exec::{BlockExec, RethCtx};
use crate::wire::{decode_eth_tx, apphash_from};

#[derive(Clone)]
pub struct EvmAbciApp {
    inner: Arc<Mutex<State>>,
}

pub struct State {
    reth: RethCtx,
    height: i64,
    last_app_hash: [u8; 32],
    in_block: Option<BlockExec>,
}

impl EvmAbciApp {
    pub fn boot(path: &str) -> Result<Self> {
        let reth = RethCtx::open(path)?;
        let inner = Arc::new(Mutex::new(State {
            reth,
            height: 0,
            last_app_hash: [0u8; 32],
            in_block: None,
        }));
        Ok(Self { inner })
    }
}

impl Application for EvmAbciApp {
    fn info(
        &self,
        _req: abci::RequestInfo,
    ) -> abci::ResponseInfo {
        let st = self.inner.lock().unwrap();
        abci::ResponseInfo {
            data: "reth-abci".into(),
            version: "0.1.0".into(),
            app_version: 1,
            last_block_height: st.height,
            last_block_app_hash: st.last_app_hash.to_vec().into(),
        }
    }

    fn init_chain(
        &self,
        _req: abci::RequestInitChain,
    ) -> abci::ResponseInitChain {
        Default::default()
    }

    fn begin_block(
        &self,
        req: abci::RequestBeginBlock,
    ) -> abci::ResponseBeginBlock {
        let mut st = self.inner.lock().unwrap();
        let proto_header = req.header.expect("missing header");
        let header = TmHeader::try_from(proto_header).expect("invalid header");
        st.in_block = Some(BlockExec::new(&st.reth, header));
        Default::default()
    }

    fn check_tx(
        &self,
        req: abci::RequestCheckTx,
    ) -> abci::ResponseCheckTx {
        let st = self.inner.lock().unwrap();
        match decode_eth_tx(&req.tx).and_then(|etx| st.reth.validate_tx_basic(&etx)) {
            Ok(_) => abci::ResponseCheckTx { code: 0, ..Default::default() },
            Err(e) => abci::ResponseCheckTx {
                code: 1,
                log: format!("{}", e),
                ..Default::default()
            },
        }
    }

    fn prepare_proposal(
        &self,
        req: abci::RequestPrepareProposal,
    ) -> abci::ResponsePrepareProposal {
        let st = self.inner.lock().unwrap();
        let out = st.reth.propose_block(req.max_tx_bytes as usize);
        abci::ResponsePrepareProposal {
            txs: out.txs.into_iter().map(|b| b.into()).collect(),
        }
    }

    fn process_proposal(
        &self,
        req: abci::RequestProcessProposal,
    ) -> abci::ResponseProcessProposal {
        let st = self.inner.lock().unwrap();
        let txs: Vec<Vec<u8>> = req.txs.iter().map(|b| b.to_vec()).collect();
        let valid = st.reth.quick_validate_proposal(&txs);
        abci::ResponseProcessProposal {
            status: if valid { abci::response_process_proposal::ProposalStatus::Accept as i32 } else { abci::response_process_proposal::ProposalStatus::Reject as i32 },
        }
    }

    fn deliver_tx(
        &self,
        req: abci::RequestDeliverTx,
    ) -> abci::ResponseDeliverTx {
        // Clone reth context first to avoid borrowing conflicts
        let reth = {
            let st = self.inner.lock().unwrap();
            st.reth.clone()
        };
        
        // Now get mutable access to the execution context
        let mut st = self.inner.lock().unwrap();
        let Some(exec) = st.in_block.as_mut() else { return Default::default(); };
        
        match decode_eth_tx(&req.tx).and_then(|etx| exec.apply_tx(&reth, etx)) {
            Ok(_r) => abci::ResponseDeliverTx { code: 0, ..Default::default() },
            Err(e) => abci::ResponseDeliverTx {
                code: 1,
                log: format!("{}", e),
                ..Default::default()
            },
        }
    }

    fn end_block(
        &self,
        _req: abci::RequestEndBlock,
    ) -> abci::ResponseEndBlock {
        Default::default()
    }

    fn commit(&self) -> abci::ResponseCommit {
        let mut st = self.inner.lock().unwrap();
        let exec = st.in_block.take().expect("begin_block not called");
        let (state_root, receipts_root, _gas_used, _ts) = exec.commit().expect("commit");
        let app_hash = apphash_from(state_root, receipts_root);
        st.last_app_hash = app_hash;
        st.height += 1;
        abci::ResponseCommit {
            data: app_hash.to_vec().into(),
            retain_height: 0,
        }
    }

    fn offer_snapshot(
        &self,
        _req: abci::RequestOfferSnapshot,
    ) -> abci::ResponseOfferSnapshot {
        abci::ResponseOfferSnapshot {
          result: abci::response_offer_snapshot::Result::Accept as i32,
        }
    }

    fn list_snapshots(&self) -> abci::ResponseListSnapshots {
        abci::ResponseListSnapshots {
            snapshots: vec![],
        }
    }

    fn load_snapshot_chunk(
        &self,
        _req: abci::RequestLoadSnapshotChunk,
    ) -> abci::ResponseLoadSnapshotChunk {
        abci::ResponseLoadSnapshotChunk {
            chunk: vec![].into(),
        }
    }

    fn apply_snapshot_chunk(
        &self,
        _req: abci::RequestApplySnapshotChunk,
    ) -> abci::ResponseApplySnapshotChunk {
        abci::ResponseApplySnapshotChunk {
            result: abci::response_apply_snapshot_chunk::Result::Accept as i32,
            refetch_chunks: vec![],
            reject_senders: vec![],
        }
    }

    fn query(&self, _req: abci::RequestQuery) -> abci::ResponseQuery {
        abci::ResponseQuery {
            code: 0,
            log: String::new(),
            info: String::new(),
            index: 0,
            key: vec![].into(),
            value: vec![].into(),
            proof_ops: None,
            height: 0,
            codespace: String::new(),
        }
    }
}
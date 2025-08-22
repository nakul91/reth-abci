use std::sync::{Arc, Mutex};
use anyhow::Result;
use tracing::info;

use tendermint_abci::Application;
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
        info!("Booting EVM ABCI app with data path: {}", path);
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
    fn info(&self, _req: abci::RequestInfo) -> abci::ResponseInfo {
        let st = self.inner.lock().unwrap();
        info!("ABCI Info called - height: {}", st.height);
        abci::ResponseInfo {
            data: "reth-abci".into(),
            version: "0.1.0".into(),
            app_version: 1,
            last_block_height: st.height,
            last_block_app_hash: st.last_app_hash.to_vec().into(),
        }
    }

    fn init_chain(&self, req: abci::RequestInitChain) -> abci::ResponseInitChain {
        info!("Initializing chain with {} validators", req.validators.len());
        Default::default()
    }

    fn begin_block(&self, req: abci::RequestBeginBlock) -> abci::ResponseBeginBlock {
        let mut st = self.inner.lock().unwrap();

        let Some(proto_header) = req.header else {
            info!("Received begin_block without header — skipping block setup.");
            return Default::default();
        };

        let Ok(header) = TmHeader::try_from(proto_header) else {
            info!("Invalid header in begin_block — skipping.");
            return Default::default();
        };

        info!("Beginning block at height {}", header.height);
        st.in_block = Some(BlockExec::new(&st.reth, header));
        Default::default()
    }

    fn check_tx(&self, req: abci::RequestCheckTx) -> abci::ResponseCheckTx {
        let st = self.inner.lock().unwrap();
        match decode_eth_tx(&req.tx).and_then(|etx| st.reth.validate_tx_basic(&etx)) {
            Ok(_) => {
                info!("CheckTx passed for tx");
                abci::ResponseCheckTx {
                    code: 0,
                    gas_wanted: 100_000,
                    ..Default::default()
                }
            }
            Err(e) => {
                info!("CheckTx failed: {}", e);
                abci::ResponseCheckTx {
                    code: 1,
                    log: format!("{}", e),
                    ..Default::default()
                }
            }
        }
    }

    fn prepare_proposal(
        &self,
        req: abci::RequestPrepareProposal,
    ) -> abci::ResponsePrepareProposal {
        let st = self.inner.lock().unwrap();
        info!("Preparing proposal with max {} bytes", req.max_tx_bytes);
    
        let out = st.reth.propose_block(req.max_tx_bytes as usize);
    
        abci::ResponsePrepareProposal {
            txs: out.txs.into_iter().map(Into::into).collect(),
            ..Default::default()
        }
    }

    fn process_proposal(&self, req: abci::RequestProcessProposal) -> abci::ResponseProcessProposal {
        let st = self.inner.lock().unwrap();
        let txs: Vec<Vec<u8>> = req.txs.iter().map(|b| b.to_vec()).collect();
        let valid = st.reth.quick_validate_proposal(&txs);
        info!("Processing proposal with {} txs - valid: {}", txs.len(), valid);
        abci::ResponseProcessProposal {
            status: if valid {
                abci::response_process_proposal::ProposalStatus::Accept as i32
            } else {
                abci::response_process_proposal::ProposalStatus::Reject as i32
            },
        }
    }

    fn deliver_tx(&self, req: abci::RequestDeliverTx) -> abci::ResponseDeliverTx {
        let reth = {
            let st = self.inner.lock().unwrap();
            st.reth.clone()
        };

        let mut st = self.inner.lock().unwrap();
        let Some(exec) = st.in_block.as_mut() else {
            return abci::ResponseDeliverTx {
                code: 2,
                log: "No block in progress".into(),
                ..Default::default()
            };
        };

        match decode_eth_tx(&req.tx).and_then(|etx| exec.apply_tx(&reth, etx)) {
            Ok(receipt) => {
                info!("Transaction executed successfully - gas used: {}", receipt.gas_used);
                abci::ResponseDeliverTx {
                    code: 0,
                    gas_wanted: 100_000,
                    gas_used: receipt.gas_used as i64,
                    events: receipt.into_abci_events(),
                    ..Default::default()
                }
            }
            Err(e) => {
                info!("Transaction failed: {}", e);
                abci::ResponseDeliverTx {
                    code: 1,
                    log: format!("{}", e),
                    ..Default::default()
                }
            }
        }
    }

    fn end_block(&self, req: abci::RequestEndBlock) -> abci::ResponseEndBlock {
        info!("Ending block at height {}", req.height);
        Default::default()
    }

    fn commit(&self) -> abci::ResponseCommit {
        let mut st = self.inner.lock().unwrap();
        let Some(exec) = st.in_block.take() else {
            info!("No block in progress during commit — returning previous app hash.");
            return abci::ResponseCommit {
                data: st.last_app_hash.to_vec().into(),
                retain_height: st.height,
            };
        };

        match exec.commit() {
            Ok((state_root, receipts_root, gas_used, _ts)) => {
                let app_hash = apphash_from(state_root, receipts_root);
                st.last_app_hash = app_hash;
                st.height += 1;

                info!(
                    "Committed block {} - gas used: {}, app hash: {}",
                    st.height,
                    gas_used,
                    hex::encode(&app_hash)
                );

                abci::ResponseCommit {
                    data: app_hash.to_vec().into(),
                    retain_height: 0,
                }
            }
            Err(e) => {
                info!("Failed to commit block: {}", e);
                abci::ResponseCommit {
                    data: st.last_app_hash.to_vec().into(),
                    retain_height: st.height,
                }
            }
        }
    }

    fn offer_snapshot(&self, _req: abci::RequestOfferSnapshot) -> abci::ResponseOfferSnapshot {
        abci::ResponseOfferSnapshot {
            result: abci::response_offer_snapshot::Result::Reject as i32,
        }
    }

    fn list_snapshots(&self) -> abci::ResponseListSnapshots {
        abci::ResponseListSnapshots { snapshots: vec![] }
    }

    fn load_snapshot_chunk(&self, _req: abci::RequestLoadSnapshotChunk) -> abci::ResponseLoadSnapshotChunk {
        abci::ResponseLoadSnapshotChunk {
            chunk: vec![].into(),
        }
    }

    fn apply_snapshot_chunk(&self, _req: abci::RequestApplySnapshotChunk) -> abci::ResponseApplySnapshotChunk {
        abci::ResponseApplySnapshotChunk {
            result: abci::response_apply_snapshot_chunk::Result::Abort as i32,
            refetch_chunks: vec![],
            reject_senders: vec![],
        }
    }

    fn query(&self, req: abci::RequestQuery) -> abci::ResponseQuery {
        info!("Query received for path: {}", req.path);
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
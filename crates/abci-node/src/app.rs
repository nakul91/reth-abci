use anyhow::Result;
use bytes::Bytes;
use tendermint_abci::{Application, Request, Response};
use tendermint_abci::application::OfferSnapshotResult;
use tracing::{info, warn};

use crate::exec::{BlockExec, RethCtx};
use crate::wire::{decode_eth_tx, apphash_from};

pub struct EvmAbciApp {
    pub reth: RethCtx,
    pub in_block: Option<BlockExec>,
    pub last_app_hash: [u8; 32],
    pub height: i64,
}

impl EvmAbciApp {
    pub fn boot(path: &str) -> Result<Self> {
        let reth = RethCtx::open(path)?;
        Ok(Self {
            reth,
            in_block: None,
            last_app_hash: [0u8; 32],
            height: 0,
        })
    }
}

impl Application for EvmAbciApp {
    fn info(&self, _req: Request) -> Response {
        let mut r = Response::default();
        r.info.data = "reth-abci".into();
        r.info.version = "0.1.0".into();
        r.info.last_block_height = self.height.into();
        r.info.last_block_app_hash = self.last_app_hash.to_vec().into();
        r
    }

    fn init_chain(&mut self, _req: Request) -> Response {
        // Optional: initialize genesis in MDBX if empty.
        Response::default()
    }

    fn begin_block(&mut self, req: Request) -> Response {
        let header = req.begin_block().unwrap().header.clone();
        self.in_block = Some(BlockExec::new(&self.reth, header));
        Response::default()
    }

    fn check_tx(&self, req: Request) -> Response {
        let tx_bytes = req.check_tx().unwrap().tx.clone();
        let mut r = Response::default();
        match decode_eth_tx(&tx_bytes)
            .and_then(|etx| self.reth.validate_tx_basic(&etx))
        {
            Ok(_) => {
                r.check_tx_mut().code = 0;
            }
            Err(e) => {
                r.check_tx_mut().code = 1;
                r.check_tx_mut().log = format!("invalid tx: {e:#}").into();
            }
        }
        r
    }

    fn prepare_proposal(&mut self, req: Request) -> Response {
        let pp = req.prepare_proposal().unwrap();
        let out = self.reth.propose_block(pp.max_bytes as usize);
        let mut r = Response::default();
        r.prepare_proposal_mut().txs = out.txs.into_iter().map(|b| b.into()).collect();
        r
    }

    fn process_proposal(&mut self, req: Request) -> Response {
        let prop = req.process_proposal().unwrap();
        let ok = self.reth.quick_validate_proposal(&prop.txs);
        let mut r = Response::default();
        r.process_proposal_mut().status = if ok {
            tendermint_abci::application::ProcessProposalStatus::Accept
        } else {
            tendermint_abci::application::ProcessProposalStatus::Reject
        };
        r
    }

    fn deliver_tx(&mut self, req: Request) -> Response {
        let tx_bytes = req.deliver_tx().unwrap().tx.clone();
        let mut r = Response::default();
        let Some(exec) = self.in_block.as_mut() else { return r; };

        match decode_eth_tx(&tx_bytes).and_then(|etx| exec.apply_tx(&self.reth, etx)) {
            Ok(receipt) => {
                r.deliver_tx_mut().code = 0;
                r.deliver_tx_mut().events = receipt.into_abci_events();
            }
            Err(e) => {
                r.deliver_tx_mut().code = 1;
                r.deliver_tx_mut().log = format!("exec err: {e:#}").into();
            }
        }
        r
    }

    fn end_block(&mut self, _req: Request) -> Response {
        // (Optional) validator updates, basefee calc if using 1559, consensus param updates
        Response::default()
    }

    fn commit(&mut self, _req: Request) -> Response {
        let exec = self.in_block.take().expect("begin_block not called");
        let (state_root, receipts_root, _gas_used, _ts) = exec.commit().expect("commit");

        let app_hash = apphash_from(state_root, receipts_root);
        self.last_app_hash = app_hash;
        self.height += 1;

        let mut r = Response::default();
        r.commit_mut().data = app_hash.to_vec().into();
        r
    }

    fn offer_snapshot(&mut self, _req: Request) -> Response {
        let mut r = Response::default();
        r.offer_snapshot_mut().result = OfferSnapshotResult::Accept;
        r
    }
}

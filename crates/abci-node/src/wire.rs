use anyhow::Result;

// When with-reth, re-export the actual TxEnvelope. Otherwise, define a placeholder type.
#[cfg(feature = "with-reth")]
pub use reth_primitives::TxEnvelope;

#[cfg(not(feature = "with-reth"))]
#[derive(Clone, Debug)]
pub struct TxEnvelope;

// A type alias so app/exec can refer to a single name independent of the feature.
#[cfg(feature = "with-reth")]
pub type TxEnvelopeAny = reth_primitives::TxEnvelope;
#[cfg(not(feature = "with-reth"))]
pub type TxEnvelopeAny = TxEnvelope;

#[cfg(feature = "with-reth")]
pub fn decode_eth_tx(raw: &[u8]) -> Result<TxEnvelopeAny> {
    use reth_primitives::TxEnvelope as E;
    let tx = E::decode_opaque(raw)?;
    Ok(tx)
}

#[cfg(not(feature = "with-reth"))]
pub fn decode_eth_tx(_raw: &[u8]) -> Result<TxEnvelopeAny> {
    // Stub: return a placeholder
    Ok(TxEnvelope)
}

pub fn apphash_from(state_root: [u8;32], receipts_root: [u8;32]) -> [u8;32] {
    // Use keccak when with-reth; otherwise simple placeholder hash (NOT SECURE)
    #[cfg(feature = "with-reth")]
    {
        use reth_primitives::keccak256;
        return keccak256([state_root, receipts_root].concat()).0;
    }
    #[cfg(not(feature = "with-reth"))]
    {
        // Poor-man's hash for the stub (NOT SECURE, replace after enabling with-reth)
        let mut out = [0u8; 32];
        for i in 0..32 {
            out[i] = state_root[i] ^ receipts_root[i];
        }
        out
    }
}

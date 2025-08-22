use anyhow::Result;

// When with-reth, use the actual transaction types from reth
#[cfg(feature = "with-reth")]
use reth_primitives::TransactionSigned;

#[cfg(not(feature = "with-reth"))]
#[derive(Clone, Debug)]
pub struct TransactionSigned;

// Type alias for cleaner code
#[cfg(feature = "with-reth")]
pub type TxEnvelopeAny = reth_primitives::TransactionSigned;

#[cfg(not(feature = "with-reth"))]
pub type TxEnvelopeAny = TransactionSigned;

#[cfg(feature = "with-reth")]
pub fn decode_eth_tx(raw: &[u8]) -> Result<TxEnvelopeAny> {
    use reth_primitives::{TransactionSigned, Bytes};
    use reth::primitives::Bytes as RethBytes;
    
    // Try to decode the transaction
    // First check if it's an EIP-2718 typed transaction
    if !raw.is_empty() && raw[0] <= 0x7f {
        // This is a typed transaction
        TransactionSigned::decode_enveloped(RethBytes::from(raw.to_vec()))
            .map_err(|e| anyhow::anyhow!("Failed to decode typed tx: {}", e))
    } else {
        // Legacy transaction
        use reth_primitives::Decodable;
        let mut buf = &raw[..];
        TransactionSigned::decode(&mut buf)
            .map_err(|e| anyhow::anyhow!("Failed to decode legacy tx: {}", e))
    }
}

#[cfg(not(feature = "with-reth"))]
pub fn decode_eth_tx(_raw: &[u8]) -> Result<TxEnvelopeAny> {
    // Stub: return a placeholder
    Ok(TransactionSigned)
}

#[cfg(feature = "with-reth")]
pub fn encode_eth_tx(tx: &TxEnvelopeAny) -> Vec<u8> {
    use reth_primitives::Encodable;
    
    // Encode the transaction
    let mut buf = Vec::new();
    tx.encode(&mut buf);
    buf
}

#[cfg(not(feature = "with-reth"))]
pub fn encode_eth_tx(_tx: &TxEnvelopeAny) -> Vec<u8> {
    vec![]
}

pub fn apphash_from(state_root: [u8; 32], receipts_root: [u8; 32]) -> [u8; 32] {
    #[cfg(feature = "with-reth")]
    {
        use reth::primitives::keccak256;
        // Proper app hash: keccak256(state_root || receipts_root)
        let mut data = Vec::with_capacity(64);
        data.extend_from_slice(&state_root);
        data.extend_from_slice(&receipts_root);
        keccak256(&data).0
    }
    
    #[cfg(not(feature = "with-reth"))]
    {
        // Simple XOR for stub (NOT SECURE - only for testing without reth)
        let mut out = [0u8; 32];
        for i in 0..32 {
            out[i] = state_root[i] ^ receipts_root[i];
        }
        out
    }
}

// Helper function to validate transaction format
#[cfg(feature = "with-reth")]
pub fn validate_tx_format(raw: &[u8]) -> bool {
    decode_eth_tx(raw).is_ok()
}

#[cfg(not(feature = "with-reth"))]
pub fn validate_tx_format(_raw: &[u8]) -> bool {
    true
}

// Helper to extract sender from transaction
#[cfg(feature = "with-reth")]
pub fn get_tx_sender(tx: &TxEnvelopeAny) -> Result<reth::primitives::Address> {
    tx.recover_signer()
        .map_err(|e| anyhow::anyhow!("Failed to recover signer: {}", e))
}

#[cfg(not(feature = "with-reth"))]
pub fn get_tx_sender(_tx: &TxEnvelopeAny) -> Result<[u8; 20]> {
    Ok([0u8; 20])
}
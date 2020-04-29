//! holochain_types::dna::zome is a set of structs for working with holochain dna.

// use super::wasm::DnaWasm;
use holochain_serialized_bytes::prelude::*;

/// Represents an individual "zome".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, SerializedBytes)]
pub struct Zome {
    /// The WasmHash representing the WASM byte code for this zome.
    pub wasm_hash: holo_hash_core::WasmHash,
    // /// The Wasm code for this Zome.
    // pub code: DnaWasm,
}

impl Eq for Zome {}

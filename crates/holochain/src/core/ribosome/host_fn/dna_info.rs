use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;
use holochain_zome_types::info::DnaInfo;

pub fn dna_info(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: (),
) -> Result<DnaInfo, WasmError> {
    unimplemented!()
}


use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::error::RibosomeResult;
use holochain_zome_types::EncryptInput;
use holochain_zome_types::EncryptOutput;
use std::sync::Arc;

pub async fn encrypt(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: EncryptInput,
) -> RibosomeResult<EncryptOutput> {
    unimplemented!();
}

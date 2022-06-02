use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_wasmer_host::prelude::WasmError;

use holochain_types::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn create_link<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CreateLinkInput,
) -> Result<HeaderHash, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let CreateLinkInput {
                base_address,
                target_address,
                link_type,
                tag,
                chain_top_ordering,
            } = input;

            // TODO: This can be removed when we remove zome ids from headers.
            let zome_id = match ribosome.zome_types().find_zome_id_from_link(&link_type) {
                Some(i) => i,
                None => {
                    return Err(WasmError::Host(format!(
                        "Link type {} not found in DNA {}",
                        link_type.0,
                        ribosome.dna_hash()
                    )))
                }
            };

            // Construct the link add
            let header_builder =
                builder::CreateLink::new(base_address, target_address, zome_id, link_type, tag);

            let header_hash = tokio_helper::block_forever_on(tokio::task::spawn(async move {
                // push the header into the source chain
                let header_hash = call_context
                    .host_context
                    .workspace_write()
                    .source_chain()
                    .as_ref()
                    .expect("Must have source chain if write_workspace access is given")
                    .put(None, header_builder, None, chain_top_ordering)
                    .await?;
                Ok::<HeaderHash, RibosomeError>(header_hash)
            }))
            .map_err(|join_error| WasmError::Host(join_error.to_string()))?
            .map_err(|ribosome_error| WasmError::Host(ribosome_error.to_string()))?;

            // return the hash of the committed link
            // note that validation is handled by the workflow
            // if the validation fails this commit will be rolled back by virtue of the DB transaction
            // being atomic
            Ok(header_hash)
        }
        _ => Err(WasmError::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "create_link".into(),
            )
            .to_string(),
        )),
    }
}

// we rely on the tests for get_links and get_link_details

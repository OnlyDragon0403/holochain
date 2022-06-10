use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::error::CascadeError;
use holochain_cascade::Cascade;
use holochain_wasmer_host::prelude::WasmError;

use crate::core::ribosome::HostFnAccess;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_types::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn delete<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: DeleteInput,
) -> Result<HeaderHash, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let DeleteInput {
                deletes_header_hash,
                chain_top_ordering,
            } = input;
            let (deletes_entry_address, _) =
                get_original_entry_data(call_context.clone(), deletes_header_hash.clone())?;

            let host_access = call_context.host_context();

            // handle timeouts at the source chain layer
            tokio_helper::block_forever_on(async move {
                let source_chain = host_access
                    .workspace_write()
                    .source_chain()
                    .as_ref()
                    .expect("Must have source chain if write_workspace access is given");
                let header_builder = builder::Delete {
                    deletes_address: deletes_header_hash,
                    deletes_entry_address,
                };
                let header_hash = source_chain
                    .put(header_builder, None, chain_top_ordering)
                    .await
                    .map_err(|source_chain_error| {
                        WasmError::Host(source_chain_error.to_string())
                    })?;
                Ok(header_hash)
            })
        }
        _ => Err(WasmError::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "delete".into(),
            )
            .to_string(),
        )),
    }
}

pub(crate) fn get_original_entry_data(
    call_context: Arc<CallContext>,
    address: HeaderHash,
) -> Result<(EntryHash, EntryType), WasmError> {
    let network = call_context.host_context.network().clone();
    let workspace = call_context.host_context.workspace();

    tokio_helper::block_forever_on(async move {
        let mut cascade = Cascade::from_workspace_network(&workspace, network);
        let maybe_original_element: Option<SignedHeaderHashed> = cascade
            .get_details(address.clone().into(), GetOptions::content())
            .await?
            .map(|el| {
                match el {
                    holochain_zome_types::metadata::Details::Element(e) => {
                        Ok(e.element.into_inner().0)
                    }
                    // Should not be trying to get original headers via EntryHash
                    holochain_zome_types::metadata::Details::Entry(_) => {
                        Err(CascadeError::InvalidResponse(address.clone().into()))
                    }
                }
            })
            .transpose()?;

        match maybe_original_element {
            Some(SignedHeaderHashed {
                hashed: HeaderHashed {
                    content: header, ..
                },
                ..
            }) => match header.into_entry_data() {
                Some((entry_hash, entry_type)) => Ok((entry_hash, entry_type)),
                _ => Err(RibosomeError::ElementDeps(address.into())),
            },
            None => Err(RibosomeError::ElementDeps(address.into())),
        }
    })
    .map_err(|ribosome_error| WasmError::Host(ribosome_error.to_string()))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_delete_entry_test<'a>() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Crd).await;

        let thing_a: HeaderHash = conductor.call(&alice, "create", ()).await;
        let get_thing: Option<Element> = conductor.call(&alice, "reed", thing_a.clone()).await;
        match get_thing {
            Some(element) => assert!(element.entry().as_option().is_some()),

            None => unreachable!(),
        }

        let _: HeaderHash = conductor
            .call(&alice, "delete_via_hash", thing_a.clone())
            .await;

        let get_thing: Option<Element> = conductor.call(&alice, "reed", thing_a).await;
        match get_thing {
            None => {
                // this is what we want, deletion => None for a get
            }
            _ => unreachable!(),
        }
    }
}

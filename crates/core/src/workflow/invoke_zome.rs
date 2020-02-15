use crate::{
    agent::SourceChain,
    cell::error::CellResult,
    nucleus::{ZomeInvocation, ZomeInvocationResult},
    ribosome::Ribosome,
    txn::source_chain,
};
use sx_types::shims::*;

pub async fn invoke_zome(
    invocation: ZomeInvocation,
    source_chain: SourceChain<'_>,
    cursor_rw: source_chain::CursorRw,
) -> CellResult<ZomeInvocationResult> {
    let dna = source_chain.dna()?;
    let ribosome = Ribosome::new(dna);
    let bundle = source_chain.bundle()?;
    let (result, bundle) = ribosome.call_zome_function(bundle, invocation)?;
    source_chain.try_commit(bundle)?;
    Ok(result)
}

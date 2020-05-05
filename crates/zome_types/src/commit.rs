use crate::validate::ValidateEntryResult;
use holo_hash_core::HeaderHash;
use holochain_serialized_bytes::prelude::*;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum CommitEntryResult {
    Success(HeaderHash),
    ValidateFailed(ValidateEntryResult),
}

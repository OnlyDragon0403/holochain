use crate::{
    conductor::api::error::ConductorApiError,
    core::{
        ribosome::{error::RibosomeError, guest_callback::init::InitResult},
        workflow::error::WorkflowRunError,
    },
};
use holochain_state::error::DatabaseError;
use holochain_types::cell::CellId;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CellError {
    #[error("error dealing with workspace state: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("The Dna was not found in the store")]
    DnaMissing,
    #[error("Failed to join the create cell task: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("Genesis failed: {0}")]
    Genesis(#[from] Box<ConductorApiError>),
    #[error("This cell has not had a successful genesis and cannot be created")]
    CellWithoutGenesis(CellId),
    #[error("The cell failed to cleanup it's environment because: {0}. Recommend manually deleting the database at: {1}")]
    Cleanup(String, PathBuf),
    #[error(transparent)]
    WorkflowRunError(#[from] Box<WorkflowRunError>),
    #[error(transparent)]
    RibosomeError(#[from] RibosomeError),
    #[error("The cell tried to run the initialize zomes callback but failed because {0:?}")]
    InitFailed(InitResult),
}

pub type CellResult<T> = Result<T, CellError>;

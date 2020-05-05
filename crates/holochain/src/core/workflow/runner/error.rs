use crate::conductor::CellError;
use crate::core::{state::workspace::WorkspaceError, workflow::WorkflowError};
use holochain_state::error::DatabaseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkflowRunError {
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),

    #[error(transparent)]
    WorkflowError(#[from] WorkflowError),

    #[error(transparent)]
    WorkspaceError(#[from] WorkspaceError),

    #[error(transparent)]
    CellError(#[from] CellError),
}

/// Internal type to handle running workflows
pub type WorkflowRunResult<T> = Result<T, WorkflowRunError>;

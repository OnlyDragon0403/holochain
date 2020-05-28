#![deny(missing_docs)]

//! Defines [ConductorHandle], a lightweight cloneable reference to a Conductor
//! with a limited public interface.
//!
//! A ConductorHandle can be produced via [Conductor::into_handle]
//!
//! ```rust, no_run
//! # async fn async_main () {
//! # use holochain_state::test_utils::{test_conductor_env, test_wasm_env, TestEnvironment};
//! use holochain_2020::conductor::{Conductor, ConductorBuilder, ConductorHandle};
//! # let env = test_conductor_env();
//! #   let TestEnvironment {
//! #       env: wasm_env,
//! #      tmpdir: _tmpdir,
//! # } = test_wasm_env();
//! let handle: ConductorHandle = ConductorBuilder::new()
//!    .test(env, wasm_env)
//!    .await
//!    .unwrap();
//!
//! // handles are cloneable
//! let handle2 = handle.clone();
//!
//! assert_eq!(handle.list_dnas().await, Ok(vec![]));
//! handle.shutdown().await;
//!
//! // handle2 will only get errors from now on, since the other handle
//! // shut down the conductor.
//! assert!(handle2.list_dnas().await.is_err());
//!
//! # }
//! ```
//!
//! The purpose of this handle is twofold:
//!
//! First, it specifies how to synchronize
//! read/write access to a single Conductor across multiple references. The various
//! Conductor APIs - [CellConductorApi], [AdminInterfaceApi], and [AppInterfaceApi],
//! use a ConductorHandle as their sole method of interaction with a Conductor.
//!
//! Secondly, it hides the concrete type of the Conductor behind a dyn Trait.
//! The Conductor is a central point of configuration, and has several
//! type parameters, used to modify functionality including specifying mock
//! types for testing. If we did not have a way of hiding this type genericity,
//! code which interacted with the Conductor would also have to be highly generic.

use super::{
    api::error::ConductorApiResult,
    config::AdminInterfaceConfig,
    dna_store::DnaStore,
    error::{ConductorResult, CreateAppError},
    manager::TaskManagerRunHandle,
    Cell, Conductor,
};
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::workflow::ZomeCallInvocationResult;
use derive_more::From;
use holochain_types::{
    app::{AppId, InstalledApp},
    autonomic::AutonomicCue,
    cell::CellId,
    dna::DnaFile,
    prelude::*,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::*;

#[cfg(test)]
use super::state::ConductorState;
#[cfg(test)]
use holochain_state::env::EnvironmentWrite;

/// A handle to the Conductor that can easily be passed around and cheaply cloned
pub type ConductorHandle = Arc<dyn ConductorHandleT>;

/// Base trait for ConductorHandle
#[async_trait::async_trait]
pub trait ConductorHandleT: Send + Sync {
    /// Returns error if conductor is shutting down
    async fn check_running(&self) -> ConductorResult<()>;

    /// Add a collection of Admin interfaces and spawn the necessary tasks.
    ///
    /// This requires a concrete ConductorHandle to be passed into the
    /// interface tasks. This is a bit weird to do, but it was the only way
    /// around having a circular reference in the types.
    ///
    /// Never use a ConductorHandle for different Conductor here!
    async fn add_admin_interfaces(
        self: Arc<Self>,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()>;

    /// Add an app interface
    async fn add_app_interface(self: Arc<Self>, port: u16) -> ConductorResult<u16>;

    /// Install a [Dna] in this Conductor
    async fn install_dna(&self, dna: DnaFile) -> ConductorResult<()>;

    /// Get the list of hashes of installed Dnas in this Conductor
    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>>;

    /// Get a [Dna] from the [DnaStore]
    async fn get_dna(&self, hash: &DnaHash) -> Option<DnaFile>;

    /// Add the [DnaFile]s from the wasm and dna_def databases into memory
    async fn add_dnas(&self) -> ConductorResult<()>;

    /// Dispatch a network event to the correct cell.
    async fn dispatch_holochain_p2p_event(
        &self,
        cell_id: &CellId,
        event: holochain_p2p::event::HolochainP2pEvent,
    ) -> ConductorResult<()>;

    /// Invoke a zome function on a Cell
    async fn call_zome(
        &self,
        invocation: ZomeCallInvocation,
    ) -> ConductorApiResult<ZomeCallInvocationResult>;

    /// Cue the autonomic system to perform some action early (experimental)
    async fn autonomic_cue(&self, cue: AutonomicCue, cell_id: &CellId) -> ConductorApiResult<()>;

    /// Get a Websocket port which will
    async fn get_arbitrary_admin_websocket_port(&self) -> Option<u16>;

    /// Return the JoinHandle for all managed tasks, which when resolved will
    /// signal that the Conductor has completely shut down.
    ///
    /// NB: The JoinHandle is not cloneable,
    /// so this can only ever be called successfully once.
    async fn take_shutdown_handle(&self) -> Option<TaskManagerRunHandle>;

    /// Send a signal to all managed tasks asking them to end ASAP.
    async fn shutdown(&self);

    /// Request access to this conductor's keystore
    fn keystore(&self) -> &KeystoreSender;

    /// Request access to this conductor's networking handle
    fn holochain_p2p(&self) -> &holochain_p2p::actor::HolochainP2pSender;

    /// Run genesis on [CellId]s and add them to the db
    async fn genesis_cells(
        self: Arc<Self>,
        app_id: AppId,
        cell_ids_with_proofs: Vec<(CellId, Option<SerializedBytes>)>,
    ) -> ConductorResult<()>;

    /// Setup the cells from the database
    /// Only creates any cells that are not already created
    async fn setup_cells(self: Arc<Self>) -> ConductorResult<Vec<CreateAppError>>;

    /// Activate an app
    async fn activate_app(&self, app_id: AppId) -> ConductorResult<()>;

    /// Deactivate an app
    async fn deactivate_app(&self, app_id: AppId) -> ConductorResult<()>;

    /// Dump the cells state
    async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String>;

    // HACK: remove when B-01593 lands
    #[cfg(test)]
    async fn get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvironmentWrite>;

    // HACK: remove when B-01593 lands
    #[cfg(test)]
    async fn get_state_from_handle(&self) -> ConductorApiResult<ConductorState>;
}

/// The current "production" implementation of a ConductorHandle.
/// The implementation specifies how read/write access to the Conductor
/// should be synchronized across multiple concurrent Handles.
///
/// Synchronization is currently achieved via a simple RwLock, but
/// this could be swapped out with, e.g. a channel Sender/Receiver pair
/// using an actor model.
#[derive(From)]
pub struct ConductorHandleImpl<DS: DnaStore + 'static> {
    pub(crate) conductor: RwLock<Conductor<DS>>,
    pub(crate) keystore: KeystoreSender,
    pub(crate) holochain_p2p: holochain_p2p::actor::HolochainP2pSender,
}

#[async_trait::async_trait]
impl<DS: DnaStore + 'static> ConductorHandleT for ConductorHandleImpl<DS> {
    /// Check that shutdown has not been called
    async fn check_running(&self) -> ConductorResult<()> {
        self.conductor.read().await.check_running()
    }

    async fn add_admin_interfaces(
        self: Arc<Self>,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()> {
        let mut lock = self.conductor.write().await;
        lock.add_admin_interfaces_via_handle(configs, self.clone())
            .await
    }

    async fn add_app_interface(self: Arc<Self>, port: u16) -> ConductorResult<u16> {
        let mut lock = self.conductor.write().await;
        lock.add_app_interface_via_handle(port, self.clone()).await
    }

    async fn install_dna(&self, dna: DnaFile) -> ConductorResult<()> {
        self.conductor.read().await.put_wasm(dna.clone()).await?;
        Ok(self.conductor.write().await.dna_store_mut().add(dna)?)
    }

    async fn add_dnas(&self) -> ConductorResult<()> {
        let dnas = self
            .conductor
            .read()
            .await
            .load_wasms_into_dna_files()
            .await?;
        self.conductor.write().await.dna_store_mut().add_dnas(dnas);
        Ok(())
    }

    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>> {
        Ok(self.conductor.read().await.dna_store().list())
    }

    async fn get_dna(&self, hash: &DnaHash) -> Option<DnaFile> {
        self.conductor.read().await.dna_store().get(hash)
    }

    async fn dispatch_holochain_p2p_event(
        &self,
        cell_id: &CellId,
        event: holochain_p2p::event::HolochainP2pEvent,
    ) -> ConductorResult<()> {
        let lock = self.conductor.read().await;
        let cell: &Cell = lock.cell_by_id(cell_id)?;
        cell.handle_holochain_p2p_event(event).await?;
        Ok(())
    }

    async fn call_zome(
        &self,
        invocation: ZomeCallInvocation,
    ) -> ConductorApiResult<ZomeCallInvocationResult> {
        // FIXME: D-01058: We are holding this read lock for
        // the entire call to call_zome and blocking
        // any writes to the conductor
        let lock = self.conductor.read().await;
        debug!(cell_id = ?invocation.cell_id);
        let cell: &Cell = lock.cell_by_id(&invocation.cell_id)?;
        cell.call_zome(invocation).await.map_err(Into::into)
    }

    async fn autonomic_cue(&self, cue: AutonomicCue, cell_id: &CellId) -> ConductorApiResult<()> {
        let lock = self.conductor.write().await;
        let cell = lock.cell_by_id(cell_id)?;
        let _ = cell.handle_autonomic_process(cue.into()).await;
        Ok(())
    }

    async fn take_shutdown_handle(&self) -> Option<TaskManagerRunHandle> {
        self.conductor.write().await.take_shutdown_handle()
    }

    async fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
        self.conductor
            .read()
            .await
            .get_arbitrary_admin_websocket_port()
    }

    async fn shutdown(&self) {
        self.conductor.write().await.shutdown()
    }

    fn keystore(&self) -> &KeystoreSender {
        &self.keystore
    }

    fn holochain_p2p(&self) -> &holochain_p2p::actor::HolochainP2pSender {
        &self.holochain_p2p
    }

    async fn genesis_cells(
        self: Arc<Self>,
        app_id: AppId,
        cells_ids_with_proofs: Vec<(CellId, Option<SerializedBytes>)>,
    ) -> ConductorResult<()> {
        let cell_ids = {
            self.conductor
                .read()
                .await
                .genesis_cells(cells_ids_with_proofs, self.clone())
                .await?
        };
        let app = InstalledApp { app_id, cell_ids };
        // Update the db
        self.conductor
            .write()
            .await
            .add_inactive_app_to_db(app)
            .await
    }

    async fn setup_cells(self: Arc<Self>) -> ConductorResult<Vec<CreateAppError>> {
        let cells = {
            let lock = self.conductor.read().await;
            lock.create_active_app_cells(self.clone())
                .await?
                .into_iter()
        };
        let add_cells_tasks = cells.map(|result| async {
            match result {
                Ok(cells) => {
                    self.conductor.write().await.add_cells(cells);
                    None
                }
                Err(e) => Some(e),
            }
        });
        Ok(futures::future::join_all(add_cells_tasks)
            .await
            .into_iter()
            // Remove successful and collect the errors
            .filter_map(|r| r)
            .collect())
    }

    async fn activate_app(&self, app_id: AppId) -> ConductorResult<()> {
        self.conductor
            .write()
            .await
            .activate_app_in_db(app_id)
            .await
    }

    async fn deactivate_app(&self, app_id: AppId) -> ConductorResult<()> {
        let cell_ids_to_remove = self
            .conductor
            .write()
            .await
            .deactivate_app_in_db(app_id)
            .await?;
        self.conductor
            .write()
            .await
            .remove_cells(cell_ids_to_remove);
        Ok(())
    }

    async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String> {
        self.conductor.read().await.dump_cell_state(cell_id).await
    }

    #[cfg(test)]
    async fn get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvironmentWrite> {
        let lock = self.conductor.read().await;
        let cell = lock.cell_by_id(cell_id)?;
        Ok(cell.state_env().clone())
    }

    #[cfg(test)]
    async fn get_state_from_handle(&self) -> ConductorApiResult<ConductorState> {
        let lock = self.conductor.read().await;
        Ok(lock.get_state_from_handle().await?)
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use mockall::mock;

    // Unfortunate workaround to get mockall to work with async_trait, due to the complexity of each.
    // The mock! expansion here creates mocks on a non-async version of the API, and then the actual trait is implemented
    // by delegating each async trait method to its sync counterpart
    // See https://github.com/asomers/mockall/issues/75
    mock! {

        pub ConductorHandle {
            fn sync_check_running(&self) -> ConductorResult<()>;

            fn sync_add_admin_interfaces(
                &self,
                configs: Vec<AdminInterfaceConfig>,
            ) -> ConductorResult<()>;

            fn sync_add_app_interface(
                &self,
                port: u16,
            ) -> ConductorResult<u16>;

            fn sync_install_dna(&self, dna: DnaFile) -> ConductorResult<()>;

            fn sync_list_dnas(&self) -> ConductorResult<Vec<DnaHash>>;

            fn sync_add_dnas(&self) -> ConductorResult<()>;

            fn sync_get_dna(&self, hash: &DnaHash) -> Option<DnaFile>;

            fn sync_dispatch_holochain_p2p_event(&self, cell_id: &CellId, event: holochain_p2p::event::HolochainP2pEvent) -> ConductorResult<()>;

            fn sync_call_zome(
                &self,
                invocation: ZomeCallInvocation,
            ) -> ConductorApiResult<ZomeCallInvocationResult>;

            fn sync_autonomic_cue(&self, cue: AutonomicCue, cell_id: &CellId) -> ConductorApiResult<()>;

            fn sync_take_shutdown_handle(&self) -> Option<TaskManagerRunHandle>;

            fn sync_get_arbitrary_admin_websocket_port(&self) -> Option<u16>;

            fn sync_shutdown(&self);

            fn sync_keystore(&self) -> &KeystoreSender;

            fn sync_holochain_p2p(&self) -> &holochain_p2p::actor::HolochainP2pSender;

            fn sync_genesis_cells(
                &self,
                app_id: AppId,
                cell_ids_with_proofs: Vec<(CellId, Option<SerializedBytes>)>,
            ) -> ConductorResult<()>;

            fn sync_setup_cells(&self) -> ConductorResult<Vec<CreateAppError>>;

            fn sync_activate_app(&self, app_id: AppId) -> ConductorResult<()>;

            fn sync_deactivate_app(&self, app_id: AppId) -> ConductorResult<()>;

            fn sync_dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String>;

            #[cfg(test)]
            fn sync_get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvironmentWrite>;

            #[cfg(test)]
            fn sync_get_state_from_handle(&self) -> ConductorApiResult<ConductorState>;
        }

        trait Clone {
            fn clone(&self) -> Self;
        }
    }

    #[async_trait::async_trait]
    impl ConductorHandleT for MockConductorHandle {
        async fn check_running(&self) -> ConductorResult<()> {
            self.sync_check_running()
        }

        async fn add_admin_interfaces(
            self: Arc<Self>,
            configs: Vec<AdminInterfaceConfig>,
        ) -> ConductorResult<()> {
            self.sync_add_admin_interfaces(configs)
        }

        async fn add_app_interface(self: Arc<Self>, port: u16) -> ConductorResult<u16> {
            self.sync_add_app_interface(port)
        }

        async fn add_dnas(&self) -> ConductorResult<()> {
            self.sync_add_dnas()
        }

        async fn install_dna(&self, dna: DnaFile) -> ConductorResult<()> {
            self.sync_install_dna(dna)
        }

        async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>> {
            self.sync_list_dnas()
        }

        async fn get_dna(&self, hash: &DnaHash) -> Option<DnaFile> {
            self.sync_get_dna(hash)
        }

        async fn dispatch_holochain_p2p_event(
            &self,
            cell_id: &CellId,
            event: holochain_p2p::event::HolochainP2pEvent,
        ) -> ConductorResult<()> {
            self.sync_dispatch_holochain_p2p_event(cell_id, event)
        }

        async fn call_zome(
            &self,
            invocation: ZomeCallInvocation,
        ) -> ConductorApiResult<ZomeCallInvocationResult> {
            self.sync_call_zome(invocation)
        }

        async fn autonomic_cue(
            &self,
            cue: AutonomicCue,
            cell_id: &CellId,
        ) -> ConductorApiResult<()> {
            self.sync_autonomic_cue(cue, cell_id)
        }

        async fn take_shutdown_handle(&self) -> Option<TaskManagerRunHandle> {
            self.sync_take_shutdown_handle()
        }

        async fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
            self.sync_get_arbitrary_admin_websocket_port()
        }

        async fn shutdown(&self) {
            self.sync_shutdown()
        }

        fn keystore(&self) -> &KeystoreSender {
            self.sync_keystore()
        }

        fn holochain_p2p(&self) -> &holochain_p2p::actor::HolochainP2pSender {
            self.sync_holochain_p2p()
        }

        async fn genesis_cells(
            self: Arc<Self>,
            app_id: AppId,
            cell_ids_with_proofs: Vec<(CellId, Option<SerializedBytes>)>,
        ) -> ConductorResult<()> {
            self.sync_genesis_cells(app_id, cell_ids_with_proofs)
        }

        /// Setup the cells from the database
        /// Only creates any cells that are not already created
        async fn setup_cells(self: Arc<Self>) -> ConductorResult<Vec<CreateAppError>> {
            self.sync_setup_cells()
        }

        async fn activate_app(&self, app_id: AppId) -> ConductorResult<()> {
            self.sync_activate_app(app_id)
        }

        async fn deactivate_app(&self, app_id: AppId) -> ConductorResult<()> {
            self.sync_deactivate_app(app_id)
        }

        /// Dump the cells state
        async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String> {
            self.sync_dump_cell_state(cell_id)
        }

        // HACK: remove when B-01593 lands
        #[cfg(test)]
        async fn get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvironmentWrite> {
            self.sync_get_cell_env(cell_id)
        }

        // HACK: remove when B-01593 lands
        #[cfg(test)]
        async fn get_state_from_handle(&self) -> ConductorApiResult<ConductorState> {
            self.sync_get_state_from_handle()
        }
    }
}

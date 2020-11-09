// this is largely a passthrough that routes to a specific space handler

use crate::{actor, actor::*, event::*, *};
use futures::{future::FutureExt, stream::StreamExt};
use kitsune_p2p_types::{async_lazy::AsyncLazy, transport::*, transport_pool::*};
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

pub mod bootstrap;
mod gossip;
mod space;
use ghost_actor::dependencies::{must_future, tracing};
use space::*;

ghost_actor::ghost_chan! {
    pub(crate) chan Internal<crate::KitsuneP2pError> {
        /// Register space event handler
        fn register_space_event_handler(recv: futures::channel::mpsc::Receiver<KitsuneP2pEvent>) -> ();
    }
}

pub(crate) struct KitsuneP2pActor {
    channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
    internal_sender: ghost_actor::GhostSender<Internal>,
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    transport: ghost_actor::GhostSender<TransportListener>,
    spaces: HashMap<Arc<KitsuneSpace>, AsyncLazy<ghost_actor::GhostSender<KitsuneP2p>>>,
}

fn build_transport(
    t_conf: TransportConfig,
) -> must_future::MustBoxFuture<
    'static,
    TransportResult<(
        ghost_actor::GhostSender<TransportListener>,
        TransportEventReceiver,
    )>,
> {
    must_future::MustBoxFuture::new(async move {
        match t_conf {
            TransportConfig::Mem {} => {
                Ok(kitsune_p2p_types::transport_mem::spawn_bind_transport_mem().await?)
            }
            TransportConfig::Quic {
                bind_to,
                override_host,
                override_port,
            } => {
                let sub_conf = kitsune_p2p_transport_quic::ConfigListenerQuic::default()
                    .set_bind_to(bind_to)
                    .set_override_host(override_host)
                    .set_override_port(override_port);
                Ok(kitsune_p2p_transport_quic::spawn_transport_listener_quic(sub_conf).await?)
            }
            TransportConfig::Proxy {
                sub_transport,
                proxy_config,
            } => {
                let (sub_lstn, sub_evt) = build_transport(*sub_transport).await?;
                let sub_conf = match proxy_config {
                    ProxyConfig::RemoteProxyClient { proxy_url } => {
                        kitsune_p2p_proxy::ProxyConfig::remote_proxy_client(
                            kitsune_p2p_proxy::TlsConfig::new_ephemeral().await?,
                            proxy_url.into(),
                        )
                    }
                    ProxyConfig::LocalProxyServer {
                        proxy_accept_config,
                    } => kitsune_p2p_proxy::ProxyConfig::local_proxy_server(
                        kitsune_p2p_proxy::TlsConfig::new_ephemeral().await?,
                        match proxy_accept_config {
                            Some(ProxyAcceptConfig::AcceptAll) => {
                                kitsune_p2p_proxy::AcceptProxyCallback::accept_all()
                            }
                            None | Some(ProxyAcceptConfig::RejectAll) => {
                                kitsune_p2p_proxy::AcceptProxyCallback::reject_all()
                            }
                        },
                    ),
                };
                Ok(
                    kitsune_p2p_proxy::spawn_kitsune_proxy_listener(sub_conf, sub_lstn, sub_evt)
                        .await?,
                )
            }
        }
    })
}

impl KitsuneP2pActor {
    pub async fn new(
        config: KitsuneP2pConfig,
        channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
        internal_sender: ghost_actor::GhostSender<Internal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    ) -> KitsuneP2pResult<Self> {
        let (t_pool, transport, mut t_event) = spawn_transport_pool().await?;
        for t_conf in config.transport_pool {
            let (l, e) = build_transport(t_conf).await?;
            t_pool.push_sub_transport(l, e).await?;
        }

        tokio::task::spawn(async move {
            while let Some(event) = t_event.next().await {
                match event {
                    TransportEvent::IncomingChannel(url, _write, _read) => {
                        tracing::warn!("INCOMING CHANNEL: {}", url);
                        unimplemented!()
                    }
                }
            }
        });

        Ok(Self {
            channel_factory,
            internal_sender,
            evt_sender,
            transport,
            spaces: HashMap::new(),
        })
    }
}

impl ghost_actor::GhostControlHandler for KitsuneP2pActor {}

impl ghost_actor::GhostHandler<Internal> for KitsuneP2pActor {}

impl InternalHandler for KitsuneP2pActor {
    fn handle_register_space_event_handler(
        &mut self,
        recv: futures::channel::mpsc::Receiver<KitsuneP2pEvent>,
    ) -> InternalHandlerResult<()> {
        let f = self.channel_factory.attach_receiver(recv);
        Ok(async move {
            f.await?;
            Ok(())
        }
        .boxed()
        .into())
    }
}

impl ghost_actor::GhostHandler<KitsuneP2pEvent> for KitsuneP2pActor {}

impl KitsuneP2pEventHandler for KitsuneP2pActor {
    fn handle_put_agent_info_signed(
        &mut self,
        input: crate::event::PutAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self.evt_sender.put_agent_info_signed(input))
    }

    fn handle_get_agent_info_signed(
        &mut self,
        input: crate::event::GetAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        Ok(self.evt_sender.get_agent_info_signed(input))
    }

    fn handle_call(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<Vec<u8>> {
        Ok(self.evt_sender.call(space, to_agent, from_agent, payload))
    }

    fn handle_notify(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self.evt_sender.notify(space, to_agent, from_agent, payload))
    }

    fn handle_gossip(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        op_hash: Arc<KitsuneOpHash>,
        op_data: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self
            .evt_sender
            .gossip(space, to_agent, from_agent, op_hash, op_data))
    }

    fn handle_fetch_op_hashes_for_constraints(
        &mut self,
        input: FetchOpHashesForConstraintsEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<Arc<KitsuneOpHash>>> {
        Ok(self.evt_sender.fetch_op_hashes_for_constraints(input))
    }

    fn handle_fetch_op_hash_data(
        &mut self,
        input: FetchOpHashDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
        Ok(self.evt_sender.fetch_op_hash_data(input))
    }

    fn handle_sign_network_data(
        &mut self,
        input: SignNetworkDataEvt,
    ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> {
        Ok(self.evt_sender.sign_network_data(input))
    }
}

impl ghost_actor::GhostHandler<KitsuneP2p> for KitsuneP2pActor {}

impl KitsuneP2pHandler for KitsuneP2pActor {
    fn handle_list_transport_bindings(&mut self) -> KitsuneP2pHandlerResult<Vec<url2::Url2>> {
        let fut = self.transport.bound_url();
        Ok(async move {
            let urls = fut.await?;
            Ok(urls
                .query_pairs()
                .map(|(_, url)| url2::url2!("{}", url))
                .collect())
        }
        .boxed()
        .into())
    }

    fn handle_join(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        let internal_sender = self.internal_sender.clone();
        let space2 = space.clone();
        let transport = self.transport.clone();
        let space_sender = match self.spaces.entry(space.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(AsyncLazy::new(async move {
                let (send, evt_recv) = spawn_space(space2, transport)
                    .await
                    .expect("cannot fail to create space");
                internal_sender
                    .register_space_event_handler(evt_recv)
                    .await
                    .expect("FAIL");
                send
            })),
        };
        let space_sender = space_sender.get();
        Ok(async move { space_sender.await.join(space, agent).await }
            .boxed()
            .into())
    }

    fn handle_leave(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Ok(async move { Ok(()) }.boxed().into()),
            Some(space) => space.get(),
        };
        Ok(async move {
            space_sender.await.leave(space.clone(), agent).await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_rpc_single(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(space)),
            Some(space) => space.get(),
        };
        Ok(async move {
            space_sender
                .await
                .rpc_single(space, to_agent, from_agent, payload)
                .await
        }
        .boxed()
        .into())
    }

    #[tracing::instrument(skip(self, input))]
    fn handle_rpc_multi(
        &mut self,
        input: actor::RpcMulti,
    ) -> KitsuneP2pHandlerResult<Vec<actor::RpcMultiResponse>> {
        let space_sender = match self.spaces.get_mut(&input.space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(input.space)),
            Some(space) => space.get(),
        };
        Ok(async move { space_sender.await.rpc_multi(input).await }
            .boxed()
            .into())
    }

    fn handle_notify_multi(&mut self, input: actor::NotifyMulti) -> KitsuneP2pHandlerResult<u8> {
        let space_sender = match self.spaces.get_mut(&input.space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(input.space)),
            Some(space) => space.get(),
        };
        Ok(async move { space_sender.await.notify_multi(input).await }
            .boxed()
            .into())
    }
}

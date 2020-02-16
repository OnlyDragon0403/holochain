use futures::{channel, executor::ThreadPool, prelude::*, task::SpawnExt};
use parking_lot::RwLock;
use std::sync::Arc;
use sx_conductor_lib::{
    api::ConductorApiExternal,
    conductor::Conductor,
    interface::{channel::ChannelInterface, Interface},
};

fn main() {
    println!("Running silly ChannelInterface example");
    let executor = ThreadPool::new().unwrap();
    futures::executor::block_on(example(executor));
}

async fn example(executor: ThreadPool) {
    let (tx_network, _rx_network) = channel::mpsc::channel(1);
    let (mut tx_dummy, rx_dummy) = channel::mpsc::unbounded();
    let conductor = Conductor::new(tx_network);
    let lock = Arc::new(RwLock::new(conductor));
    let handle = ConductorApiExternal::new(lock);
    let interface_fut = executor
        .spawn_with_handle(ChannelInterface::new(rx_dummy).spawn(handle))
        .unwrap();
    let driver_fut = executor
        .spawn_with_handle(async move {
            for _ in 0..50 as u32 {
                dbg!("sending dummy msg");
                tx_dummy.send(true).await.unwrap();
            }
            tx_dummy.send(false).await.unwrap();
        })
        .unwrap();
    futures::join!(interface_fut, driver_fut);
}

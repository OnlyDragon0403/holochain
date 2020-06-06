#[cfg(test)]
mod tests {
    use crate::{event::*, spawn::*, types::*};
    use std::sync::Arc;

    #[tokio::test(threaded_scheduler)]
    async fn test_request_workflow() {
        let space1: Arc<KitsuneSpace> =
            Arc::new(b"ssssssssssssssssssssssssssssssssssss".to_vec().into());
        let a1: Arc<KitsuneAgent> =
            Arc::new(b"111111111111111111111111111111111111".to_vec().into());
        let a2: Arc<KitsuneAgent> =
            Arc::new(b"222222222222222222222222222222222222".to_vec().into());

        let (mut p2p, mut evt) = spawn_kitsune_p2p().await.unwrap();

        let space1_clone = space1.clone();
        let a2_clone = a2.clone();
        tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use KitsuneP2pEvent::*;
                match evt {
                    Request {
                        respond,
                        space,
                        agent,
                        data,
                        ..
                    } => {
                        if space != space1_clone {
                            panic!("unexpected space");
                        }
                        if agent != a2_clone {
                            panic!("unexpected agent");
                        }
                        if &*data != b"hello" {
                            panic!("unexpected request");
                        }
                        let _ = respond(Ok(b"echo: hello".to_vec()));
                    }
                    _ => panic!("unexpected event"),
                }
            }
        });

        p2p.join(space1.clone(), a1.clone()).await.unwrap();
        p2p.join(space1.clone(), a2.clone()).await.unwrap();

        let res = p2p
            .request(space1, a2, Arc::new(b"hello".to_vec()))
            .await
            .unwrap();
        assert_eq!(b"echo: hello".to_vec(), res);
    }
}
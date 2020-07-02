use crate::*;

/// Extend holo_hash::AgentPubKey with additional signature functionality
/// from Keystore.
pub trait AgentPubKeyExt {
    /// create a new agent keypair in given keystore, returning the AgentPubKey
    fn new_from_pure_entropy(
        keystore: &KeystoreSender,
    ) -> KeystoreApiFuture<holo_hash::AgentPubKey>
    where
        Self: Sized;

    /// sign some arbitrary data
    fn sign<D>(&self, keystore: &KeystoreSender, data: D) -> KeystoreApiFuture<Signature>
    where
        D: TryInto<SerializedBytes, Error = SerializedBytesError>;

    /// sign some arbitrary raw bytes
    fn sign_raw(&self, keystore: &KeystoreSender, data: &[u8]) -> KeystoreApiFuture<Signature>;

    /// verify a signature for given data with this agent public_key is valid
    fn verify_signature<D>(&self, signature: &Signature, data: D) -> KeystoreApiFuture<bool>
    where
        D: TryInto<SerializedBytes, Error = SerializedBytesError>;

    /// verify a signature for given raw bytes with this agent public_key is valid
    fn verify_signature_raw(&self, signature: &Signature, data: &[u8]) -> KeystoreApiFuture<bool>;
}

impl AgentPubKeyExt for holo_hash::AgentPubKey {
    fn new_from_pure_entropy(keystore: &KeystoreSender) -> KeystoreApiFuture<holo_hash::AgentPubKey>
    where
        Self: Sized,
    {
        let f = keystore.generate_sign_keypair_from_pure_entropy();
        ghost_actor::dependencies::must_future::MustBoxFuture::new(async move { f.await })
    }

    fn sign<D>(&self, keystore: &KeystoreSender, data: D) -> KeystoreApiFuture<Signature>
    where
        D: TryInto<SerializedBytes, Error = SerializedBytesError>,
    {
        use ghost_actor::dependencies::futures::future::FutureExt;
        let keystore = keystore.clone();
        let maybe_data: Result<SerializedBytes, SerializedBytesError> = data.try_into();
        let key = self.clone();
        async move {
            let data = maybe_data?;
            keystore.sign(SignInput { key, data }).await
        }
        .boxed()
        .into()
    }

    fn sign_raw(&self, keystore: &KeystoreSender, data: &[u8]) -> KeystoreApiFuture<Signature> {
        use ghost_actor::dependencies::futures::future::FutureExt;
        let keystore = keystore.clone();
        let input = SignInput::new_raw(self.clone(), data.to_vec());
        async move { keystore.sign(input).await }.boxed().into()
    }

    fn verify_signature<D>(&self, signature: &Signature, data: D) -> KeystoreApiFuture<bool>
    where
        D: TryInto<SerializedBytes, Error = SerializedBytesError>,
    {
        use ghost_actor::dependencies::futures::future::FutureExt;
        use holo_hash::HoloHashCoreHash;

        let result: KeystoreApiResult<(
            holochain_crypto::DynCryptoBytes,
            holochain_crypto::DynCryptoBytes,
            holochain_crypto::DynCryptoBytes,
        )> = (|| {
            let pub_key = holochain_crypto::crypto_insecure_buffer_from_bytes(self.get_bytes())?;
            let signature = holochain_crypto::crypto_insecure_buffer_from_bytes(&signature.0)?;
            let data: SerializedBytes = data.try_into()?;
            let data = holochain_crypto::crypto_insecure_buffer_from_bytes(data.bytes())?;
            Ok((signature, data, pub_key))
        })();

        async move {
            let (mut signature, mut data, mut pub_key) = result?;
            Ok(
                holochain_crypto::crypto_sign_verify(&mut signature, &mut data, &mut pub_key)
                    .await?,
            )
        }
        .boxed()
        .into()
    }

    fn verify_signature_raw(&self, signature: &Signature, data: &[u8]) -> KeystoreApiFuture<bool> {
        use ghost_actor::dependencies::futures::future::FutureExt;
        use holo_hash::HoloHashCoreHash;

        let result: KeystoreApiResult<(
            holochain_crypto::DynCryptoBytes,
            holochain_crypto::DynCryptoBytes,
            holochain_crypto::DynCryptoBytes,
        )> = (|| {
            let pub_key = holochain_crypto::crypto_insecure_buffer_from_bytes(self.get_bytes())?;
            let signature = holochain_crypto::crypto_insecure_buffer_from_bytes(&signature.0)?;
            let data = holochain_crypto::crypto_insecure_buffer_from_bytes(data)?;
            Ok((signature, data, pub_key))
        })();

        async move {
            let (mut signature, mut data, mut pub_key) = result?;
            Ok(
                holochain_crypto::crypto_sign_verify(&mut signature, &mut data, &mut pub_key)
                    .await?,
            )
        }
        .boxed()
        .into()
    }
}

//! Defines a Element, the basic unit of Holochain data.

use crate::{prelude::*, HeaderHashed};
use derive_more::{From, Into};
use futures::future::FutureExt;
use holo_hash::HeaderAddress;
use holo_hash::{hash_type, HashableContentBytes};
use holochain_keystore::{KeystoreError, Signature};
use holochain_serialized_bytes::prelude::*;
pub use holochain_zome_types::element::*;
use holochain_zome_types::entry::Entry;
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::header::Header;
use must_future::MustBoxFuture;

// /// a chain element which is a triple containing the signature of the header along with the
// /// entry if the header type has one.
// #[derive(Clone, Debug, PartialEq)]
// pub struct Element {
//     /// The signed header for this element
//     signed_header: SignedHeaderHashed,
//     /// If there is an entry associated with this header it will be here
//     maybe_entry: Option<Entry>,
// }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
/// Element without the hashes for sending across the network
pub struct WireElement {
    /// The signed header for this element
    signed_header: SignedHeader,
    /// If there is an entry associated with this header it will be here
    maybe_entry: Option<Entry>,
}

#[async_trait::async_trait]
pub trait ElementExt {
    async fn validate(&self) -> Result<(), KeystoreError>;
}

#[async_trait::async_trait]
impl ElementExt for Element {
    // /// Raw element constructor.  Used only when we know that the values are valid.
    // pub fn new(signed_header: SignedHeaderHashed, maybe_entry: Option<Entry>) -> Self {
    //     Self {
    //         signed_header,
    //         maybe_entry,
    //     }
    // }

    // /// Break this element into its components
    // pub fn into_inner(self) -> (SignedHeaderHashed, Option<Entry>) {
    //     (self.signed_header, self.maybe_entry)
    // }

    /// Validates a chain element
    async fn validate(&self) -> Result<(), KeystoreError> {
        self.signed_header().validate().await?;

        //TODO: make sure that any cases around entry existence are valid:
        //      SourceChainError::InvalidStructure(HeaderAndEntryMismatch(address)),
        Ok(())
    }

    // /// Access the signature portion of this triple.
    // pub fn signature(&self) -> &Signature {
    //     self.signed_header.signature()
    // }

    // /// Access the header address
    // pub fn header_address(&self) -> &HeaderAddress {
    //     self.signed_header.header_address()
    // }

    // /// Access the Header portion of this triple.
    // pub fn header(&self) -> &Header {
    //     self.signed_header.header()
    // }

    // /// Access the HeaderHashed portion.
    // pub fn header_hashed(&self) -> &HeaderHashed {
    //     self.signed_header.header_hashed()
    // }

    // /// Access the Entry portion of this triple as a ElementEntry,
    // /// which includes the context around the presence or absence of the entry.
    // pub fn entry(&self) -> ElementEntry {
    //     let maybe_visibilty = self
    //         .header()
    //         .entry_data()
    //         .map(|(_, entry_type)| entry_type.visibility());
    //     match (self.maybe_entry.as_ref(), maybe_visibilty) {
    //         (Some(entry), Some(_)) => ElementEntry::Present(entry),
    //         (None, Some(EntryVisibility::Private)) => ElementEntry::Hidden,
    //         (None, None) => ElementEntry::NotApplicable,
    //         (Some(_), None) => {
    //             unreachable!("Entry is present for a Header type which has no entry reference")
    //         }
    //         (None, Some(EntryVisibility::Public)) => unreachable!("Entry data missing for element"),
    //     }
    // }
}

// /// Represents the different ways the entry_address reference within a Header
// /// can be intepreted
// #[derive(Clone, Debug, PartialEq, Eq, derive_more::From)]
// pub enum ElementEntry<'a> {
//     /// The Header has an entry_address reference, and the Entry is accessible.
//     Present(&'a Entry),
//     /// The Header has an entry_address reference, but we are in a public
//     /// context and the entry is private.
//     Hidden,
//     /// The Header does not contain an entry_address reference.
//     NotApplicable,
// }

// impl<'a> ElementEntry<'a> {
//     /// Provides entry data if it exists.
//     ///
//     /// Collapses the enum down to the two possibilities of
//     /// extant or nonextant Entry data
//     pub fn as_option(&'a self) -> Option<&'a Entry> {
//         if let ElementEntry::Present(entry) = self {
//             Some(entry)
//         } else {
//             None
//         }
//     }
// }

// /// A combination of a Header and its signature.
// ///
// /// Has implementations From and Into its tuple form.
// #[derive(Clone, Debug, From, Into, PartialEq, Serialize, Deserialize, SerializedBytes)]
// pub struct SignedHeader(Header, Signature);

// impl SignedHeader {
//     /// Accessor for the Header
//     pub fn header(&self) -> &Header {
//         &self.0
//     }

//     /// Accessor for the Signature
//     pub fn signature(&self) -> &Signature {
//         &self.1
//     }
// }

// impl HashableContent for SignedHeader {
//     type HashType = hash_type::Header;

//     fn hash_type(&self) -> Self::HashType {
//         hash_type::Header
//     }

//     fn hashable_content(&self) -> HashableContentBytes {
//         HashableContentBytes::Content(
//             (&self.0)
//                 .try_into()
//                 .expect("Could not serialize HashableContent"),
//         )
//     }
// }

// HACK: In this representation, we have to clone the Header and store it twice,
// once in the HeaderHashed, and once in the SignedHeader. The reason is that
// the API currently requires references to both types, and it was easier to
// do a simple clone than to refactor the entire struct and API to remove the
// need for one of those references. We probably SHOULD do that refactor at
// some point.
// FIXME: refactor so that HeaderHashed is not stored, and then remove the
// header_hashed method which returns a reference to HeaderHashed.
// BTW, I tried to think about the possibility of the following, but none were easy:
// - Having a lazily instantiable SignedHeader, so we only have to clone if needed
// - Having HeaderHashed take AsRefs for its arguments, so you can have a
//    HeaderHashed of references instead of values
// FIXME: OR, even better yet, do away with this struct and just use
// HoloHashed<SignedHeader> instead, if possible and expedient
/// The header and the signature that signed it
// #[derive(Clone, Debug, PartialEq)]
// pub struct SignedHeaderHashed {
//     header: HeaderHashed,
//     signed_header: SignedHeader,
// }

#[async_trait::async_trait]
pub trait SignedHeaderHashedExt {
    /// Create a hash from data
    fn with_data(
        signed_header: SignedHeader,
    ) -> MustBoxFuture<'static, Result<SignedHeaderHashed, SerializedBytesError>>;
    // where
    //     S: Sized;
    /// Sign sme content
    async fn new(
        keystore: &KeystoreSender,
        header: HeaderHashed,
    ) -> Result<SignedHeaderHashed, KeystoreError>;
    /// Validate the data
    async fn validate(&self) -> Result<(), KeystoreError>;
}

#[allow(missing_docs)]
#[async_trait::async_trait]
impl SignedHeaderHashedExt for SignedHeaderHashed {
    // /// Unwrap the complete contents of this "Hashed" wrapper.
    // pub fn into_inner(self) -> (SignedHeader, HeaderHash) {
    //     let (header, hash) = self.header.into_inner();
    //     ((header, self.signed_header.1).into(), hash)
    // }

    // /// Access the main item stored in this wrapper type.
    // pub fn as_content(&self) -> &SignedHeader {
    //     &self.signed_header
    // }

    // /// Access the already-calculated hash stored in this wrapper type.
    // pub fn as_hash(&self) -> &HeaderHash {
    //     self.header.as_hash()
    // }

    fn with_data(
        signed_header: SignedHeader,
    ) -> MustBoxFuture<'static, Result<Self, SerializedBytesError>>
    where
        Self: Sized,
    {
        async move {
            let (header, signature) = signed_header.into();
            Ok(Self {
                header: HeaderHashed::with_data(header.clone()).await?,
                signed_header: SignedHeader(header, signature),
            })
        }
        .boxed()
        .into()
    }
    /// SignedHeader constructor
    async fn new(keystore: &KeystoreSender, header: HeaderHashed) -> Result<Self, KeystoreError> {
        let signature = header.author().sign(keystore, &*header).await?;
        Ok(Self::with_presigned(header, signature))
    }

    /// Validates a signed header
    async fn validate(&self) -> Result<(), KeystoreError> {
        if !self
            .header()
            .author()
            .verify_signature(self.signature(), self.header())
            .await?
        {
            return Err(KeystoreError::InvalidSignature(
                self.signature().clone(),
                format!("header {:?}", self.header_address()),
            ));
        }
        Ok(())
    }
}

// impl SignedHeaderHashed {

//     /// Constructor for an already signed header
//     pub fn with_presigned(header: HeaderHashed, signature: Signature) -> Self {
//         let signed_header = SignedHeader(header.as_content().clone(), signature);
//         Self {
//             header,
//             signed_header,
//         }
//     }

//     /// Break apart into a HeaderHashed and a Signature
//     pub fn into_header_and_signature(self) -> (HeaderHashed, Signature) {
//         (self.header, self.signed_header.1)
//     }

//     /// Access the Header Hash.
//     pub fn header_address(&self) -> &HeaderAddress {
//         self.header.as_hash()
//     }

//     /// Access the Header portion.
//     pub fn header(&self) -> &Header {
//         &self.header
//     }

//     /// Access the HeaderHashed portion.
//     pub fn header_hashed(&self) -> &HeaderHashed {
//         &self.header
//     }

//     /// Access the signature portion.
//     pub fn signature(&self) -> &Signature {
//         self.signed_header.signature()
//     }

// }

// impl From<HoloHashed<SignedHeader>> for SignedHeaderHashed {
//     fn from(hashed: HoloHashed<SignedHeader>) -> SignedHeaderHashed {
//         let (signed_header, hash) = hashed.into_inner();
//         SignedHeaderHashed {
//             header: HeaderHashed::with_pre_hashed(signed_header.header().clone(), hash),
//             signed_header,
//         }
//     }
// }

// impl From<SignedHeaderHashed> for HoloHashed<SignedHeader> {
//     fn from(shh: SignedHeaderHashed) -> HoloHashed<SignedHeader> {
//         let hash = shh.header.into_hash();
//         HoloHashed::with_pre_hashed(shh.signed_header, hash)
//     }
// }

impl WireElement {
    /// Convert into a [Element] when receiving from the network
    pub async fn into_element(self) -> Result<Element, SerializedBytesError> {
        Ok(Element::new(
            SignedHeaderHashed::with_data(self.signed_header).await?,
            self.maybe_entry,
        ))
    }
    /// Convert from a [Element] when sending to the network
    pub fn from_element(e: Element) -> Self {
        let (signed_header, maybe_entry) = e.into_inner();
        Self {
            signed_header: signed_header.into_inner().0,
            maybe_entry: maybe_entry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SignedHeader, SignedHeaderHashed};
    use crate::fixt::*;
    use ::fixt::prelude::*;
    use holo_hash::{HasHash, HoloHashed};

    #[tokio::test(threaded_scheduler)]
    async fn test_signed_header_roundtrip() {
        let signature = SignatureFixturator::new(Unpredictable).next().unwrap();
        let header = HeaderFixturator::new(Unpredictable).next().unwrap();
        let signed_header = SignedHeader(header, signature);
        let hashed: HoloHashed<SignedHeader> = HoloHashed::from_content(signed_header).await;
        let shh: SignedHeaderHashed = hashed.clone().into();

        assert_eq!(shh.header_address(), hashed.as_hash());

        let round: HoloHashed<SignedHeader> = shh.into();

        assert_eq!(hashed, round);
    }
}

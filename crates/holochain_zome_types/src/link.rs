use crate::element::SignedHeaderHashed;
use crate::ChainTopOrdering;
use holo_hash::HeaderHash;
use holochain_integrity_types::ToZomeName;
use holochain_integrity_types::ZomeName;
use holochain_serialized_bytes::prelude::*;

pub use holochain_integrity_types::link::*;

#[derive(
    Debug,
    PartialOrd,
    Ord,
    Clone,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    SerializedBytes,
)]
pub struct Link {
    /// The [`Entry`](crate::entry::Entry) being linked to
    pub target: holo_hash::AnyLinkableHash,
    /// When the link was added
    pub timestamp: crate::Timestamp,
    /// A tag used to find this link
    pub tag: LinkTag,
    /// The hash of this link's create header
    pub create_link_hash: HeaderHash,
}

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
/// The location of the links being queried.
pub enum LinkTypeQuery<Z = ZomeName> {
    /// All link types in this zome.
    AllTypes(Z),
    /// Only this link type in this zome.
    SingleType(Z, LinkType),
}

/// Zome IO inner type for link creation.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct CreateLinkInput {
    pub base_address: holo_hash::AnyLinkableHash,
    pub target_address: holo_hash::AnyLinkableHash,
    pub link_type: LinkType,
    pub tag: LinkTag,
    pub chain_top_ordering: ChainTopOrdering,
}

impl CreateLinkInput {
    pub fn new(
        base_address: holo_hash::AnyLinkableHash,
        target_address: holo_hash::AnyLinkableHash,
        type_location: LinkType,
        tag: LinkTag,
        chain_top_ordering: ChainTopOrdering,
    ) -> Self {
        Self {
            base_address,
            target_address,
            link_type: type_location,
            tag,
            chain_top_ordering,
        }
    }
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct DeleteLinkInput {
    /// Address of the link being deleted.
    pub address: holo_hash::HeaderHash,
    /// Chain top ordering rules for writes.
    pub chain_top_ordering: ChainTopOrdering,
}

impl DeleteLinkInput {
    pub fn new(address: holo_hash::HeaderHash, chain_top_ordering: ChainTopOrdering) -> Self {
        Self {
            address,
            chain_top_ordering,
        }
    }
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct GetLinksInput {
    pub base_address: holo_hash::AnyLinkableHash,
    pub link_type: LinkTypeRanges,
    pub tag_prefix: Option<crate::link::LinkTag>,
}

impl GetLinksInput {
    pub fn new(
        base_address: holo_hash::AnyLinkableHash,
        link_type: LinkTypeRanges,
        tag_prefix: Option<crate::link::LinkTag>,
    ) -> Self {
        Self {
            base_address,
            link_type,
            tag_prefix,
        }
    }
}

type CreateLinkWithDeleteLinks = Vec<(SignedHeaderHashed, Vec<SignedHeaderHashed>)>;
#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// CreateLinks with and DeleteLinks on them
/// `[CreateLink, [DeleteLink]]`
pub struct LinkDetails(CreateLinkWithDeleteLinks);

impl From<CreateLinkWithDeleteLinks> for LinkDetails {
    fn from(v: CreateLinkWithDeleteLinks) -> Self {
        Self(v)
    }
}

impl From<LinkDetails> for CreateLinkWithDeleteLinks {
    fn from(link_details: LinkDetails) -> Self {
        link_details.0
    }
}

impl LinkDetails {
    pub fn into_inner(self) -> CreateLinkWithDeleteLinks {
        self.into()
    }
}

impl ToZomeName for LinkTypeQuery {
    fn zome_name(&self) -> ZomeName {
        match self {
            LinkTypeQuery::AllTypes(z) | LinkTypeQuery::SingleType(z, _) => z.clone(),
        }
    }
}

impl From<ZomeName> for LinkTypeQuery {
    fn from(z: ZomeName) -> Self {
        LinkTypeQuery::AllTypes(z)
    }
}

//! Data structures representing the operations that can be performed within a Holochain DHT.
//!
//! See the [item-level documentation for `DhtOp`][DhtOp] for more details.
//!
//! [DhtOp]: enum.DhtOp.html

use crate::element::ChainElement;
use crate::{
    composite_hash::{AnyDhtHash, EntryHash},
    header,
    prelude::*,
    Header,
};
use error::{DhtOpError, DhtOpResult};
use header::NewEntryHeader;
use holochain_zome_types::{entry_def::EntryVisibility, Entry};
use serde::{Deserialize, Serialize};

#[allow(missing_docs)]
pub mod error;

/// A unit of DHT gossip. Used to notify an authority of new (meta)data to hold
/// as well as changes to the status of already held data.
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, Eq, PartialEq)]
pub enum DhtOp {
    /// Used to notify the authority for a header that it has been created.
    ///
    /// Conceptually, authorities receiving this `DhtOp` do three things:
    ///
    /// - Ensure that the element passes validation.
    /// - Store the header into their DHT shard.
    /// - Store the entry into their CAS.
    ///   - Note: they do not become responsible for keeping the set of
    ///     references from that entry up-to-date.
    StoreElement(Signature, Header, Option<Box<Entry>>),
    /// Used to notify the authority for an entry that it has been created
    /// anew. (The same entry can be created more than once.)
    ///
    /// Conceptually, authorities receiving this `DhtOp` do four things:
    ///
    /// - Ensure that the element passes validation.
    /// - Store the entry into their DHT shard.
    /// - Store the header into their CAS.
    ///   - Note: they do not become responsible for keeping the set of
    ///     references from that header up-to-date.
    /// - Add a "created-by" reference from the entry to the hash of the header.
    ///
    /// TODO: document how those "created-by" references are stored in
    /// reality.
    StoreEntry(Signature, NewEntryHeader, Box<Entry>),
    /// Used to notify the authority for an agent's public key that that agent
    /// has commited a new header.
    ///
    /// Conceptually, authorities receiving this `DhtOp` do three things:
    ///
    /// - Ensure that *the header alone* passes surface-level validation.
    /// - Store the header into their DHT shard.
    ///   - FIXME: @artbrock, do they?
    /// - Add an "agent-activity" reference from the public key to the hash
    ///   of the header.
    ///
    /// TODO: document how those "agent-activity" references are stored in
    /// reality.
    RegisterAgentActivity(Signature, Header),
    /// Op for updating an entry
    // TODO: This entry is here for validation by the entry update header holder
    // link's don't do this. The entry is validated by store entry. Maybe we either
    // need to remove the Entry here or add it to link.
    RegisterReplacedBy(Signature, header::EntryUpdate, Option<Box<Entry>>),
    /// Op for deleting an entry
    RegisterDeletedBy(Signature, header::EntryDelete),
    /// Op for adding a link  
    RegisterAddLink(Signature, header::LinkAdd),
    /// Op for removing a link
    RegisterRemoveLink(Signature, header::LinkRemove),
}

/// A type for storing in databases that don't need the actual
/// data. Everything is a hash of the type.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DhtOpLight {
    StoreElement(Signature, HeaderHash, Option<EntryHash>),
    StoreEntry(Signature, HeaderHash, EntryHash),
    RegisterAgentActivity(Signature, HeaderHash),
    RegisterReplacedBy(Signature, HeaderHash, EntryHash),
    RegisterDeletedBy(Signature, HeaderHash),
    RegisterAddLink(Signature, HeaderHash),
    RegisterRemoveLink(Signature, HeaderHash),
}

impl DhtOp {
    /// Returns the basis hash which determines which agents will receive this DhtOp
    pub async fn dht_basis(&self) -> DhtOpResult<AnyDhtHash> {
        Ok(match self {
            Self::StoreElement(_, header, _) => {
                let (_, hash): (_, HeaderHash) = header::HeaderHashed::with_data(header.clone())
                    .await?
                    .into();
                hash.into()
            }
            Self::StoreEntry(_, header, _) => header.entry().clone().into(),
            Self::RegisterAgentActivity(_, header) => header.author().clone().into(),
            Self::RegisterReplacedBy(_, header, _) => header.replaces_address.clone().into(),
            Self::RegisterDeletedBy(_, header) => header.removes_address.clone().into(),
            Self::RegisterAddLink(_, header) => header.base_address.clone().into(),
            Self::RegisterRemoveLink(_, header) => header.base_address.clone().into(),
        })
    }

    fn as_unique_form(&self) -> UniqueForm<'_> {
        match self {
            Self::StoreElement(_, header, _) => UniqueForm::StoreElement(header),
            Self::StoreEntry(_, header, _) => UniqueForm::StoreEntry(header),
            Self::RegisterAgentActivity(_, header) => UniqueForm::RegisterAgentActivity(header),
            Self::RegisterReplacedBy(_, header, _) => UniqueForm::RegisterReplacedBy(header),
            Self::RegisterDeletedBy(_, header) => UniqueForm::RegisterDeletedBy(header),
            Self::RegisterAddLink(_, header) => UniqueForm::RegisterAddLink(header),
            Self::RegisterRemoveLink(_, header) => UniqueForm::RegisterRemoveLink(header),
        }
    }
}

#[derive(Serialize)]
enum UniqueForm<'a> {
    // As an optimization, we don't include signatures. They would be redundant
    // with headers and therefore would waste hash/comparison time to include.
    StoreElement(&'a Header),
    StoreEntry(&'a NewEntryHeader),
    RegisterAgentActivity(&'a Header),
    RegisterReplacedBy(&'a header::EntryUpdate),
    RegisterDeletedBy(&'a header::EntryDelete),
    RegisterAddLink(&'a header::LinkAdd),
    RegisterRemoveLink(&'a header::LinkRemove),
}

impl DhtOpLight {
    /// Convert a [DhtOp] to a [DhtOpLight]
    pub async fn from_op(op: DhtOp) -> Result<Self, SerializedBytesError> {
        match op {
            DhtOp::StoreElement(s, h, _) => {
                let e = h.entry_data().map(|(e, _)| e.clone());
                let (_, h) = header::HeaderHashed::with_data(h).await?.into();
                Ok(DhtOpLight::StoreElement(s, h, e))
            }
            DhtOp::StoreEntry(s, h, _) => {
                let e = h.entry().clone();
                let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
                Ok(DhtOpLight::StoreEntry(s, h, e))
            }
            DhtOp::RegisterAgentActivity(s, h) => {
                let (_, h) = header::HeaderHashed::with_data(h).await?.into();
                Ok(DhtOpLight::RegisterAgentActivity(s, h))
            }
            DhtOp::RegisterReplacedBy(s, h, _) => {
                let e = h.entry_hash.clone();
                let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
                Ok(DhtOpLight::RegisterReplacedBy(s, h, e))
            }
            DhtOp::RegisterDeletedBy(s, h) => {
                let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
                Ok(DhtOpLight::RegisterAgentActivity(s, h))
            }
            DhtOp::RegisterAddLink(s, h) => {
                let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
                Ok(DhtOpLight::RegisterAgentActivity(s, h))
            }
            DhtOp::RegisterRemoveLink(s, h) => {
                let (_, h) = header::HeaderHashed::with_data(h.into()).await?.into();
                Ok(DhtOpLight::RegisterAgentActivity(s, h))
            }
        }
    }
}

/// Turn a chain element into a DhtOp
pub fn ops_from_element(element: &ChainElement) -> DhtOpResult<Vec<DhtOp>> {
    // TODO: avoid cloning everything

    let (signed_header, maybe_entry) = element.clone().into_inner();
    let (header, sig) = signed_header.into_header_and_signature();
    let (header, _): (Header, _) = header.into();

    // TODO: avoid allocation, we have a static maximum of four items and
    // callers simply want to iterate over the ops.
    //
    // Maybe use `ArrayVec`?
    let mut ops = vec![
        DhtOp::StoreElement(
            sig.clone(),
            header.clone(),
            maybe_entry.clone().map(Box::new),
        ),
        DhtOp::RegisterAgentActivity(sig.clone(), header.clone()),
    ];

    match &header {
        Header::Dna(_)
        | Header::ChainOpen(_)
        | Header::ChainClose(_)
        | Header::AgentValidationPkg(_)
        | Header::InitZomesComplete(_) => {}
        Header::LinkAdd(link_add) => ops.push(DhtOp::RegisterAddLink(sig, link_add.clone())),
        Header::LinkRemove(link_remove) => {
            ops.push(DhtOp::RegisterRemoveLink(sig, link_remove.clone()))
        }
        Header::EntryCreate(header) => match header.entry_type.visibility() {
            EntryVisibility::Public => ops.push(DhtOp::StoreEntry(
                sig,
                NewEntryHeader::Create(header.clone()),
                Box::new(
                    maybe_entry
                        .ok_or_else(|| DhtOpError::HeaderWithoutEntry(header.clone().into()))?,
                ),
            )),
            EntryVisibility::Private => {
                // This entry is private so remove it from StoreElement
                if let Some(DhtOp::StoreElement(_, _, e)) = ops.get_mut(0) {
                    *e = None;
                } else {
                    panic!("First op should always be store element");
                }
            }
        },
        Header::EntryUpdate(entry_update) => match entry_update.entry_type.visibility() {
            EntryVisibility::Public => {
                let entry = maybe_entry
                    .ok_or_else(|| DhtOpError::HeaderWithoutEntry(entry_update.clone().into()))?;
                ops.push(DhtOp::StoreEntry(
                    sig.clone(),
                    NewEntryHeader::Update(entry_update.clone()),
                    Box::new(entry.clone()),
                ));
                ops.push(DhtOp::RegisterReplacedBy(
                    sig,
                    entry_update.clone(),
                    Some(Box::new(entry)),
                ));
            }
            EntryVisibility::Private => {
                // This entry is private so remove it from StoreElement
                if let Some(DhtOp::StoreElement(_, _, e)) = ops.get_mut(0) {
                    *e = None;
                } else {
                    panic!("First op should always be store element");
                }
                ops.push(DhtOp::RegisterReplacedBy(sig, entry_update.clone(), None));
            }
        },
        Header::EntryDelete(entry_delete) => {
            ops.push(DhtOp::RegisterDeletedBy(sig, entry_delete.clone()))
        }
    }
    Ok(ops)
}

// This has to be done manually because the macro
// implements both directions and that isn't possible with references
// TODO: Maybe add a one-way version to holochain_serialized_bytes?
impl<'a> TryFrom<&UniqueForm<'a>> for SerializedBytes {
    type Error = SerializedBytesError;
    fn try_from(u: &UniqueForm<'a>) -> Result<Self, Self::Error> {
        match holochain_serialized_bytes::to_vec_named(u) {
            Ok(v) => Ok(SerializedBytes::from(
                holochain_serialized_bytes::UnsafeBytes::from(v),
            )),
            Err(e) => Err(SerializedBytesError::ToBytes(e.to_string())),
        }
    }
}

make_hashed_base! {
    Visibility(pub),
    HashedName(DhtOpHashed),
    ContentType(DhtOp),
    HashType(DhtOpHash),
}

impl DhtOpHashed {
    /// Create a hashed [DhtOp]
    pub async fn with_data(op: DhtOp) -> Self {
        let sb = SerializedBytes::try_from(&op.as_unique_form())
            .expect("`UniqueForm` must be serializable into MessagePack");
        DhtOpHashed::with_pre_hashed(op, DhtOpHash::with_data(UnsafeBytes::from(sb).into()).await)
    }
}

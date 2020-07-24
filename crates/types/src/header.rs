//! Holochain's [`Header`] and its variations.
//!
//! All header variations contain the fields `author` and `timestamp`.
//! Furthermore, all variations besides pub struct `Dna` (which is the first header
//! in a chain) contain the field `prev_header`.

#![allow(missing_docs)]

use crate::prelude::*;
use holo_hash::{EntryHash, HeaderAddress};
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::link::LinkTag;

pub mod builder;
pub use builder::{HeaderBuilder, HeaderBuilderCommon};
use holo_hash::impl_hashable_content;

/// Header contains variants for each type of header.
///
/// This struct really defines a local source chain, in the sense that it
/// implements the pointers between hashes that a hash chain relies on, which
/// are then used to check the integrity of data using cryptographic hash
/// functions.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
#[serde(tag = "type")]
pub enum Header {
    // The first header in a chain (for the DNA) doesn't have a previous header
    Dna(Dna),
    AgentValidationPkg(AgentValidationPkg),
    InitZomesComplete(InitZomesComplete),
    LinkAdd(LinkAdd),
    LinkRemove(LinkRemove),
    ChainOpen(ChainOpen),
    ChainClose(ChainClose),
    EntryCreate(EntryCreate),
    EntryUpdate(EntryUpdate),
    ElementDelete(ElementDelete),
}

/// a utility wrapper to write intos for our data types
macro_rules! write_into_header {
    ($($n:ident),*,) => {
        $(
            impl HeaderInner for $n {
                fn into_header(self) -> Header {
                    Header::$n(self)
                }
            }
        )*
    };
}

/// A trait to specify the common parts of a Header
pub trait HeaderInner {
    /// Get a full header from the subset
    fn into_header(self) -> Header;
}

impl<I: HeaderInner> From<I> for Header {
    fn from(i: I) -> Self {
        i.into_header()
    }
}

write_into_header! {
    Dna,
    AgentValidationPkg,
    InitZomesComplete,
    LinkAdd,
    LinkRemove,
    ChainOpen,
    ChainClose,
    EntryCreate,
    EntryUpdate,
    ElementDelete,
}

/// a utility macro just to not have to type in the match statement everywhere.
macro_rules! match_header {
    ($h:ident => |$i:ident| { $($t:tt)* }) => {
        match $h {
            Header::Dna($i) => { $($t)* }
            Header::AgentValidationPkg($i) => { $($t)* }
            Header::InitZomesComplete($i) => { $($t)* }
            Header::LinkAdd($i) => { $($t)* }
            Header::LinkRemove($i) => { $($t)* }
            Header::ChainOpen($i) => { $($t)* }
            Header::ChainClose($i) => { $($t)* }
            Header::EntryCreate($i) => { $($t)* }
            Header::EntryUpdate($i) => { $($t)* }
            Header::ElementDelete($i) => { $($t)* }
        }
    };
}

impl Header {
    /// Returns the address and entry type of the Entry, if applicable.
    // TODO: DRY: possibly create an `EntryData` struct which is used by both
    // EntryCreate and EntryUpdate
    pub fn entry_data(&self) -> Option<(&EntryHash, &EntryType)> {
        match self {
            Self::EntryCreate(EntryCreate {
                entry_hash,
                entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            Self::EntryUpdate(EntryUpdate {
                entry_hash,
                entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            _ => None,
        }
    }

    /// returns the public key of the agent who signed this header.
    pub fn author(&self) -> &AgentPubKey {
        match_header!(self => |i| { &i.author })
    }

    /// returns the timestamp of when the header was created
    pub fn timestamp(&self) -> Timestamp {
        match_header!(self => |i| { i.timestamp })
    }

    /// returns the sequence ordinal of this header
    pub fn header_seq(&self) -> u32 {
        match_header!(self => |i| { i.header_seq })
    }

    /// returns the previous header except for the DNA header which doesn't have a previous
    pub fn prev_header(&self) -> Option<&HeaderAddress> {
        Some(match self {
            Self::Dna(Dna { .. }) => return None,
            Self::AgentValidationPkg(AgentValidationPkg { prev_header, .. }) => prev_header,
            Self::InitZomesComplete(InitZomesComplete { prev_header, .. }) => prev_header,
            Self::LinkAdd(LinkAdd { prev_header, .. }) => prev_header,
            Self::LinkRemove(LinkRemove { prev_header, .. }) => prev_header,
            Self::ElementDelete(ElementDelete { prev_header, .. }) => prev_header,
            Self::ChainClose(ChainClose { prev_header, .. }) => prev_header,
            Self::ChainOpen(ChainOpen { prev_header, .. }) => prev_header,
            Self::EntryCreate(EntryCreate { prev_header, .. }) => prev_header,
            Self::EntryUpdate(EntryUpdate { prev_header, .. }) => prev_header,
        })
    }
}

pub type HeaderHashed = HoloHashed<Header>;

impl_hashable_content!(Header, Header);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
/// A header of one of the two types that create a new entry.
pub enum NewEntryHeader {
    /// A header which simply creates a new entry
    Create(EntryCreate),
    /// A header which creates a new entry that is semantically related to a
    /// previously created entry or header
    Update(EntryUpdate),
}

impl NewEntryHeader {
    /// Get the entry on this header
    pub fn entry(&self) -> &EntryHash {
        match self {
            NewEntryHeader::Create(EntryCreate { entry_hash, .. })
            | NewEntryHeader::Update(EntryUpdate { entry_hash, .. }) => entry_hash,
        }
    }

    /// Get the visibility of this header
    pub fn visibility(&self) -> &EntryVisibility {
        match self {
            NewEntryHeader::Create(EntryCreate { entry_type, .. })
            | NewEntryHeader::Update(EntryUpdate { entry_type, .. }) => entry_type.visibility(),
        }
    }
}

impl From<NewEntryHeader> for Header {
    fn from(h: NewEntryHeader) -> Self {
        match h {
            NewEntryHeader::Create(h) => Header::EntryCreate(h),
            NewEntryHeader::Update(h) => Header::EntryUpdate(h),
        }
    }
}

/// this id is an internal reference, which also serves as a canonical ordering
/// for zome initialization.  The value should be auto-generated from the Zome Bundle def
// TODO: Check this can never be written to > 255
#[derive(
    Debug,
    Copy,
    Clone,
    Hash,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    SerializedBytes,
    derive_more::Display,
    derive_more::From,
    derive_more::Into,
)]
pub struct ZomeId(u8);

#[derive(
    Debug,
    Copy,
    Clone,
    Hash,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    SerializedBytes,
    derive_more::Display,
    derive_more::From,
    derive_more::Into,
)]
pub struct EntryDefId(u8);

/// Specifies whether an [EntryUpdate] refers to an [Entry] or a [Header]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub enum IntendedFor {
    Header,
    Entry,
}

/// The Dna Header is always the first header in a source chain
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct Dna {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    // No previous header, because DNA is always first chain entry
    pub hash: DnaHash,
}

/// Header for an agent validation package, used to determine whether an agent
/// is allowed to participate in this DNA
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct AgentValidationPkg {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    pub membrane_proof: Option<SerializedBytes>,
}

/// A header which declares that all zome init functions have successfully
/// completed, and the chain is ready for commits. Contains no explicit data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct InitZomesComplete {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,
}

/// Declares that a metadata Link should be made between two EntryHashes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct LinkAdd {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    pub base_address: EntryHash,
    pub target_address: EntryHash,
    pub zome_id: ZomeId,
    pub tag: LinkTag,
}

/// Declares that a previously made Link should be nullified and considered removed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct LinkRemove {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,
    pub base_address: EntryHash,
    /// The address of the `LinkAdd` being reversed
    pub link_add_address: HeaderAddress,
}

/// When migrating to a new version of a DNA, this header is committed to the
/// new chain to declare the migration path taken. **Currently unused**
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct ChainOpen {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    pub prev_dna_hash: DnaHash,
}

/// When migrating to a new version of a DNA, this header is committed to the
/// old chain to declare the migration path taken. **Currently unused**
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct ChainClose {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    pub new_dna_hash: DnaHash,
}

/// A header which "speaks" Entry content into being. The same content can be
/// referenced by multiple such headers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct EntryCreate {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
}

/// A header which specifies that some new Entry content is intended to be an
/// update to some old Entry.
///
/// The update may refer to either a previous Header, or a previous Entry, via
/// the `intended_for` field. The update is registered as metadata on the
/// intended target, the result of which is is that both Headers and Entries can
/// have a tree of such metadata update references. Entries get "updated" to
/// other entries, and Headers get "updated" to other headers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct EntryUpdate {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    pub intended_for: IntendedFor,
    pub replaces_address: HeaderHash,

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
}

/// Declare that a previously published Header should be nullified and
/// considered deleted.
///
/// Via the associated [DhtOp], this also has an effect on Entries: namely,
/// that a previously published Entry will become inaccessible if all of its
/// Headers are marked deleted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct ElementDelete {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    /// Address of the Element being deleted
    pub removes_address: HeaderAddress,
}

/// Allows Headers which reference Entries to know what type of Entry it is
/// referencing. Useful for examining Headers without needing to fetch the
/// corresponding Entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub enum EntryType {
    /// An AgentPubKey
    AgentPubKey,
    /// An app-provided entry, along with its app-provided AppEntryType
    App(AppEntryType),
    /// A Capability claim
    CapClaim,
    /// A Capability grant.
    CapGrant,
}

impl EntryType {
    pub fn visibility(&self) -> &EntryVisibility {
        match self {
            EntryType::AgentPubKey => &EntryVisibility::Public,
            EntryType::App(t) => &t.visibility(),
            EntryType::CapClaim => &EntryVisibility::Private,
            EntryType::CapGrant => &EntryVisibility::Private,
        }
    }
}

/// Information about a class of Entries provided by the DNA
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct AppEntryType {
    /// u8 identifier of what entry type this is
    /// this needs to match the position of the entry type returned by entry defs
    pub(crate) id: EntryDefId,
    /// u8 identifier of what zome this is for
    /// this needs to be shared across the dna
    /// comes from the numeric index position of a zome in dna config
    pub(crate) zome_id: ZomeId,
    // @todo don't do this, use entry defs instead
    pub(crate) visibility: EntryVisibility,
}

impl AppEntryType {
    pub fn new(id: EntryDefId, zome_id: ZomeId, visibility: EntryVisibility) -> Self {
        Self {
            id,
            zome_id,
            visibility,
        }
    }

    pub fn id(&self) -> EntryDefId {
        self.id
    }
    pub fn zome_id(&self) -> ZomeId {
        self.zome_id
    }
    pub fn visibility(&self) -> &EntryVisibility {
        &self.visibility
    }
}

impl Dna {
    /// The Dna header can't implement HeaderBuilder because it lacks a
    /// `prev_header` field, so this helper is provided as a special case
    pub fn from_builder(hash: DnaHash, builder: HeaderBuilderCommon) -> Self {
        Self {
            author: builder.author,
            timestamp: builder.timestamp,
            header_seq: builder.header_seq,
            hash,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fixt::HeaderBuilderCommonFixturator,
        test_utils::{fake_dna_hash, fake_entry_content_hash},
    };
    use ::fixt::prelude::*;

    #[test]
    fn test_header_msgpack_roundtrip() {
        let orig: Header = Dna::from_builder(
            fake_dna_hash(1),
            HeaderBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
        )
        .into();
        let bytes = rmp_serde::to_vec_named(&orig).unwrap();
        let res: Header = rmp_serde::from_read_ref(&bytes).unwrap();
        assert_eq!(orig, res);
    }

    #[test]
    fn test_entrycreate_msgpack_roundtrip() {
        let orig: Header = EntryCreate::from_builder(
            HeaderBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
            EntryType::App(AppEntryType::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            fake_entry_content_hash(1).into(),
        )
        .into();
        let bytes = rmp_serde::to_vec_named(&orig).unwrap();
        println!("{:?}", bytes);
        let res: Header = rmp_serde::from_read_ref(&bytes).unwrap();
        assert_eq!(orig, res);
    }

    #[test]
    fn test_entrycreate_serializedbytes_roundtrip() {
        let orig: Header = EntryCreate::from_builder(
            HeaderBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
            EntryType::App(AppEntryType::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            fake_entry_content_hash(1).into(),
        )
        .into();
        let bytes: SerializedBytes = orig.clone().try_into().unwrap();
        let res: Header = bytes.try_into().unwrap();
        assert_eq!(orig, res);
    }
}

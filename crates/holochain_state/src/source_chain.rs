use holo_hash::AgentPubKey;
use holo_hash::HasHash;
use holo_hash::HeaderHash;
use holochain_keystore::KeystoreSender;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::dht_op::produce_op_lights_from_elements;
use holochain_types::dht_op::produce_op_lights_from_iter;
use holochain_types::dht_op::DhtOpLight;
use holochain_types::dht_op::OpOrder;
use holochain_types::dht_op::UniqueForm;
use holochain_types::element::SignedHeaderHashedExt;
use holochain_types::env::EnvRead;
use holochain_types::timestamp;
use holochain_types::EntryHashed;
use holochain_zome_types::CapGrant;
use holochain_zome_types::CapSecret;
use holochain_zome_types::Element;
use holochain_zome_types::Entry;
use holochain_zome_types::GrantedFunction;
use holochain_zome_types::Header;
use holochain_zome_types::HeaderBuilder;
use holochain_zome_types::HeaderBuilderCommon;
use holochain_zome_types::HeaderHashed;
use holochain_zome_types::HeaderInner;
use holochain_zome_types::QueryFilter;
use holochain_zome_types::Signature;
use holochain_zome_types::SignedHeader;
use holochain_zome_types::SignedHeaderHashed;

use crate::prelude::*;
use crate::query::chain_head::ChainHeadQuery;
use crate::scratch::Scratch;
use crate::scratch::SyncScratch;
use holochain_serialized_bytes::prelude::*;

pub use error::*;

mod error;
#[derive(Clone)]
pub struct SourceChain {
    scratch: SyncScratch,
    vault: EnvRead,
    author: Arc<AgentPubKey>,
    persisted_len: u32,
    persisted_head: HeaderHash,
    public_only: bool,
}

// TODO fix this.  We shouldn't really have nil values but this would
// show if the database is corrupted and doesn't have an element
#[derive(Serialize, Deserialize)]
pub struct SourceChainJsonDump {
    pub elements: Vec<Option<SourceChainJsonElement>>,
    pub published_ops_count: usize,
}

#[derive(Serialize, Deserialize)]
pub struct SourceChainJsonElement {
    pub signature: Signature,
    pub header_address: HeaderHash,
    pub header: Header,
    pub entry: Option<Entry>,
}

impl SourceChain {
    pub fn new(vault: EnvRead, author: AgentPubKey) -> SourceChainResult<Self> {
        let scratch = Scratch::new().into_sync();
        let author = Arc::new(author);
        let (persisted_head, persisted_len) = vault
            .conn()?
            .with_reader(|txn| chain_head_db(&txn, author.clone()))?;
        Ok(Self {
            scratch,
            vault,
            author,
            persisted_len,
            persisted_head,
            public_only: false,
        })
    }
    pub fn public_only(&mut self) {
        self.public_only = true;
    }
    /// Take a snapshot of the scratch space that will
    /// not remain in sync with future updates.
    pub fn snapshot(&self) -> SourceChainResult<Scratch> {
        Ok(self.scratch.apply(|scratch| scratch.clone())?)
    }

    pub fn scratch(&self) -> SyncScratch {
        self.scratch.clone()
    }

    pub fn agent_pubkey(&self) -> &AgentPubKey {
        self.author.as_ref()
    }

    /// This has to clone all the data because we can't return
    /// references to constructed data.
    // TODO: Maybe we should store data as elements in the scratch?
    pub fn elements(&self) -> SourceChainResult<Vec<Element>> {
        Ok(self.scratch.apply(|scratch| scratch.elements().collect())?)
    }

    pub async fn put<H: HeaderInner, B: HeaderBuilder<H>>(
        &self,
        header_builder: B,
        maybe_entry: Option<Entry>,
    ) -> SourceChainResult<HeaderHash> {
        // Check scratch for newer head.
        let (prev_header, header_seq) = self.scratch.apply(|scratch| {
            let chain_head = chain_head_scratch(&(*scratch), self.author.as_ref());
            let (prev_header, chain_len) =
                chain_head.unwrap_or_else(|| (self.persisted_head.clone(), self.persisted_len));
            let header_seq = chain_len + 1;
            (prev_header, header_seq)
        })?;

        // Build the header.
        let common = HeaderBuilderCommon {
            author: (*self.author).clone(),
            timestamp: timestamp::now(),
            header_seq,
            prev_header,
        };
        let header = header_builder.build(common).into();
        let header = HeaderHashed::from_content_sync(header);
        let hash = header.as_hash().clone();

        // Sign the header.
        let header = SignedHeaderHashed::new(self.vault.keystore(), header).await?;
        let element = Element::new(header, maybe_entry);

        // Put into scratch.
        self.scratch
            .apply(|scratch| insert_element_scratch(scratch, element))?;
        Ok(hash)
    }

    pub fn has_initialized(&self) -> SourceChainResult<bool> {
        Ok(self.len()? > 3)
    }

    pub fn len(&self) -> SourceChainResult<u32> {
        Ok(self.scratch.apply(|scratch| {
            let scratch_max = chain_head_scratch(&(*scratch), self.author.as_ref()).map(|(_, s)| s);
            scratch_max
                .map(|s| std::cmp::max(s, self.persisted_len))
                .unwrap_or(self.persisted_len)
        })?)
    }
    pub fn valid_cap_grant(
        &self,
        check_function: &GrantedFunction,
        check_agent: &AgentPubKey,
        check_secret: Option<&CapSecret>,
    ) -> SourceChainResult<Option<CapGrant>> {
        todo!("Implement cap query")
    }

    /// Query Headers in the source chain.
    /// This returns a Vec rather than an iterator because it is intended to be
    /// used by the `query` host function, which crosses the wasm boundary
    // FIXME: This query needs to be tested.
    pub fn query(&self, query: &QueryFilter) -> SourceChainResult<Vec<Element>> {
        let (range_min, range_max) = match query.sequence_range.clone() {
            Some(range) => (Some(range.start), Some(range.end)),
            None => (None, None),
        };
        let mut elements = self.vault.conn()?.with_reader(|txn| {
            let mut sql = "
                SELECT 
                Header.hash AS header_hash, Header.blob AS header_blob
            "
            .to_string();
            if query.include_entries {
                sql.push_str(
                    "
                    , Entry.blob AS entry_blob
                    ",
                );
            }
            sql.push_str(
                "
                FROM Header
                ",
            );
            if query.include_entries {
                sql.push_str(
                    "
                    LEFT JOIN Entry On Header.entry_hash = Entry.hash
                    ",
                );
            }
            sql.push_str(
                "
                JOIN DhtOp On DhtOp.header_hash = Header.hash
                WHERE
                Header.author = :author
                AND
                DhtOp.is_authored = 1
                AND
                (:range_min IS NULL OR Header.seq >= :range_min)
                AND
                (:range_max IS NULL OR Header.seq < :range_max)
                AND
                (:entry_type IS NULL OR Header.entry_type = :entry_type)
                AND
                (:header_type IS NULL OR Header.type = :header_type)
                ",
            );
            let mut stmt = txn.prepare(&sql)?;
            let elements = stmt
                .query_and_then(
                    named_params! {
                        ":author": self.author.as_ref(),
                        ":range_min": range_min,
                        ":range_max": range_max,
                        ":entry_type": query.entry_type,
                        ":header_type": query.header_type,
                    },
                    |row| {
                        let header = from_blob::<SignedHeader>(row.get("header_blob")?)?;
                        let SignedHeader(header, signature) = header;
                        let hash: HeaderHash = row.get("header_hash")?;
                        let header = HeaderHashed::with_pre_hashed(header, hash);
                        let shh = SignedHeaderHashed::with_presigned(header, signature);
                        let entry = if query.include_entries {
                            let entry = from_blob::<Entry>(row.get("entry_blob")?)?;
                            Some(entry)
                        } else {
                            None
                        };
                        StateQueryResult::Ok(Element::new(shh, entry))
                    },
                )?
                .collect::<StateQueryResult<Vec<_>>>();
            elements
        })?;
        self.scratch.apply(|scratch| {
            let scratch_iter = scratch
                .headers()
                .filter(|shh| query.check(shh.header()))
                .filter_map(|shh| {
                    let entry = match shh.header().entry_hash() {
                        Some(eh) if query.include_entries => scratch.get_entry(eh).ok()?,
                        _ => None,
                    };
                    Some(Element::new(shh.clone(), entry))
                });
            elements.extend(scratch_iter);
        })?;
        Ok(elements)
    }

    pub fn flush(&self) -> SourceChainResult<()> {
        // Nothing to write
        if self.scratch.apply(|s| s.is_empty())? {
            return Ok(());
        }
        let (headers, ops, entries) = self.scratch.apply_and_then(|scratch| {
            let length = scratch.num_headers();

            // The op related data ends up here.
            let mut ops = Vec::with_capacity(length);

            // Drain out the headers.
            let signed_headers = scratch.drain_headers().collect::<Vec<_>>();
            // Headers end up back in here.
            let mut headers = Vec::with_capacity(signed_headers.len());

            // Loop through each header and produce op related data.
            for shh in signed_headers {
                // &HeaderHash, &Header, EntryHash are needed to produce the ops.
                let entry_hash = shh.header().entry_hash().cloned();
                let item = (shh.as_hash(), shh.header(), entry_hash);
                let ops_inner = produce_op_lights_from_iter(vec![item].into_iter(), 1)?;

                // Break apart the SignedHeaderHashed.
                let (header, sig) = shh.into_header_and_signature();
                let (header, hash) = header.into_inner();

                // We need to take the header by value and put it back each loop.
                let mut h = Some(header);
                for op in ops_inner {
                    let op_type = op.get_type();
                    // Header is required by value to produce the DhtOpHash.
                    let (header, op_hash) =
                        UniqueForm::op_hash(op_type, h.expect("This can't be empty"))?;
                    let op_order = OpOrder::new(op_type, header.timestamp());
                    // Put the header back by value.
                    h = Some(header);
                    // Collect the DhtOpLight, DhtOpHash and OpOrder.
                    ops.push((op, op_hash, op_order));
                }

                // Put the SignedHeaderHashed back together.
                let shh = SignedHeaderHashed::with_presigned(
                    HeaderHashed::with_pre_hashed(h.expect("This can't be empty"), hash),
                    sig,
                );
                // Put the header back in the list.
                headers.push(shh);
            }

            // Drain out any entries.
            let entries = scratch.drain_entries().collect::<Vec<_>>();
            SourceChainResult::Ok((headers, ops, entries))
        })?;

        // Write the entries, headers and ops to the database in one transaction.
        self.vault.conn()?.with_commit(|txn| {
            // As at check.
            let (new_persisted_head, _) = chain_head_db(&txn, self.author.clone())?;
            match headers.last().map(|shh| shh.header_address()) {
                Some(scratch_head) => {
                    if self.persisted_head != new_persisted_head {
                        return Err(SourceChainError::HeadMoved(
                            Some(self.persisted_head.clone()),
                            Some(new_persisted_head),
                        ));
                    }
                }
                // Nothing to write
                None => return Ok(()),
            }

            for entry in entries {
                insert_entry(txn, entry)?;
            }
            for header in headers {
                insert_header(txn, header)?;
            }
            for (op, op_hash, op_order) in ops {
                insert_op_lite(txn, op, op_hash, true, op_order)?;
            }
            SourceChainResult::Ok(())
        })?;
        Ok(())
    }
}

pub fn put_raw(
    txn: &mut Transaction,
    shh: SignedHeaderHashed,
    ops: Vec<DhtOpLight>,
    entry: Option<Entry>,
) -> StateMutationResult<()> {
    let (header, signature) = shh.into_header_and_signature();
    let (header, hash) = header.into_inner();
    let mut header = Some(header);
    let mut hashes = Vec::with_capacity(ops.len());
    for op in &ops {
        let op_type = op.get_type();
        let (h, op_hash) =
            UniqueForm::op_hash(op_type, header.take().expect("This can't be empty"))?;
        let op_order = OpOrder::new(op_type, h.timestamp());
        header = Some(h);
        hashes.push((op_hash, op_order));
    }
    let shh = SignedHeaderHashed::with_presigned(
        HeaderHashed::with_pre_hashed(header.expect("This can't be empty"), hash),
        signature,
    );
    if let Some(entry) = entry {
        insert_entry(txn, EntryHashed::from_content_sync(entry))?;
    }
    insert_header(txn, shh)?;
    for (op, (op_hash, op_order)) in ops.into_iter().zip(hashes) {
        insert_op_lite(txn, op, op_hash, true, op_order)?;
    }
    Ok(())
}

fn chain_head_db(
    txn: &Transaction,
    author: Arc<AgentPubKey>,
) -> SourceChainResult<(HeaderHash, u32)> {
    let chain_head = ChainHeadQuery::new(author);
    let (prev_header, last_header_seq) = chain_head
        .run(Txn::from(txn))?
        .ok_or(SourceChainError::ChainEmpty)?;
    Ok((prev_header, last_header_seq))
}

fn chain_head_scratch(scratch: &Scratch, author: &AgentPubKey) -> Option<(HeaderHash, u32)> {
    scratch
        .headers()
        .filter_map(|shh| {
            if shh.header().author() == author {
                Some((shh.header_address().clone(), shh.header().header_seq()))
            } else {
                None
            }
        })
        .max_by_key(|h| h.1)
}

async fn put_db<H: HeaderInner, B: HeaderBuilder<H>>(
    txn: &mut Transaction<'_>,
    keystore: &KeystoreSender,
    author: Arc<AgentPubKey>,
    header_builder: B,
    maybe_entry: Option<Entry>,
) -> SourceChainResult<HeaderHash> {
    let (prev_header, last_header_seq) = chain_head_db(txn, author.clone())?;
    let header_seq = last_header_seq + 1;

    let common = HeaderBuilderCommon {
        author: (*author).clone(),
        timestamp: timestamp::now(),
        header_seq,
        prev_header,
    };
    let header = header_builder.build(common).into();
    let header = HeaderHashed::from_content_sync(header);
    let header = SignedHeaderHashed::new(&keystore, header).await?;
    let element = Element::new(header, maybe_entry);
    let ops = produce_op_lights_from_elements(vec![&element])?;
    let (header, entry) = element.into_inner();
    let entry = entry.into_option();
    let hash = header.as_hash().clone();
    put_raw(txn, header, ops, entry)?;
    Ok(hash)
}

/// dump the entire source chain as a pretty-printed json string
pub async fn dump_state(
    vault: EnvRead,
    author: &AgentPubKey,
) -> Result<SourceChainJsonDump, SourceChainError> {
    // let mut iter = self.iter_back();
    // let mut elements = Vec::new();
    // let mut published_ops_count = 0;

    // while let Some(h) = iter.next()? {
    //     let maybe_element = self.get_element(h.header_address())?;
    //     match maybe_element {
    //         None => elements.push(None),
    //         Some(element) => {
    //             let ops = produce_op_lights_from_elements(vec![&element]).unwrap();
    //             published_ops_count += if element
    //                 .header()
    //                 .entry_type()
    //                 .map(|e| *e.visibility() == EntryVisibility::Public)
    //                 .unwrap_or(true)
    //             {
    //                 ops.len()
    //             } else {
    //                 ops.into_iter()
    //                     .filter(|op| !matches!(&op, DhtOpLight::StoreEntry(_, _, _)))
    //                     .count()
    //             };
    //             let (signed, entry) = element.into_inner();
    //             let (header, signature) = signed.into_header_and_signature();
    //             let (header, header_address) = header.into_inner();
    //             elements.push(Some(SourceChainJsonElement {
    //                 signature,
    //                 header_address,
    //                 header,
    //                 entry: entry.into_option(),
    //             }));
    //         }
    //     }
    // }

    // Ok(SourceChainJsonDump {
    //     elements,
    //     published_ops_count,
    // })
    todo!()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::prelude::*;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_types::test_utils::fake_dna_hash;
    use holochain_zome_types::capability::CapAccess;
    use holochain_zome_types::capability::ZomeCallCapGrant;

    use std::collections::HashSet;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_cap_grant() -> SourceChainResult<()> {
        todo!("re-write as sql test");
        //     let test_env = test_cell_env();
        //     let env = test_env.env();
        //     let secret = Some(CapSecretFixturator::new(Unpredictable).next().unwrap());
        //     let access = CapAccess::from(secret.unwrap());

        //     // @todo curry
        //     let _curry = CurryPayloadsFixturator::new(Empty).next().unwrap();
        //     let function: GrantedFunction = ("foo".into(), "bar".into());
        //     let mut functions: GrantedFunctions = HashSet::new();
        //     functions.insert(function.clone());
        //     let grant = ZomeCallCapGrant::new("tag".into(), access.clone(), functions.clone());
        //     let mut agents = AgentPubKeyFixturator::new(Predictable);
        //     let alice = agents.next().unwrap();
        //     let bob = agents.next().unwrap();
        //     {
        //         let mut store = SourceChainBuf::new(env.clone().into())?;
        //         store.genesis(fake_dna_hash(1), alice.clone(), None).await?;
        //         env.conn()
        //             .unwrap()
        //             .with_commit(|writer| store.flush_to_txn(writer))?;
        //     }

        //     {
        //         let chain = SourceChain::new(env.clone().into())?;
        //         assert_eq!(
        //             chain.valid_cap_grant(&function, &alice, secret.as_ref())?,
        //             Some(CapGrant::ChainAuthor(alice.clone())),
        //         );

        //         // bob should not match anything as the secret hasn't been committed yet
        //         assert_eq!(
        //             chain.valid_cap_grant(&function, &bob, secret.as_ref())?,
        //             None
        //         );
        //     }

        //     let (original_header_address, original_entry_address) = {
        //         let mut chain = SourceChain::new(env.clone().into())?;
        //         let (entry, entry_hash) =
        //             EntryHashed::from_content_sync(Entry::CapGrant(grant.clone())).into_inner();
        //         let header_builder = builder::Create {
        //             entry_type: EntryType::CapGrant,
        //             entry_hash: entry_hash.clone(),
        //         };
        //         let header = chain.put(header_builder, Some(entry)).await?;

        //         env.conn()
        //             .unwrap()
        //             .with_commit(|writer| chain.flush_to_txn(writer))?;

        //         (header, entry_hash)
        //     };

        //     {
        //         let chain = SourceChain::new(env.clone().into())?;
        //         // alice should find her own authorship with higher priority than the committed grant
        //         // even if she passes in the secret
        //         assert_eq!(
        //             chain.valid_cap_grant(&function, &alice, secret.as_ref())?,
        //             Some(CapGrant::ChainAuthor(alice.clone())),
        //         );

        //         // bob should be granted with the committed grant as it matches the secret he passes to
        //         // alice at runtime
        //         assert_eq!(
        //             chain.valid_cap_grant(&function, &bob, secret.as_ref())?,
        //             Some(grant.clone().into())
        //         );
        //     }

        //     // let's roll the secret and assign the grant to bob specifically
        //     let mut assignees = HashSet::new();
        //     assignees.insert(bob.clone());
        //     let updated_secret = Some(CapSecretFixturator::new(Unpredictable).next().unwrap());
        //     let updated_access = CapAccess::from((updated_secret.clone().unwrap(), assignees));
        //     let updated_grant = ZomeCallCapGrant::new("tag".into(), updated_access.clone(), functions);

        //     let (updated_header_hash, updated_entry_hash) = {
        //         let mut chain = SourceChain::new(env.clone().into())?;
        //         let (entry, entry_hash) =
        //             EntryHashed::from_content_sync(Entry::CapGrant(updated_grant.clone())).into_inner();
        //         let header_builder = builder::Update {
        //             entry_type: EntryType::CapGrant,
        //             entry_hash: entry_hash.clone(),
        //             original_header_address,
        //             original_entry_address,
        //         };
        //         let header = chain.put(header_builder, Some(entry)).await?;

        //         env.conn()
        //             .unwrap()
        //             .with_commit(|writer| chain.flush_to_txn(writer))?;

        //         (header, entry_hash)
        //     };

        //     {
        //         let chain = SourceChain::new(env.clone().into())?;
        //         // alice should find her own authorship with higher priority than the committed grant
        //         // even if she passes in the secret
        //         assert_eq!(
        //             chain.valid_cap_grant(&function, &alice, secret.as_ref())?,
        //             Some(CapGrant::ChainAuthor(alice.clone())),
        //         );
        //         assert_eq!(
        //             chain.valid_cap_grant(&function, &alice, updated_secret.as_ref())?,
        //             Some(CapGrant::ChainAuthor(alice.clone())),
        //         );

        //         // bob MUST provide the updated secret as the old one is invalidated by the new one
        //         assert_eq!(
        //             chain.valid_cap_grant(&function, &bob, secret.as_ref())?,
        //             None
        //         );
        //         assert_eq!(
        //             chain.valid_cap_grant(&function, &bob, updated_secret.as_ref())?,
        //             Some(updated_grant.into())
        //         );
        //     }

        //     {
        //         let mut chain = SourceChain::new(env.clone().into())?;
        //         let header_builder = builder::Delete {
        //             deletes_address: updated_header_hash,
        //             deletes_entry_address: updated_entry_hash,
        //         };
        //         chain.put(header_builder, None).await?;

        //         env.conn()
        //             .unwrap()
        //             .with_commit(|writer| chain.flush_to_txn(writer))?;
        //     }

        //     {
        //         let chain = SourceChain::new(env.clone().into())?;
        //         // alice should find her own authorship
        //         assert_eq!(
        //             chain.valid_cap_grant(&function, &alice, secret.as_ref())?,
        //             Some(CapGrant::ChainAuthor(alice.clone())),
        //         );
        //         assert_eq!(
        //             chain.valid_cap_grant(&function, &alice, updated_secret.as_ref())?,
        //             Some(CapGrant::ChainAuthor(alice)),
        //         );

        //         // bob has no access
        //         assert_eq!(
        //             chain.valid_cap_grant(&function, &bob, secret.as_ref())?,
        //             None
        //         );
        //         assert_eq!(
        //             chain.valid_cap_grant(&function, &bob, updated_secret.as_ref())?,
        //             None
        //         );
        //     }

        //     Ok(())
        // }

        // @todo bring all this back when we want to administer cap claims better
        // #[tokio::test(flavor = "multi_thread")]
        // async fn test_get_cap_claim() -> SourceChainResult<()> {
        //     let test_env = test_cell_env();
        //     let env = test_env.env();
        //     let env = env.conn().unwrap().await;
        //     let secret = CapSecretFixturator::new(Unpredictable).next().unwrap();
        //     let agent_pubkey = fake_agent_pubkey_1().into();
        //     let claim = CapClaim::new("tag".into(), agent_pubkey, secret.clone());
        //     {
        //         let mut store = SourceChainBuf::new(env.clone().into(), &env).await?;
        //         store
        //             .genesis(fake_dna_hash(1), fake_agent_pubkey_1(), None)
        //             .await?;
        //         arc.conn().unwrap().with_commit(|writer| store.flush_to_txn(writer))?;
        //     }
        //
        //     {
        //         let mut chain = SourceChain::new(env.clone().into(), &env).await?;
        //         chain.put_cap_claim(claim.clone()).await?;
        //
        // // ideally the following would work, but it won't because currently
        // // we can't get claims from the scratch space
        // // this will be fixed once we add the capability index
        //
        // // assert_eq!(
        // //     chain.get_persisted_cap_claim_by_secret(&secret)?,
        // //     Some(claim.clone())
        // // );
        //
        //         arc.conn().unwrap().with_commit(|writer| chain.flush_to_txn(writer))?;
        //     }
        //
        //     {
        //         let chain = SourceChain::new(env.clone().into(), &env).await?;
        //         assert_eq!(
        //             chain.get_persisted_cap_claim_by_secret(&secret).await?,
        //             Some(claim)
        //         );
        //     }
        //
        //     Ok(())
    }
}

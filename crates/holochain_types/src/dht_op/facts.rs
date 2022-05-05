//! Facts about DhtOps

use super::*;
use ::contrafact::*;
use holochain_keystore::MetaLairClient;

/// Fact: The DhtOp is internally consistent in all of its references:
/// - TODO: The DhtOp variant matches the Header variant
/// - The Signature matches the Header
/// - If the header references an Entry, the Entry will exist and be of the appropriate hash
/// - If the header does not reference an Entry, the entry will be None
pub fn valid_dht_op<'a>(
    keystore: MetaLairClient,
    author: AgentPubKey,
    must_be_public: bool,
) -> Facts<'static, DhtOp> {
    facts![
        brute(
            "Header type matches Entry existence, and is public if exists",
            move |op: &DhtOp| {
                let header = op.header();
                let h = header.entry_data();
                let e = op.entry();
                dbg!(&h, &e);
                match (h, e) {
                    (Some((_entry_hash, entry_type)), Some(_e)) => {
                        // Ensure that entries are public
                        !must_be_public || entry_type.visibility().is_public()
                    }
                    (None, None) => true,
                    _ => false,
                }
            }
        ),
        mapped(
            "If there is entry data, the header must point to it",
            |op: &DhtOp| {
                if let Some(entry) = op.entry() {
                    // NOTE: this could be a `lens` if the previous check were short-circuiting,
                    // but it is possible that this check will run even if the previous check fails,
                    // so use a prism instead.
                    facts![prism(
                        "header's entry hash",
                        |op: &mut DhtOp| op.header_entry_data_mut().map(|(hash, _)| hash),
                        eq("hash of matching entry", EntryHash::with_data_sync(entry)),
                    )]
                } else {
                    facts![always()]
                }
            }
        ),
        lens(
            "The author is the one specified",
            DhtOp::author_mut,
            eq_(author)
        ),
        mapped("The Signature matches the Header", move |op: &DhtOp| {
            use holochain_keystore::AgentPubKeyExt;
            let header = op.header();
            let agent = header.author();
            let actual = tokio_helper::block_forever_on(agent.sign(&keystore, &header))
                .expect("Can sign the header");
            facts![lens("signature", DhtOp::signature_mut, eq_(actual))]
        })
    ]
}

#[cfg(test)]
mod tests {
    use arbitrary::{Arbitrary, Unstructured};
    use holochain_keystore::test_keystore::spawn_test_keystore;

    use super::*;
    use holochain_zome_types::header::facts as header_facts;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_valid_dht_op() {
        // TODO: Must add constraint on dht op variant wrt header variant

        let mut uu = Unstructured::new(&NOISE);
        let u = &mut uu;
        let keystore = spawn_test_keystore().await.unwrap();
        let agent = AgentPubKey::new_random(&keystore).await.unwrap();

        let e = Entry::arbitrary(u).unwrap();

        let mut hn = not_(header_facts::is_new_entry_header()).build(u);
        *hn.author_mut() = agent.clone();

        let mut he = header_facts::is_new_entry_header().build(u);
        *he.entry_data_mut().unwrap().0 = EntryHash::with_data_sync(&e);
        let mut he = Header::from(he);
        *he.author_mut() = agent.clone();

        let se = agent.sign(&keystore, &he).await.unwrap();
        let sn = agent.sign(&keystore, &hn).await.unwrap();

        let op1 = DhtOp::StoreElement(se.clone(), he.clone(), Some(Box::new(e.clone())));
        let op2 = DhtOp::StoreElement(se.clone(), he.clone(), None);
        let op3 = DhtOp::StoreElement(sn.clone(), hn.clone(), Some(Box::new(e.clone())));
        let op4 = DhtOp::StoreElement(sn.clone(), hn.clone(), None);
        let fact = valid_dht_op(keystore, agent, false);

        fact.check(&op1).unwrap();
        assert!(fact.check(&op2).is_err());
        assert!(fact.check(&op3).is_err());
        fact.check(&op4).unwrap();
    }
}

impl DhtOp {
    /// Mutable access to the Author
    pub fn author_mut(&mut self) -> &mut AgentPubKey {
        match self {
            DhtOp::StoreElement(_, h, _) => h.author_mut(),
            DhtOp::StoreEntry(_, h, _) => h.author_mut(),
            DhtOp::RegisterAgentActivity(_, h) => h.author_mut(),
            DhtOp::RegisterUpdatedContent(_, h, _) => &mut h.author,
            DhtOp::RegisterUpdatedElement(_, h, _) => &mut h.author,
            DhtOp::RegisterDeletedBy(_, h) => &mut h.author,
            DhtOp::RegisterDeletedEntryHeader(_, h) => &mut h.author,
            DhtOp::RegisterAddLink(_, h) => &mut h.author,
            DhtOp::RegisterRemoveLink(_, h) => &mut h.author,
        }
    }

    /// Access to the Timestamp
    pub fn timestamp(&self) -> Timestamp {
        match self {
            DhtOp::StoreElement(_, h, _) => h.timestamp(),
            DhtOp::StoreEntry(_, h, _) => h.timestamp(),
            DhtOp::RegisterAgentActivity(_, h) => h.timestamp(),
            DhtOp::RegisterUpdatedContent(_, h, _) => h.timestamp,
            DhtOp::RegisterUpdatedElement(_, h, _) => h.timestamp,
            DhtOp::RegisterDeletedBy(_, h) => h.timestamp,
            DhtOp::RegisterDeletedEntryHeader(_, h) => h.timestamp,
            DhtOp::RegisterAddLink(_, h) => h.timestamp,
            DhtOp::RegisterRemoveLink(_, h) => h.timestamp,
        }
    }

    /// Mutable access to the Timestamp
    pub fn timestamp_mut(&mut self) -> &mut Timestamp {
        match self {
            DhtOp::StoreElement(_, h, _) => h.timestamp_mut(),
            DhtOp::StoreEntry(_, h, _) => h.timestamp_mut(),
            DhtOp::RegisterAgentActivity(_, h) => h.timestamp_mut(),
            DhtOp::RegisterUpdatedContent(_, h, _) => &mut h.timestamp,
            DhtOp::RegisterUpdatedElement(_, h, _) => &mut h.timestamp,
            DhtOp::RegisterDeletedBy(_, h) => &mut h.timestamp,
            DhtOp::RegisterDeletedEntryHeader(_, h) => &mut h.timestamp,
            DhtOp::RegisterAddLink(_, h) => &mut h.timestamp,
            DhtOp::RegisterRemoveLink(_, h) => &mut h.timestamp,
        }
    }

    /// Mutable access to the Signature
    pub fn signature_mut(&mut self) -> &mut Signature {
        match self {
            DhtOp::StoreElement(s, _, _) => s,
            DhtOp::StoreEntry(s, _, _) => s,
            DhtOp::RegisterAgentActivity(s, _) => s,
            DhtOp::RegisterUpdatedContent(s, _, _) => s,
            DhtOp::RegisterUpdatedElement(s, _, _) => s,
            DhtOp::RegisterDeletedBy(s, _) => s,
            DhtOp::RegisterDeletedEntryHeader(s, _) => s,
            DhtOp::RegisterAddLink(s, _) => s,
            DhtOp::RegisterRemoveLink(s, _) => s,
        }
    }

    /// Mutable access to the seq of the Header, if applicable
    pub fn header_seq_mut(&mut self) -> Option<&mut u32> {
        match self {
            DhtOp::StoreElement(_, ref mut h, _) => h.header_seq_mut(),
            DhtOp::StoreEntry(_, ref mut h, _) => Some(h.header_seq_mut()),
            DhtOp::RegisterAgentActivity(_, ref mut h) => h.header_seq_mut(),
            DhtOp::RegisterUpdatedContent(_, ref mut h, _) => Some(&mut h.header_seq),
            DhtOp::RegisterUpdatedElement(_, ref mut h, _) => Some(&mut h.header_seq),
            DhtOp::RegisterDeletedBy(_, ref mut h) => Some(&mut h.header_seq),
            DhtOp::RegisterDeletedEntryHeader(_, ref mut h) => Some(&mut h.header_seq),
            DhtOp::RegisterAddLink(_, ref mut h) => Some(&mut h.header_seq),
            DhtOp::RegisterRemoveLink(_, ref mut h) => Some(&mut h.header_seq),
        }
    }

    /// Mutable access to the entry data of the Header, if applicable
    pub fn header_entry_data_mut(&mut self) -> Option<(&mut EntryHash, &mut EntryType)> {
        match self {
            DhtOp::StoreElement(_, ref mut h, _) => h.entry_data_mut(),
            DhtOp::StoreEntry(_, ref mut h, _) => Some(h.entry_data_mut()),
            DhtOp::RegisterAgentActivity(_, ref mut h) => h.entry_data_mut(),
            DhtOp::RegisterUpdatedContent(_, ref mut h, _) => {
                Some((&mut h.entry_hash, &mut h.entry_type))
            }
            DhtOp::RegisterUpdatedElement(_, ref mut h, _) => {
                Some((&mut h.entry_hash, &mut h.entry_type))
            }
            _ => None,
        }
    }
}

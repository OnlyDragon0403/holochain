use crate::{
    prelude::{DatabaseError, DatabaseResult},
    sql::sql_p2p_metrics,
};
use holochain_zome_types::prelude::*;
use kitsune_p2p::event::{MetricKind, MetricQuery, MetricQueryAnswer};
use kitsune_p2p::*;
use rusqlite::*;
use std::{
    num::TryFromIntError,
    sync::Arc,
    time::{Duration, SystemTime},
};

fn time_to_micros(t: SystemTime) -> DatabaseResult<i64> {
    t.duration_since(std::time::UNIX_EPOCH.into())
        .map_err(|e| DatabaseError::Other(e.into()))?
        .as_micros()
        .try_into()
        .map_err(|e: TryFromIntError| DatabaseError::Other(e.into()))
}

fn time_from_micros(micros: i64) -> SystemTime {
    std::time::UNIX_EPOCH
        .checked_add(Duration::from_micros(micros as u64))
        .expect("Time must be after 1970 and roughly before 200000")
}

/// Record a p2p metric datum
pub fn put_metric_datum(
    txn: &mut Transaction,
    agent: Arc<KitsuneAgent>,
    metric: MetricKind,
    timestamp: std::time::SystemTime,
) -> DatabaseResult<()> {
    let agent_bytes: &[u8] = agent.as_ref();
    txn.execute(
        sql_p2p_metrics::INSERT,
        named_params! {
            ":agent": agent_bytes,
            ":kind": metric.to_string(),
            ":moment": time_to_micros(timestamp)?
        },
    )?;
    Ok(())
}

/// Query the p2p_metrics database in a variety of ways
pub fn query_metrics(
    txn: &mut Transaction,
    query: MetricQuery,
) -> DatabaseResult<MetricQueryAnswer> {
    Ok(match query {
        MetricQuery::LastSync { agent } => {
            let agent_bytes: &[u8] = agent.as_ref();
            let timestamp: i64 = txn.query_row(
                sql_p2p_metrics::QUERY_LAST_SYNC,
                named_params! {
                    ":agent": agent_bytes,
                    ":kind": MetricKind::QuickGossip.to_string(),
                },
                |row| row.get(0),
            )?;
            dbg!(&timestamp);
            MetricQueryAnswer::LastSync(time_from_micros(timestamp))
        }
        MetricQuery::Oldest {
            last_connect_error_threshold,
        } => {
            let agent_bytes: Vec<u8> = txn.query_row(
                sql_p2p_metrics::QUERY_OLDEST,
                named_params! {
                    ":error_threshold": time_to_micros(last_connect_error_threshold)?,
                    ":kind_error": MetricKind::ConnectError.to_string(),
                    ":kind_slow_gossip": MetricKind::SlowGossip.to_string(),
                },
                |row| row.get(0),
            )?;
            MetricQueryAnswer::Oldest(KitsuneAgent::new(agent_bytes))
        }
    })
}

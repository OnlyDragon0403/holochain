-- Use this query only in the wrapping arc case,
-- i.e. when :storage_start_loc > :storage_end_loc
--
-- This is one version of this query. There is another version which may be faster.
SELECT
  hash,
  authored_timestamp
FROM
  DHtOp
WHERE
  DhtOp.when_integrated IS NOT NULL
  AND DhtOp.authored_timestamp >= :from
  AND DhtOp.authored_timestamp < :to
  AND (
    storage_center_loc < :storage_end_loc
    OR storage_center_loc > :storage_start_loc
  )

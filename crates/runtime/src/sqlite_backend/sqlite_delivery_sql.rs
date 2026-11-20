pub(crate) const CLAIM: &str = "WITH candidate AS (SELECT d.operation_id FROM operation_deliveries d \
     JOIN operations o ON o.id = d.operation_id WHERE o.delivery_target->>'adapter' = ?3 \
     AND NOT d.settled AND (d.lease_expires_at IS NULL OR d.lease_expires_at <= CAST(unixepoch('subsec') * 1000 AS INTEGER)) \
     AND NOT EXISTS (SELECT 1 FROM operation_deliveries predecessor_delivery \
     JOIN operations predecessor ON predecessor.id = predecessor_delivery.operation_id \
     WHERE NOT predecessor_delivery.settled AND predecessor.lane_id = o.lane_id \
     AND (predecessor.state IN ('accepted', 'running') OR predecessor_delivery.next_event_sequence \
     < predecessor.next_event_sequence) \
     AND predecessor.id < o.id AND predecessor.delivery_target->>'adapter' = \
     o.delivery_target->>'adapter') \
     AND (d.next_event_sequence < o.next_event_sequence OR (o.state IN ('accepted', 'running') \
     AND ?4 IS NOT NULL AND (d.presented_at IS NULL \
     OR d.presented_at <= CAST(unixepoch('subsec') * 1000 AS INTEGER) - ?4))) \
     ORDER BY d.operation_id LIMIT 1) \
     UPDATE operation_deliveries AS d SET lease_owner = ?1, lease_token = d.lease_token + 1, \
     lease_expires_at = CAST(unixepoch('subsec') * 1000 AS INTEGER) + ?2 FROM candidate c \
     WHERE d.operation_id = c.operation_id RETURNING operation_id, next_event_sequence, \
     lease_token, external_id, (SELECT delivery_target FROM operations WHERE id = operation_deliveries.operation_id), \
     (SELECT consecutive_failures FROM scheduled_work WHERE released_operation_id = operation_deliveries.operation_id), \
     (SELECT created_at FROM operations WHERE id = operation_deliveries.operation_id)";

pub(crate) const CHECKPOINT: &str = "UPDATE operation_deliveries SET \
     next_event_sequence = COALESCE(?2 + 1, next_event_sequence), \
     external_id = CASE WHEN ?2 IS NULL THEN COALESCE(?3, external_id) ELSE ?3 END, \
     presented_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), lease_owner = NULL, lease_expires_at = NULL, \
     settled = settled OR ?4 \
     WHERE operation_id = ?1";

-- A recipient may delete the copy an accepted offer made. The offer stays accepted, as a
-- record for both profiles, and stops naming the copy once the copy is gone. It can never
-- be pointed at another fact, and an offer is still only accepted by making a copy.
DROP TRIGGER shares_copy_required;
CREATE TRIGGER shares_copy_required BEFORE UPDATE OF accepted_fact_id, status ON fact_shares
WHEN (NEW.status='accepted' AND NEW.accepted_fact_id IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM facts WHERE id=NEW.accepted_fact_id AND profile_id=NEW.recipient_id))
 OR (NEW.status='accepted' AND NEW.accepted_fact_id IS NULL
      AND (OLD.status<>'accepted' OR EXISTS (SELECT 1 FROM facts WHERE id=OLD.accepted_fact_id)))
 OR (NEW.status<>'accepted' AND NEW.accepted_fact_id IS NOT NULL)
 OR (OLD.status<>'pending' AND NEW.status<>OLD.status)
BEGIN SELECT RAISE(ABORT, 'invalid share resolution'); END;

DROP TRIGGER accepted_copy_immutable;
CREATE TRIGGER accepted_copy_immutable BEFORE UPDATE OF accepted_fact_id ON fact_shares
WHEN OLD.status<>'pending' AND NEW.accepted_fact_id IS NOT OLD.accepted_fact_id
 AND (NEW.accepted_fact_id IS NOT NULL OR EXISTS (SELECT 1 FROM facts WHERE id=OLD.accepted_fact_id))
BEGIN SELECT RAISE(ABORT, 'accepted copy is immutable'); END;

-- SQLite gives a new row the highest ID plus one, so deleting the newest fact would hand
-- its ID to the next. Deleted IDs are kept, and nothing else of the fact, so an ID someone
-- kept, a retried delete or an offer's source never names a different fact.
CREATE TABLE deleted_facts (id INTEGER PRIMARY KEY) STRICT;

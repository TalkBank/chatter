-- Private single-document receipt. No JSON blobs or implicit public output.
CREATE TABLE document (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    map_key TEXT NOT NULL,
    output_path TEXT,
    source_blake3 TEXT NOT NULL,
    output_blake3 TEXT,
    reason TEXT,
    status TEXT NOT NULL CHECK (status IN ('prepared', 'written', 'publication_failed', 'unmapped', 'input_refused', 'output_refused')),
    CHECK ((status IN ('prepared', 'written', 'publication_failed') AND output_path IS NOT NULL AND output_blake3 IS NOT NULL AND reason IS NULL)
        OR (status IN ('unmapped', 'input_refused', 'output_refused') AND output_path IS NULL AND output_blake3 IS NULL AND reason IS NOT NULL))
) STRICT;

CREATE TABLE event (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES document(id),
    decision TEXT NOT NULL CHECK (decision IN ('replace', 'proposed', 'case_near_miss', 'possessive_review', 'unreplaced_lemma')),
    scope TEXT NOT NULL CHECK (scope IN ('main', 'mor', 'wor', 'participant', 'group', 'education', 'custom', 'header_prose', 'tier_prose', 'lemma_main', 'lemma_post_clitic')),
    utterance INTEGER CHECK (utterance >= 0),
    word INTEGER CHECK (word >= 0),
    target INTEGER CHECK (target >= 0),
    header INTEGER CHECK (header >= 0),
    tier INTEGER CHECK (tier >= 0),
    item INTEGER CHECK (item >= 0),
    clitic INTEGER CHECK (clitic >= 0),
    source_start INTEGER CHECK (source_start >= 0),
    source_end INTEGER CHECK (source_end >= source_start),
    output_start INTEGER CHECK (output_start >= 0),
    output_end INTEGER CHECK (output_end >= output_start),
    original TEXT NOT NULL,
    replacement TEXT,
    CHECK ((source_start IS NULL) = (source_end IS NULL)),
    CHECK ((output_start IS NULL) = (output_end IS NULL)),
    CHECK ((decision = 'replace' AND replacement IS NOT NULL AND source_start IS NOT NULL AND output_start IS NOT NULL)
        OR (decision = 'proposed' AND replacement IS NOT NULL AND output_start IS NULL AND output_end IS NULL)
        OR (decision NOT IN ('replace', 'proposed') AND replacement IS NULL AND output_start IS NULL AND output_end IS NULL)),
    CHECK ((scope IN ('main', 'mor', 'wor') AND utterance IS NOT NULL AND word IS NOT NULL AND header IS NULL AND tier IS NULL AND item IS NULL AND clitic IS NULL)
        OR (scope IN ('participant', 'group', 'education', 'custom', 'header_prose') AND header IS NOT NULL AND utterance IS NULL AND word IS NULL AND target IS NULL AND tier IS NULL AND item IS NULL AND clitic IS NULL)
        OR (scope = 'tier_prose' AND utterance IS NOT NULL AND tier IS NOT NULL AND word IS NULL AND target IS NULL AND header IS NULL AND item IS NULL AND clitic IS NULL)
        OR (scope = 'lemma_main' AND utterance IS NOT NULL AND item IS NOT NULL AND clitic IS NULL AND word IS NULL AND target IS NULL AND header IS NULL AND tier IS NULL)
        OR (scope = 'lemma_post_clitic' AND utterance IS NOT NULL AND item IS NOT NULL AND clitic IS NOT NULL AND word IS NULL AND target IS NULL AND header IS NULL AND tier IS NULL))
) STRICT;
CREATE INDEX event_decision ON event(decision);
CREATE INDEX event_location ON event(utterance, word, header, tier);

CREATE TABLE refusal (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES document(id),
    domain TEXT NOT NULL CHECK (domain IN ('word', 'mor', 'wor', 'pronunciation', 'metadata', 'prose')),
    reason TEXT NOT NULL,
    utterance INTEGER CHECK (utterance >= 0),
    word INTEGER CHECK (word >= 0),
    target INTEGER CHECK (target >= 0),
    source_start INTEGER CHECK (source_start >= 0),
    source_end INTEGER CHECK (source_end >= source_start),
    tier TEXT,
    CHECK ((source_start IS NULL) = (source_end IS NULL)),
    CHECK ((utterance IS NULL) = (word IS NULL))
) STRICT;

-- Counts are derived, not independently maintained status flags. A prepared
-- zero-change document remains distinguishable from one with no map entry.
CREATE VIEW document_summary AS
SELECT document.*,
    (SELECT count(*) FROM event WHERE decision = 'replace') AS applied_field_count,
    (SELECT count(*) FROM event WHERE decision = 'proposed') AS proposed_field_count,
    (SELECT count(*) FROM event WHERE decision NOT IN ('replace', 'proposed')) AS review_count,
    (SELECT count(*) FROM refusal) AS refusal_count,
    (NOT EXISTS (SELECT 1 FROM event) AND NOT EXISTS (SELECT 1 FROM refusal)) AS no_findings
FROM document;

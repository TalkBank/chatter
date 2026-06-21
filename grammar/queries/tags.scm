; Lean tags query for v0.1.
; Prioritize practical transcript navigation over exhaustive symbol indexing.
; TODO(post-v1): expand symbol surfaces as needed.

; Speaker turns
(main_tier
  speaker: (speaker) @name) @definition.function

; Participant declarations
(participant
  code: (speaker) @name) @definition.variable

; High-level file anchors
(participants_header) @definition.module
(begin_header) @reference.class
(end_header) @reference.class

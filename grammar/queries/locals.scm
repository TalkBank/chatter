; Lean locals query for v0.1.
; Keep speaker navigation useful without broad scope inference.
; TODO(post-v1): expand cross-tier and advanced local scopes.

(source_file) @local.scope

; Speaker definitions in @Participants
(participant
  code: (speaker) @local.definition)

; Main-tier speaker references
(main_tier
  speaker: (speaker) @local.reference)

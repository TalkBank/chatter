; Lean highlight queries for v0.1.
; Covers major CHAT structure without exhaustive per-node capture mapping.
; TODO(post-v1): expand coverage and refine token categories.

; Core headers
(utf8_header) @keyword
(begin_header) @keyword
(end_header) @keyword
(header) @keyword.directive

; Main-tier basics
(speaker) @variable.builtin
(word_with_optional_annotations) @string
(nonword_with_optional_annotations) @tag
(pause_token) @comment.block
(event) @tag
(linker) @keyword.control

; High-value annotations
(replacement) @string.special
(error_marker_annotation) @error

; Dependent-tier classes
(mor_dependent_tier) @type
(gra_dependent_tier) @type
(pho_dependent_tier) @type
(com_dependent_tier) @comment
(err_dependent_tier) @error
(tim_dependent_tier) @number
(unsupported_dependent_tier) @comment

; Mor/Gra internals
(mor_pos) @type.builtin
(mor_lemma) @string
(gra_relation) @function.method

; Structural punctuation
(terminator) @punctuation.special
(tab) @punctuation.delimiter
(separator) @punctuation.delimiter

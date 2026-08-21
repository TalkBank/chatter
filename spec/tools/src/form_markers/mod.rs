//! The CHAT special-form-marker registry, and the four artifacts derived from
//! it.
//!
//! # What this replaced
//!
//! The set of markers (`@b`, `@k`, `@z:label`, ...) was spelled out by hand in
//! sixteen places across this repository: five in the model's `form.rs` alone
//! (the enum, its parse, its inverse, and two lists in its documentation), five
//! in the re2c lexer (one of them a dead definition nothing referenced, and
//! four live rules), a comment in `grammar.js`, a literal in the parser's E203
//! diagnostic, two tables in the book, a list in the E202 spec, and a doc
//! comment on a regression test.
//!
//! They drifted, as copies do, and always silently. The `grammar.js` comment
//! and the enum disagreed for two years. Two copies had drifted again within an
//! hour of a session that corrected the others: the book's short list glossed
//! `@si` as "signed word" (that is `@sl`; `@si` is singing), and a test's doc
//! comment still advertised `@a` one commit after `@a` was retired.
//!
//! # The shape of the cure
//!
//! One owner, [`registry`], and renderers, [`render`], that produce each site
//! from it. The gate below asserts that every committed artifact equals what
//! the renderer produces, and it calls the RENDERER, not a second description
//! of the output, so there is nothing for the gate itself to drift from.

pub mod registry;
pub mod render;

#[cfg(test)]
mod tests {
    use super::registry::FormMarkerRegistry;
    use super::registry::RegistryError;
    use super::render;
    use crate::repo_paths::{self, RepoRoot};

    /// The checkout under test.
    fn root() -> RepoRoot {
        RepoRoot::resolve(None).expect(repo_paths::NOT_A_CHECKOUT)
    }

    fn load() -> FormMarkerRegistry {
        FormMarkerRegistry::load(root().as_path()).expect("the committed registry must be valid")
    }

    /// THE GATE. Every site that carries the marker inventory must equal what
    /// the registry says it should be.
    ///
    /// Iterates `render::OUTPUTS`, the same list the generator binary writes,
    /// so the gate cannot check a different set of files from the set that is
    /// produced. Pairing path with renderer by hand in both places was the
    /// first version, and it type-checked with the wrong renderer.
    ///
    /// Proven to fail: deleting a row from the registry, or a line from any of
    /// the committed files, fails this test naming that file.
    ///
    /// NOTE this test lives in the `spec/` workspace, so `just test` does NOT
    /// run it. It runs under `just test-spec`, `just test-all` and CI.
    #[test]
    fn generated_form_marker_sites_are_current() {
        let registry = load();
        for output in render::OUTPUTS {
            let rendered = (output.render)(&registry)
                .unwrap_or_else(|error| panic!("rendering {}: {error}", output.what));
            let full = root().join(output.path);
            let committed = std::fs::read_to_string(&full)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", full.display()));
            assert_eq!(
                committed, rendered,
                "{} is stale ({}). Regenerate with `just form-markers-gen`, and if that \
                 changes the model's shape or doc comments, regenerate the JSON Schema in the \
                 same commit.",
                output.path, output.what
            );
        }
    }

    /// The committed registry parses, and holds the markers the corpus
    /// authority sanctions in `depfile.cut`.
    ///
    /// This is a POLICY test, not an invariant one: it says which markers CHAT
    /// has, which no type can decide. It is the one assertion here that a
    /// reader of a future diff should stop and think about.
    #[test]
    fn committed_registry_matches_the_sanctioned_set() {
        let registry = load();
        let markers: Vec<&str> = registry
            .markers()
            .iter()
            .map(|row| row.marker.as_str())
            .collect();
        assert_eq!(
            markers,
            [
                "b", "c", "d", "f", "fp", "g", "i", "k", "l", "ls", "n", "o", "p", "q", "sas",
                "si", "sl", "t", "u", "wp", "x", "z",
            ],
            "the marker set must match the main-line `*@...` entries in \
             clan-info/lib/depfile.cut. `@a`, `@e` and `@lp` were retired by the corpus \
             authority in 2024 and must not come back; `@s` is the language suffix, which is a \
             separate construct."
        );
    }

    /// Two rows cannot claim the same marker. This one survives typing: no
    /// type can see its neighbours.
    #[test]
    fn duplicate_markers_are_refused() {
        let json = r#"{
            "version": 1,
            "description": "seeded",
            "authorities": {},
            "markers": [
                {"marker": "b", "variant": "B", "gloss": "Babbling",
                 "manual_anchor": "Babbling_Marker", "example_stem": "abame",
                 "example_note": null, "label": "forbidden"},
                {"marker": "b", "variant": "Bee", "gloss": "Something else",
                 "manual_anchor": "Other_Marker", "example_stem": "buzz",
                 "example_note": null, "label": "forbidden"}
            ]
        }"#;
        let error = FormMarkerRegistry::from_json(json).expect_err("a duplicate must be refused");
        assert!(
            matches!(
                error,
                RegistryError::Duplicate {
                    field: "marker",
                    ..
                }
            ),
            "expected a duplicate-marker error, got: {error}"
        );
    }

    /// The newtypes' `TryFrom` must actually run during deserialization.
    ///
    /// Kept because `#[serde(try_from = "String")]` is an attribute that can be
    /// dropped without any compile error: the field would still be a
    /// `MarkerCode`, and every invariant it advertises would silently stop
    /// being checked. This tests the WIRING, not the predicate.
    #[test]
    fn marker_code_invariant_is_enforced_through_serde() {
        let json = r#"{
            "version": 1,
            "description": "seeded",
            "authorities": {},
            "markers": [
                {"marker": "@B", "variant": "B", "gloss": "Babbling",
                 "manual_anchor": "Babbling_Marker", "example_stem": "abame",
                 "example_note": null, "label": "forbidden"}
            ]
        }"#;
        let error =
            FormMarkerRegistry::from_json(json).expect_err("`@B` is not a valid marker code");
        assert!(
            error.to_string().contains("@B"),
            "the error should quote the offending code, got: {error}"
        );
    }

    /// The OTHER registry in `spec/` is gated too.
    ///
    /// `spec/symbols/symbol_registry.json` generates three files (a JS module
    /// the grammar imports, and two Rust symbol-set modules) and had ZERO drift
    /// protection: no test compared any of them against the registry, and
    /// neither `just symbols-gen` nor its validator ran in CI, so a hand-edit
    /// to a generated symbol set was undetectable. By the form-marker
    /// registry's own argument that is the older and larger hole, so it is
    /// closed here rather than left as an observation.
    ///
    /// This runs the REAL generators in `--check` mode rather than
    /// re-describing their output, for the same reason the form-marker gate
    /// calls its renderers: a second description is a second thing to drift.
    /// They are still JS, so this shells out; porting them to Rust is a
    /// separate decision, and the gate should not wait on it.
    ///
    /// Adding this immediately found live drift: the Rust outputs were
    /// rustfmt-wrapped in the tree and unwrapped by the generator, so `just
    /// fmt` and `just symbols-gen` each undid the other. The generator now
    /// formats its Rust output, exactly as `render_rust` does here.
    ///
    /// The script list is DISCOVERED, not written down. It used to name two
    /// generators, so the two added on 2026-08-20 (the CA types and the book
    /// tables) would have been ungated by omission, which is how a gate quietly
    /// stops covering the thing it is named for. Any `spec/symbols/generate_*.js`
    /// is now in scope the moment it exists.
    #[test]
    fn generated_symbol_sets_are_current() {
        let symbols_dir = root().join("spec").join("symbols");
        let mut scripts: Vec<String> = std::fs::read_dir(&symbols_dir)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", symbols_dir.display()))
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name.starts_with("generate_") && name.ends_with(".js"))
            .collect();
        scripts.sort();

        // What this gate verified, counted from what the generators actually
        // reported rather than from a number written here. An earlier cut
        // asserted `scripts.len() >= 2` beside a directory holding four, which
        // is a hand-written census of the list it had just enumerated: deleting
        // two generators left it green. Counting `current:` lines instead means
        // a broken glob (no scripts), a script that emits nothing, and a script
        // that fails to run are all the same failure, which is what they are.
        let mut artifacts_verified = 0usize;

        for script in &scripts {
            let path = symbols_dir.join(script);
            let output = std::process::Command::new("node")
                .arg(&path)
                .arg("--check")
                .output()
                .unwrap_or_else(|error| panic!("cannot run node for {}: {error}", path.display()));
            assert!(
                output.status.success(),
                "{script} reports the symbol-set outputs stale. Regenerate with \
                 `just symbols-gen`.\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            artifacts_verified += String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| line.starts_with("current:"))
                .count();
        }

        assert!(
            artifacts_verified > 0,
            "ran {} generator(s) from {} and not one reported a verified artifact, \
             so this gate checked nothing.",
            scripts.len(),
            symbols_dir.display()
        );
    }

    /// An unknown field is a rename that missed a site, so it fails rather than
    /// being ignored.
    ///
    /// This is how the `example_meaning` to `example_note` rename was caught
    /// while this registry was being built: without it, a renamed field reads
    /// as an absent one and every site silently loses the text.
    #[test]
    fn unknown_fields_are_refused() {
        let json = r#"{
            "version": 1,
            "description": "seeded",
            "authorities": {},
            "markers": [
                {"marker": "b", "variant": "B", "gloss": "Babbling",
                 "manual_anchor": "Babbling_Marker", "example_stem": "abame",
                 "example_note": null, "label": "forbidden",
                 "example_meaning": "stale name"}
            ]
        }"#;
        FormMarkerRegistry::from_json(json).expect_err("an unknown field must be refused");
    }
}

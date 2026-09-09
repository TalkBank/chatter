//! The tree a gate reads, and the only way a probe changes one.
//!
//! # Why a gate may not touch `std::fs`
//!
//! A gate whose input is the live checkout can only ever be run against the
//! live checkout, so the one question nobody could answer about these gates
//! was whether they can fail at all. Answering it means planting a violation,
//! and planting it in the working tree means a crashed run leaves the
//! repository dirty and the plant visible to every other suite in the process.
//!
//! [`Tree`] is the seam: it READS THROUGH to the real checkout and holds an
//! in-memory map of overrides on top. A probe's plant edits the map, never the
//! disk. Almost all of every probe's input therefore stays the live tree,
//! which is the property a hand-built fixture directory loses: a fixture
//! drifts away from the gate's real input the day after it is written, and
//! then proves something about the fixture.
//!
//! Being the only read path is what lets the seam do a second job: it counts
//! what a gate actually read and issues an [`Examined`] witness, which
//! [`Tree::clean`], the only route to a clean verdict, REQUIRES. So a gate
//! that examined nothing cannot construct one, and a gate that went back to
//! `std::fs` cannot obtain the witness at all. The paragraph above used to be
//! the whole enforcement of both.
//!
//! # What an overlay cannot reach, stated rather than hidden
//!
//! Only bytes under the repository root. A gate whose answer depends on
//! something else, the linked tree-sitter grammar, a `const` in this crate,
//! an enum the derive macro generated at compile time, cannot be probed
//! through here, because no edit to any file changes those without a rebuild.
//! Those rules are named, one by one, in each gate's
//! [`unproven_rules`](crate::gate::Gate::unproven_rules).

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::gate::Outcome;

/// A path relative to the tree root, always forward-slashed.
///
/// One spelling, so a comparison against a path written in a policy list
/// cannot silently never match. Both absolute and relative paths used to be in
/// play in the catch-all sweep, converted at three call sites by a helper; a
/// comparison that forgot the call compiled and matched nothing, which in a
/// ratchet reads as "clean".
///
/// Forward slashes on every host, because the lists these are compared against
/// are written that way and Windows yields backslashes. That was a live
/// defect: the catch-all gate reported every listed file as newly gaining a
/// catch-all, on Windows only, and had done since it was written.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelPath(String);

impl RelPath {
    /// A path as written in a policy list or a probe.
    ///
    /// The separator rewrite is CONDITIONAL. `replace` allocates whether or
    /// not it finds anything, and no tracked path in this repository contains a
    /// backslash, so the unconditional form allocated twice per path for a case
    /// that never arises on the host that runs this: about 53,000 wasted
    /// allocations per `just test`.
    #[must_use]
    pub fn new(text: &str) -> Self {
        let text = text.trim_start_matches("./");
        match text.contains('\\') {
            true => Self(text.replace('\\', "/")),
            false => Self(text.to_owned()),
        }
    }

    /// Strip `root` from an absolute path found by a walk.
    ///
    /// # Errors
    ///
    /// When `file` is not under `root`. This used to be `unwrap_or(file)`,
    /// which kept the ABSOLUTE path and called it relative: the resulting
    /// `RelPath` then matched no entry in any policy list, and a ratchet whose
    /// comparison matches nothing reads as clean. That is the very defect this
    /// type's own doc comment above describes, reintroduced by its fallback.
    /// The walk cannot produce such a path, and saying so with a `Result` costs
    /// one `?` at the single production call site.
    pub fn under(root: &Path, file: &Path) -> Result<Self, TreeError> {
        match file.strip_prefix(root) {
            Ok(relative) => Ok(Self::new(&relative.to_string_lossy())),
            Err(_) => Err(TreeError::Walk {
                under: Self(root.display().to_string()),
                error: format!(
                    "the walk yielded {}, which is not under the tree root",
                    file.display()
                ),
            }),
        }
    }

    /// The path as text, forward-slashed.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this path sits inside `dir`. An empty `dir` is the whole tree.
    ///
    /// Allocation-free: the old form built `format!("{dir}/")` on every call,
    /// and this is reached once per walked path for every probe that hides or
    /// removes a directory.
    #[must_use]
    pub fn is_under(&self, dir: &str) -> bool {
        let dir = dir.trim_end_matches('/');
        dir.is_empty()
            || self.0 == dir
            || (self.0.starts_with(dir) && self.0.as_bytes().get(dir.len()) == Some(&b'/'))
    }

    /// Whether the extension matches, ignoring ASCII case.
    #[must_use]
    pub fn extension_is(&self, ext: &str) -> bool {
        match self.0.rsplit_once('.') {
            Some((_, found)) => found.eq_ignore_ascii_case(ext),
            None => false,
        }
    }

    /// The last path segment.
    ///
    /// Written as the two real cases rather than as `rsplit('/').next()` with
    /// an `unwrap_or`: `rsplit` always yields a first piece, so that fallback
    /// stood for a state that does not exist, in the spelling this project bans
    /// on sight. A path with no separator IS its own file name, which is a
    /// fact, not a default.
    #[must_use]
    pub fn file_name(&self) -> &str {
        match self.0.rsplit_once('/') {
            Some((_, name)) => name,
            None => &self.0,
        }
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a read or a walk through the tree produced nothing usable.
///
/// Named cases rather than one preformatted `String`: a walk failure means an
/// unknown number of files were never offered at all, where a read failure
/// names exactly what was lost, and the two call for different operator
/// actions.
#[derive(Debug)]
pub enum TreeError {
    /// No such file, in the overlay or on disk.
    Missing(RelPath),
    /// The file exists and could not be read.
    Unreadable {
        /// The file that could not be read.
        path: RelPath,
        /// The underlying failure, as reported.
        error: String,
    },
    /// The file exists and its bytes are not UTF-8.
    NotUtf8(RelPath),
    /// The directory walk itself failed, so the file set is a floor.
    Walk {
        /// The directory being enumerated.
        under: RelPath,
        /// The underlying failure, as reported.
        error: String,
    },
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(path) => write!(f, "{path}: not present in the tree"),
            Self::Unreadable { path, error } => write!(f, "{path}: {error}"),
            Self::NotUtf8(path) => write!(f, "{path}: stream did not contain valid UTF-8"),
            Self::Walk { under, error } => write!(f, "could not walk {under}: {error}"),
        }
    }
}

/// So a caller can carry one out of a `?` chain that mixes it with an IO
/// error. `Display` already says everything an operator needs; there is no
/// `source`, because the underlying failure is already rendered INTO the
/// message rather than held, which keeps the variants comparable and printable
/// without a chain.
impl std::error::Error for TreeError {}

/// What the overlay says about one path.
///
/// A sum type rather than an `Option<Vec<u8>>`, where `None` would have to mean
/// "deleted" while an absent key means "read through". Those are different
/// facts and a reader cannot tell two spellings of nothing apart.
#[derive(Clone)]
enum Override {
    /// These bytes, instead of whatever is on disk.
    Content(Vec<u8>),
    /// This file is not in the tree, whatever is on disk.
    Gone,
}

/// Proof that a gate actually examined the tree it is about to judge.
///
/// # Why a clean verdict carries one
///
/// "0 problems" and "checked nothing" print identically, which is the bug
/// class [`crate::gate`] exists to close, and all four of the spellings it
/// lists were found by a human rather than by a check. [`Tree::clean`]
/// therefore does not take a summary alone: it takes the evidence, and the
/// evidence is issued by the tree the gate read. What was "convention plus
/// review" is now a signature.
///
/// # Every way to obtain one
///
/// [`Tree::examined`], and nothing else. Both fields are private to this
/// module, so no code outside `gate::tree` can write the struct literal, and
/// there is no other constructor, no `Default`, and no arithmetic that builds
/// one from a count. A gate that reaches for `std::fs` instead of its tree
/// cannot obtain a witness at all, which is the second half of the same rule.
///
/// # What it does NOT claim
///
/// That anything was FOUND. Enumerating a directory that EXISTS and holds no
/// files is a real reading of the tree, so it yields a witness whose
/// `files_read` is 0, and the gate that legitimately measures an empty
/// directory can still report clean. A directory that is not there is a
/// different fact and mints nothing: see [`Tree::files_under`]. What has no
/// witness is the gate that made no read and no enumeration at all. The two
/// are then told apart in the TEXT as well, because every clean summary prints
/// what was examined: a sweep over 812 files and a sweep over none no longer
/// print the same sentence.
#[derive(Clone, Copy, Debug)]
pub struct Examined {
    /// Files whose contents were read through the tree.
    files_read: usize,
    /// Directories enumerated through the tree, empty ones included.
    dirs_enumerated: usize,
}

impl Examined {
    /// Two witnesses are one witness, counting both.
    ///
    /// Only a fold over evidence that already exists: it cannot mint a witness
    /// and has no way to raise a count except from another one. The probe
    /// harness uses it to carry what the gates it ran actually read, so its own
    /// clean summary rests on their evidence rather than on a number it wrote.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        Self {
            files_read: self.files_read.saturating_add(other.files_read),
            dirs_enumerated: self.dirs_enumerated.saturating_add(other.dirs_enumerated),
        }
    }
}

impl fmt::Display for Examined {
    /// Counts of OPERATIONS, worded so that a summed witness stays true: a run
    /// that read one file five times performed five reads, and "5 file read(s)"
    /// says exactly that where "examined 5 files" would imply five of them.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} file read(s), {} directory enumeration(s)",
            self.files_read, self.dirs_enumerated
        )
    }
}

/// The live checkout, plus an in-memory map of overrides.
///
/// Cheap to COPY, which is what a plant does: the map is empty for the live
/// tree and a plant adds a handful of entries. There is deliberately no
/// `Clone`; see [`Tree::overlay_copy`].
pub struct Tree {
    root: PathBuf,
    files: BTreeMap<RelPath, Override>,
    /// Directories whose files are hidden. The directory itself still exists.
    hidden: BTreeSet<RelPath>,
    /// Directories that are not in the tree at all.
    absent: BTreeSet<RelPath>,
    /// Directories whose ENUMERATION must fail.
    ///
    /// The overlay could express "this directory is empty" and "this directory
    /// is gone" and not "this directory cannot be read", so five gates each
    /// declared their walk-failure half unprobeable, in the same words, and one
    /// filesystem fault would have been needed to reach any of them. A walk
    /// failure is the worst case a walking gate has, because it means an
    /// unknown number of files were never offered, and it was the one case no
    /// probe could plant.
    faulted: BTreeSet<RelPath>,
    /// Files read through THIS tree value, and directories enumerated through
    /// it. `Cell`, because reading is `&self` everywhere and the count is the
    /// evidence a gate needs at the end of a read-only pass.
    files_read: Cell<usize>,
    dirs_enumerated: Cell<usize>,
    /// Unoverridden file contents, shared with every tree planted from this
    /// one.
    ///
    /// The probe harness re-runs a whole-tree gate per probe and a plant
    /// touches one or two files, so without this the corpus is read once per
    /// PROBE. Measured 2026-09-08: dozens of probes over about 1,100 files.
    ///
    /// It caches CONTENT, never evidence: [`Tree::read_to_string`] still counts
    /// a cache hit as the read it is, because a gate that consulted the tree
    /// consulted the tree. And only unoverridden paths reach it, so a planted
    /// file is answered from the overlay and can never be served a stale
    /// version of itself.
    cache: Rc<RefCell<BTreeMap<RelPath, Rc<str>>>>,
}

impl Tree {
    /// The checkout this test binary was built from.
    #[must_use]
    pub fn live() -> Self {
        Self::rooted(crate::repo_paths::workspace_root())
    }

    /// A tree rooted anywhere, with no overrides.
    #[must_use]
    pub fn rooted(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            files: BTreeMap::new(),
            hidden: BTreeSet::new(),
            absent: BTreeSet::new(),
            faulted: BTreeSet::new(),
            files_read: Cell::new(0),
            dirs_enumerated: Cell::new(0),
            cache: Rc::new(RefCell::new(BTreeMap::new())),
        }
    }

    /// Where the tree is rooted, for a message naming a real path.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The evidence that this tree was examined, when it was.
    ///
    /// `None` is the gate that read no file and enumerated no directory: not a
    /// small measurement, no measurement. The only producer of an [`Examined`];
    /// see that type for why there is deliberately no other.
    #[must_use]
    pub fn examined(&self) -> Option<Examined> {
        let (files_read, dirs_enumerated) = (self.files_read.get(), self.dirs_enumerated.get());
        if files_read == 0 && dirs_enumerated == 0 {
            return None;
        }
        Some(Examined {
            files_read,
            dirs_enumerated,
        })
    }

    /// Clean, carrying the evidence that THIS tree was examined.
    ///
    /// The route every gate takes to a clean verdict. It is a method on the
    /// tree rather than a free function taking a witness so that the evidence
    /// provably comes from the tree the gate was handed: a gate holding two
    /// trees cannot pair one's summary with the other's reads.
    #[must_use]
    pub fn clean(&self, summary: impl Into<String>) -> Outcome {
        Outcome::clean_if_examined(summary, self.examined())
    }

    /// Apply a probe's plant, yielding the tree the gate will judge.
    ///
    /// # Errors
    ///
    /// When the plant cannot be applied, which is a probe that has drifted
    /// away from the tree it was written against rather than a gate defect.
    /// The harness reports the two differently and must never read the first
    /// as the second.
    pub fn planted(&self, plant: Plant) -> Result<ReadTree, PlantFailed> {
        let mut edit = TreeEdit {
            tree: self.overlay_copy(),
        };
        plant(&mut edit)?;
        // No counter reset here. A plant reads its anchors through
        // `TreeEdit::read`, which is uncounted, so the tree it hands over has
        // no evidence on it by construction rather than by cleanup.
        Ok(ReadTree(edit.tree))
    }

    /// The overlay, copied; the evidence, never.
    ///
    /// Deliberately NOT a `Clone` impl, and the honest self-check is that this
    /// removed an AFFORDANCE rather than a reachable wrong value. The impl
    /// that stood here was correct: it zeroed both counters and said so. What
    /// a `Clone` in the signature invites is a later `#[derive(Clone)]`, whose
    /// copy WOULD carry the evidence, and a caller outside `planted` who then
    /// composed a clean verdict on another tree's reads. Private, with one
    /// caller, that edit is not available to make.
    fn overlay_copy(&self) -> Self {
        Self {
            root: self.root.clone(),
            files: self.files.clone(),
            hidden: self.hidden.clone(),
            absent: self.absent.clone(),
            faulted: self.faulted.clone(),
            files_read: Cell::new(0),
            dirs_enumerated: Cell::new(0),
            // Shared by HANDLE, unlike everything above it: the point of the
            // copy is that the corpus is read once per RUN rather than once
            // per probe. Content only, never evidence.
            cache: Rc::clone(&self.cache),
        }
    }

    /// Spend this tree for reading, with nothing planted.
    ///
    /// The unplanted half of [`Tree::planted`], for the caller that judges the
    /// live checkout. Takes `self` for the same reason [`crate::gate::Gate`]'s
    /// `check` does: one tree, one gate, one witness.
    #[must_use]
    pub fn into_read(self) -> ReadTree {
        ReadTree(self)
    }

    /// Whether a path is hidden by a `hide_files_under` or `remove_dir`.
    fn concealed(&self, path: &RelPath) -> bool {
        self.hidden
            .iter()
            .chain(self.absent.iter())
            .any(|dir| path.is_under(dir.as_str()))
    }

    /// Whether the directory is in the tree.
    #[must_use]
    pub fn dir_exists(&self, dir: &str) -> bool {
        let dir = RelPath::new(dir);
        if self.absent.iter().any(|gone| dir.is_under(gone.as_str())) {
            return false;
        }
        self.root.join(dir.as_str()).is_dir()
            || self.files.iter().any(|(path, over)| {
                matches!(over, Override::Content(_)) && path.is_under(dir.as_str())
            })
    }

    /// Whether the file is in the tree.
    #[must_use]
    pub fn file_exists(&self, path: &str) -> bool {
        let path = RelPath::new(path);
        match self.files.get(&path) {
            Some(Override::Content(_)) => return true,
            Some(Override::Gone) => return false,
            None => {}
        }
        !self.concealed(&path) && self.root.join(path.as_str()).is_file()
    }

    /// Read one file's text.
    ///
    /// # Errors
    ///
    /// When the file is absent, unreadable, or not UTF-8. Each is a distinct
    /// [`TreeError`], because a gate reports them differently.
    pub fn read_to_string(&self, path: &RelPath) -> Result<String, TreeError> {
        let text = self.read_uncounted(path)?;
        // A read that SUCCEEDED is what an `Examined` witnesses. A failed one
        // is not: the gate learned that it could not read, which it reports as
        // a failure rather than as a clean verdict about content it never saw.
        self.files_read.set(self.files_read.get().saturating_add(1));
        Ok(text)
    }

    /// The read itself, without recording the evidence.
    ///
    /// Split out so that [`Self::read_to_string`] counts in ONE place, at the
    /// single success point, rather than at each of the three returns below.
    fn read_uncounted(&self, path: &RelPath) -> Result<String, TreeError> {
        match self.files.get(path) {
            Some(Override::Gone) => return Err(TreeError::Missing(path.clone())),
            Some(Override::Content(bytes)) => {
                return String::from_utf8(bytes.clone())
                    .map_err(|_| TreeError::NotUtf8(path.clone()));
            }
            None => {}
        }
        if self.concealed(path) {
            return Err(TreeError::Missing(path.clone()));
        }
        // Only unoverridden files reach here, so the bytes are the disk's and
        // every tree sharing this cache would read the same ones. A plant is
        // an override and was answered above, which is what makes the sharing
        // sound: the one thing that differs between a source tree and its
        // planted copies never comes from here.
        if let Some(cached) = self.cache.borrow().get(path) {
            return Ok(cached.to_string());
        }
        let text =
            std::fs::read_to_string(self.root.join(path.as_str())).map_err(|err| {
                match err.kind() {
                    std::io::ErrorKind::NotFound => TreeError::Missing(path.clone()),
                    std::io::ErrorKind::InvalidData => TreeError::NotUtf8(path.clone()),
                    _ => TreeError::Unreadable {
                        path: path.clone(),
                        error: err.to_string(),
                    },
                }
            })?;
        // Failures are not cached: a missing file may be planted into
        // existence by the next probe, and a cached absence would answer for
        // it. Only a successful read of an unoverridden path is a fact about
        // the checkout that a later tree can reuse.
        self.cache
            .borrow_mut()
            .insert(path.clone(), Rc::from(text.as_str()));
        Ok(text)
    }

    /// Every file under `dir`, sorted, with the overlay applied.
    ///
    /// An empty `dir` means the whole tree. A missing directory yields the
    /// overlay's own additions and nothing else, and mints NO witness: see the
    /// note beside the counter below.
    ///
    /// # Errors
    ///
    /// When the walk fails. The predecessor of this walk dropped walk errors
    /// with `filter_map(Result::ok)`, so an unreadable subdirectory silently
    /// shrank the file set and the gate still reported clean over less than it
    /// claimed. A floor is not a measurement.
    pub fn files_under(&self, dir: &str) -> Result<Vec<RelPath>, TreeError> {
        let dir = RelPath::new(dir);
        // A PLANTED walk failure, before anything is collected: a gate must see
        // the same thing an unreadable directory would give it, which is an
        // error and not a smaller file set.
        if let Some(faulted) = self
            .faulted
            .iter()
            .find(|gone| dir.is_under(gone.as_str()) || gone.is_under(dir.as_str()))
        {
            return Err(TreeError::Walk {
                under: faulted.clone(),
                error: "the directory could not be enumerated".to_owned(),
            });
        }
        let mut found: BTreeSet<RelPath> = BTreeSet::new();

        let mut walked = false;
        if !self.absent.iter().any(|gone| dir.is_under(gone.as_str())) {
            let abs = if dir.as_str().is_empty() {
                self.root.clone()
            } else {
                self.root.join(dir.as_str())
            };
            if abs.is_dir() {
                walked = true;
                for entry in walkdir::WalkDir::new(&abs) {
                    let entry = entry.map_err(|err| TreeError::Walk {
                        under: dir.clone(),
                        error: err.to_string(),
                    })?;
                    if entry.file_type().is_file() {
                        found.insert(RelPath::under(&self.root, entry.path())?);
                    }
                }
            }
        }

        for (path, over) in &self.files {
            if !path.is_under(dir.as_str()) {
                continue;
            }
            match over {
                Override::Content(_) => {
                    found.insert(path.clone());
                }
                Override::Gone => {
                    found.remove(path);
                }
            }
        }

        // A completed enumeration is an examination even when it offers no
        // file: a gate measuring an EMPTY directory has looked, and must still
        // be able to report clean. A gate pointed at a directory that is not
        // there has not looked at anything, and until 2026-09-08 the two minted
        // the same witness: `is_dir()` was false, the walk never ran, and this
        // line ran anyway, so a gate whose whole scope had been removed
        // returned `Ok(vec![])` with evidence and reported clean over nothing.
        // That is the bug class this module exists to close, inside the type
        // built to close it.
        //
        // An overlay that supplied files under a directory the disk does not
        // have IS a real reading, so it counts too. The failure paths above
        // return before this line, so a walk that failed leaves no evidence.
        if walked || !found.is_empty() {
            self.dirs_enumerated
                .set(self.dirs_enumerated.get().saturating_add(1));
        }

        // An explicit write WINS over a hiding, whatever order the plant made
        // them in. Otherwise "hide this directory, then put one file back" is
        // not expressible, and that is exactly the tree a gate must tell apart
        // from an empty one.
        Ok(found
            .into_iter()
            .filter(|path| {
                matches!(self.files.get(path), Some(Override::Content(_))) || !self.concealed(path)
            })
            .collect())
    }
}

/// So a plant failure can travel out of a `?` chain that also carries an IO
/// error, the way [`TreeError`] does. `Display` says everything a probe author
/// needs and the underlying failure is already rendered into the message, so
/// there is no `source`.
impl std::error::Error for PlantFailed {}

/// A plant that could not be applied to the tree it was written against.
///
/// Its own type, not a gate failure. A probe whose anchor text has moved says
/// nothing about whether the gate can fail, and reading the first as the
/// second would accuse a sound gate of being inert.
#[derive(Debug)]
pub struct PlantFailed(String);

impl PlantFailed {
    /// State why the plant could not be applied.
    #[must_use]
    pub fn new(why: impl Into<String>) -> Self {
        Self(why.into())
    }
}

impl fmt::Display for PlantFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A tree a gate reads and judges, and cannot change.
///
/// # Why this is a second type rather than a rule
///
/// A probe plants, and THEN the gate judges. Written as one type that does
/// both, that order is a convention: [`Tree::planted`] is `pub`, so a gate
/// handed a plantable tree could plant over the violation it was given and
/// judge the result, and every probe against it would go green. "X happens
/// strictly after Y" is the tell this project hunts for, and the cure it names
/// is a phase type, so [`Tree::planted`] and [`Tree::into_read`] are the only
/// routes here and there is no route back.
///
/// It is a thin wrapper on purpose. What it WITHHOLDS is the whole point: no
/// `planted`, no [`TreeEdit`], and no way to reach the tree inside.
pub struct ReadTree(Tree);

impl ReadTree {
    /// Where the tree is rooted, for a message naming a real path.
    #[must_use]
    pub fn root(&self) -> &Path {
        self.0.root()
    }

    /// Clean, carrying the evidence that THIS tree was examined.
    #[must_use]
    pub fn clean(&self, summary: impl Into<String>) -> Outcome {
        self.0.clean(summary)
    }

    /// Whether the directory is in the tree.
    #[must_use]
    pub fn dir_exists(&self, dir: &str) -> bool {
        self.0.dir_exists(dir)
    }

    /// Whether the file is in the tree.
    #[must_use]
    pub fn file_exists(&self, path: &str) -> bool {
        self.0.file_exists(path)
    }

    /// Read one file's text, counting the read as evidence.
    ///
    /// # Errors
    ///
    /// When the file is absent from the tree, or its bytes are not UTF-8.
    pub fn read_to_string(&self, path: &RelPath) -> Result<String, TreeError> {
        self.0.read_to_string(path)
    }

    /// Every file under a directory, sorted, counting the enumeration.
    ///
    /// # Errors
    ///
    /// When the walk itself fails.
    pub fn files_under(&self, dir: &str) -> Result<Vec<RelPath>, TreeError> {
        self.0.files_under(dir)
    }
}

/// How a probe changes the tree. A function pointer, so a probe list is data.
pub type Plant = fn(&mut TreeEdit) -> Result<(), PlantFailed>;

/// The write half of a [`Tree`]: the only way a plant changes one.
///
/// Every write lands in the overlay. Nothing here touches the disk, so a
/// planted tree cannot outlive the probe that built it and a crashed run
/// cannot leave the checkout dirty.
pub struct TreeEdit {
    tree: Tree,
}

impl TreeEdit {
    /// Read a file as the gate would see it, to build an edit from it.
    ///
    /// # Errors
    ///
    /// When the file is not there: a plant that cannot read its own anchor has
    /// drifted.
    pub fn read(&self, path: &str) -> Result<String, PlantFailed> {
        // UNCOUNTED, so a plant's own reads are never the gate's evidence in
        // the first place. `planted` used to zero the counters afterwards,
        // which worked and left the property as a cleanup step somebody had to
        // remember when adding a second reader here.
        self.tree
            .read_uncounted(&RelPath::new(path))
            .map_err(|err| PlantFailed::new(format!("cannot read the plant's anchor: {err}")))
    }

    /// Write text at `path`, creating it if the tree has no such file.
    pub fn write(&mut self, path: &str, text: impl Into<String>) {
        self.write_bytes(path, text.into().into_bytes());
    }

    /// Write raw bytes at `path`, which need not be UTF-8.
    ///
    /// The one plant a text-only overlay could not express is "this file
    /// exists and fails to read", and it is the plant that proves a gate
    /// refuses to report a floor as a measurement.
    pub fn write_bytes(&mut self, path: &str, bytes: Vec<u8>) {
        self.tree
            .files
            .insert(RelPath::new(path), Override::Content(bytes));
    }

    /// Replace the one occurrence of `needle` in `path`.
    ///
    /// # Errors
    ///
    /// When the file does not contain `needle` EXACTLY once. That refusal is
    /// the whole point: a plant that quietly changed nothing would make its
    /// gate look inert, which is the accusation this mechanism exists to make
    /// carefully.
    pub fn replace_once(
        &mut self,
        path: &str,
        needle: &str,
        replacement: &str,
    ) -> Result<(), PlantFailed> {
        let text = self.read(path)?;
        let occurrences = text.matches(needle).count();
        if occurrences != 1 {
            return Err(PlantFailed::new(format!(
                "{path} contains {occurrences} occurrence(s) of the anchor text, \
                 expected exactly 1. The probe has drifted from the tree; \
                 re-anchor it rather than reading this as a gate that cannot fail.\n\
                 anchor: {needle:?}"
            )));
        }
        self.write(path, text.replacen(needle, replacement, 1));
        Ok(())
    }

    /// Append text to a file that must already exist.
    ///
    /// # Errors
    ///
    /// When the file is not there.
    pub fn append(&mut self, path: &str, tail: &str) -> Result<(), PlantFailed> {
        let text = self.read(path)?;
        self.write(path, format!("{text}{tail}"));
        Ok(())
    }

    /// Take one file out of the tree.
    pub fn remove_file(&mut self, path: &str) {
        self.tree.files.insert(RelPath::new(path), Override::Gone);
    }

    /// Hide every file under a directory, leaving the directory itself.
    ///
    /// The "present but empty" case, which is a broken measurement rather than
    /// a partial checkout, and which gates must tell apart from the directory
    /// being absent.
    pub fn hide_files_under(&mut self, dir: &str) {
        self.tree.hidden.insert(RelPath::new(dir));
    }

    /// Take a whole directory out of the tree.
    /// Make enumerating `dir`, or anything containing it, FAIL.
    ///
    /// Distinct from [`Self::hide_files_under`] (the directory is there and
    /// offers nothing) and from [`Self::remove_dir`] (the directory is not
    /// there). This is the third thing a walk can do and the only one a gate
    /// must never treat as a smaller file set: an unknown number of files were
    /// never offered, so any count taken after it is a floor.
    pub fn fail_walk_under(&mut self, dir: &str) {
        self.tree.faulted.insert(RelPath::new(dir));
    }

    /// Take a directory out of the tree entirely.
    ///
    /// Distinct from [`Self::hide_files_under`], where the directory is there
    /// and offers nothing, and from [`Self::fail_walk_under`], where it cannot
    /// be read at all. A gate that asks `dir_exists` first tells this from the
    /// other two; one that does not sees an empty set from all three.
    pub fn remove_dir(&mut self, dir: &str) {
        self.tree.absent.insert(RelPath::new(dir));
    }
}

#[cfg(test)]
mod tests {
    use super::{RelPath, Tree, TreeError};
    use std::path::Path;

    /// SURVIVES: behaviour a signature cannot state. `RelPath` renders a path
    /// for COMPARISON against policy lists written with forward slashes, so the
    /// rendering must be separator-independent. Testable on any host because
    /// the input is a path whose text contains backslashes, which is what
    /// Windows hands `to_string_lossy`.
    #[test]
    fn rel_paths_are_forward_slashed_whatever_the_host_separator() -> Result<(), TreeError> {
        let root = Path::new("/repo");
        let windows_style = Path::new("/repo/crates\\talkbank-lsp\\src\\alignment.rs");
        assert_eq!(
            RelPath::under(root, windows_style)?.as_str(),
            "crates/talkbank-lsp/src/alignment.rs",
            "a path rendered for comparison must not carry the host separator"
        );
        let unix_style = Path::new("/repo/crates/talkbank-lsp/src/alignment.rs");
        assert_eq!(
            RelPath::under(root, unix_style)?.as_str(),
            "crates/talkbank-lsp/src/alignment.rs"
        );
        Ok(())
    }

    /// SURVIVES: behaviour a signature cannot state. That a PLANTED file is
    /// never served from the shared read cache is the whole safety property of
    /// that cache, and it is a fact about two tree values at runtime.
    ///
    /// The cache exists because the probe harness re-runs a whole-tree gate per
    /// probe; it is shared by handle with every planted copy, so if an override
    /// could be answered from it a probe would judge the tree it meant to
    /// change.
    #[test]
    fn a_planted_file_is_never_answered_from_the_shared_cache()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("a.rs"), "on disk\n")?;
        let source = Tree::rooted(root.path());
        let path = RelPath::new("a.rs");

        // Populate the cache through the SOURCE tree.
        assert_eq!(source.read_to_string(&path)?, "on disk\n");

        let overwritten = source.planted(|edit| {
            edit.write("a.rs", "planted\n");
            Ok(())
        })?;
        assert_eq!(
            overwritten.read_to_string(&path)?,
            "planted\n",
            "an override must win over a cache the planted tree shares"
        );

        let removed = source.planted(|edit| {
            edit.remove_file("a.rs");
            Ok(())
        })?;
        assert!(
            matches!(removed.read_to_string(&path), Err(TreeError::Missing(_))),
            "a removed file must read as missing, not from the cache"
        );

        // And the source tree still sees the disk, so a plant cannot leak
        // sideways into the tree it was copied from.
        assert_eq!(source.read_to_string(&path)?, "on disk\n");
        Ok(())
    }

    /// SURVIVES: behaviour. A cache HIT is still a read through the tree, so it
    /// still counts as evidence. If it did not, a gate whose files were all
    /// cached would report clean having "examined nothing".
    #[test]
    fn a_cache_hit_is_still_evidence() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("a.rs"), "on disk\n")?;
        let source = Tree::rooted(root.path());
        let path = RelPath::new("a.rs");
        source.read_to_string(&path)?;

        let second = source.planted(|_| Ok(()))?;
        second.read_to_string(&path)?;
        assert!(
            second.0.examined().is_some(),
            "a tree that answered a read from the cache has still been read"
        );
        Ok(())
    }

    /// SURVIVES: behaviour reaching the filesystem. Whether an absent
    /// directory mints a witness is a fact about a real `is_dir` call.
    #[test]
    fn an_absent_directory_is_not_an_examination_but_an_empty_one_is()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        std::fs::create_dir(root.path().join("empty"))?;

        let tree = Tree::rooted(root.path());
        assert!(tree.files_under("nowhere")?.is_empty());
        assert!(
            tree.examined().is_none(),
            "a gate pointed at a directory that is not there has looked at \
             nothing, and must not be able to compose a clean verdict"
        );

        let tree = Tree::rooted(root.path());
        assert!(tree.files_under("empty")?.is_empty());
        assert!(
            tree.examined().is_some(),
            "a gate that enumerated a real, empty directory HAS looked, and \
             must still be able to report clean over it"
        );
        Ok(())
    }

    /// SURVIVES: behaviour. A path outside the root is REFUSED rather than
    /// silently kept absolute, because a `RelPath` holding an absolute path
    /// matches no entry in any policy list, and a ratchet whose comparison
    /// matches nothing reads as clean.
    #[test]
    fn a_path_outside_the_root_is_refused_rather_than_kept_absolute() {
        let outside = RelPath::under(Path::new("/repo"), Path::new("/elsewhere/file.rs"));
        assert!(
            matches!(outside, Err(TreeError::Walk { .. })),
            "a path the walk could not relativise must be an error, not an absolute RelPath"
        );
    }

    /// SURVIVES: behaviour. `is_under` is a path-segment test, not a string
    /// prefix: `crates/talkbank-lsp` must not be judged to sit under
    /// `crates/talkbank-l`.
    #[test]
    fn a_sibling_with_a_shared_prefix_is_not_under_the_directory() {
        let path = RelPath::new("crates/talkbank-lsp/src/lib.rs");
        assert!(path.is_under("crates/talkbank-lsp"));
        assert!(path.is_under("crates"));
        assert!(path.is_under(""));
        assert!(!path.is_under("crates/talkbank-l"));
    }

    /// SURVIVES: behaviour of the overlay, which no signature describes. A
    /// planted byte sequence that is not UTF-8 must reach the reader as a read
    /// FAILURE, because that is the only way to probe a gate's refusal to
    /// report a floor as a measurement.
    #[test]
    fn planted_bytes_that_are_not_utf8_fail_the_read() {
        let tree = Tree::rooted(Path::new("/nonexistent-root"));
        let planted = tree
            .planted(|edit| {
                edit.write_bytes("probe.rs", vec![0xFF, 0xFE]);
                Ok(())
            })
            .expect("the plant writes unconditionally");
        match planted.read_to_string(&RelPath::new("probe.rs")) {
            Err(TreeError::NotUtf8(path)) => assert_eq!(path.as_str(), "probe.rs"),
            Err(other) => panic!("expected a UTF-8 failure, got {other}"),
            Ok(text) => panic!("expected a UTF-8 failure, read {text:?}"),
        }
    }

    /// SURVIVES: behaviour. `replace_once` refuses an anchor it cannot place
    /// exactly once, so a drifted probe reports as drift rather than as a gate
    /// that cannot fail. Proving a guard fires is part of writing it.
    #[test]
    fn a_plant_whose_anchor_is_absent_fails_rather_than_changing_nothing() {
        let tree = Tree::rooted(Path::new("/nonexistent-root"));
        let outcome = tree.planted(|edit| {
            edit.write("probe.rs", "one two three\n");
            edit.replace_once("probe.rs", "four", "five")
        });
        match outcome {
            Err(failed) => assert!(
                failed.to_string().contains("expected exactly 1"),
                "the failure must say the anchor did not match: {failed}"
            ),
            Ok(_) => panic!("a plant whose anchor is absent must not report success"),
        }
    }
}

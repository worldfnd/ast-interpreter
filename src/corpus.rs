//! Corpus enumeration, source hashing, provenance and per-step run records: the machinery the
//! ledgers and the cross-field differential are generated from.

use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::Command;

use acvm::{AcirField, FieldElement};
use sha2::{Digest, Sha256};

use super::diff::{
    ComparableError, DUMP_FORMAT_VERSION, DiffValue, DumpProvenance, FailureKind, RunRecord,
    StepOutcome, comparable_error_of, normalize_text, values_equivalent,
};
use super::loader::NoirProject;
use super::projection::{PROJECTION_VERSION, projection_hash};
use super::validation_frontend::{Validated, ValidationError, compile_for_validation};
use super::{
    InterpretError, Value, expected_return_from_prover_toml, inputs_from_prover_toml,
    interpret_with_inputs,
};

/// The corpus, relative to a Noir checkout.
pub(crate) const CORPUS_SUBDIR: &str = "test_programs/execution_success";

pub(crate) fn referee_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The Noir checkout whose corpus the ledgers photograph: `$NOIR_CHECKOUT`, else the sibling
/// `../noir`. It has to be checked out at the pinned compiler revision; see
/// [`check_checkout_matches_stamp`].
pub(crate) fn noir_checkout() -> PathBuf {
    match std::env::var_os("NOIR_CHECKOUT") {
        Some(dir) => PathBuf::from(dir),
        None => referee_dir().join("../noir"),
    }
}

pub(crate) fn corpus_dir() -> PathBuf {
    noir_checkout().join(CORPUS_SUBDIR)
}

pub(crate) fn fixtures_dir() -> PathBuf {
    referee_dir().join("fixtures")
}

/// The field this build targets, used to tag dumps and ledgers (the two fields are separate builds).
pub(crate) fn field_tag() -> &'static str {
    if cfg!(feature = "goldilocks") {
        "goldilocks"
    } else {
        "bn254"
    }
}

/// The referee's enabled cargo features, sorted.
pub(crate) fn enabled_features() -> Vec<String> {
    let mut features = Vec::new();
    if cfg!(feature = "goldilocks") {
        features.push("goldilocks".to_string());
    }
    if cfg!(feature = "mavros-oracle") {
        features.push("mavros-oracle".to_string());
    }
    features
}

/// One program directory.
#[derive(Debug, Clone)]
pub(crate) struct CorpusProgram {
    pub name: String,
    pub dir: PathBuf,
    /// `Nargo.toml` declares a workspace; the referee runs single-package programs only.
    pub workspace: bool,
    pub source_hash: String,
}

/// Every directory under `root` with a `Nargo.toml`, in name order.
pub(crate) fn list_programs(root: &Path) -> Vec<CorpusProgram> {
    let mut programs: Vec<CorpusProgram> = std::fs::read_dir(root)
        .unwrap_or_else(|e| panic!("cannot read the program directory {}: {e}", root.display()))
        .map(|entry| entry.expect("program directory entry").path())
        .filter(|dir| dir.is_dir() && dir.join("Nargo.toml").is_file())
        .map(|dir| {
            let manifest = std::fs::read_to_string(dir.join("Nargo.toml")).unwrap_or_default();
            CorpusProgram {
                name: dir.file_name().unwrap().to_string_lossy().into_owned(),
                workspace: manifest.contains("[workspace]"),
                source_hash: source_hash(&dir),
                dir,
            }
        })
        .collect();
    programs.sort_by(|a, b| a.name.cmp(&b.name));
    programs
}

/// SHA-256 over the program's `Nargo.toml`, `Prover.toml` and `src/**`: each file as its
/// `/`-separated relative path, its length and its bytes, in path order. Build output and any
/// other file are ignored.
pub(crate) fn source_hash(dir: &Path) -> String {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for name in ["Nargo.toml", "Prover.toml"] {
        if let Ok(bytes) = std::fs::read(dir.join(name)) {
            files.push((name.to_string(), bytes));
        }
    }
    collect_files(&dir.join("src"), "src", &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Sha256::new();
    for (path, bytes) in &files {
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    hex(&hasher.finalize())
}

fn collect_files(dir: &Path, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.expect("source entry").path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let relative = format!("{prefix}/{name}");
        if path.is_dir() {
            collect_files(&path, &relative, out);
        } else {
            out.push((relative, std::fs::read(&path).expect("read source file")));
        }
    }
}

/// SHA-256 over every program's name and source hash, in name order.
pub(crate) fn corpus_hash(programs: &[CorpusProgram]) -> String {
    let mut hasher = Sha256::new();
    for program in programs {
        hasher.update(program.name.as_bytes());
        hasher.update(b" ");
        hasher.update(program.source_hash.as_bytes());
        hasher.update(b"\n");
    }
    hex(&hasher.finalize())
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Trimmed stdout of `git -C dir args`, or `None` when git fails.
pub(crate) fn git_output(dir: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// `rustc --version` as resolved from the referee's directory, so its `rust-toolchain.toml`
/// applies.
pub(crate) fn toolchain_version() -> String {
    Command::new("rustc")
        .arg("--version")
        .current_dir(referee_dir())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// The provenance of a dump of `programs` taken by this build.
pub(crate) fn provenance(corpus: &Path, programs: &[CorpusProgram]) -> DumpProvenance {
    let referee = referee_dir();
    DumpProvenance {
        format_version: DUMP_FORMAT_VERSION,
        projection_version: PROJECTION_VERSION,
        field: field_tag().to_string(),
        field_modulus: FieldElement::modulus().to_string(),
        noir_rev: noirc_driver::GIT_COMMIT.to_string(),
        interpreter_rev: git_output(&referee, &["rev-parse", "HEAD"])
            .unwrap_or_else(|| "unknown".to_string()),
        interpreter_dirty: git_output(&referee, &["status", "--porcelain"])
            .map(|status| !status.is_empty())
            .unwrap_or(true),
        corpus_dir: corpus.display().to_string(),
        corpus_hash: corpus_hash(programs),
        program_count: programs.len(),
        toolchain: toolchain_version(),
        features: enabled_features(),
        built_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_default(),
    }
}

/// The `rev` of every `worldfnd/noir` dependency in the referee's `Cargo.toml`, in file order.
pub(crate) fn pinned_revs() -> Vec<String> {
    let manifest = std::fs::read_to_string(referee_dir().join("Cargo.toml")).expect("Cargo.toml");
    manifest
        .lines()
        .filter(|line| line.contains("github.com/worldfnd/noir"))
        .filter_map(|line| {
            let (_, rest) = line.split_once("rev = \"")?;
            rest.split_once('"').map(|(rev, _)| rev.to_string())
        })
        .collect()
}

/// The pins must agree with each other and with the commit the compiler in this build was built
/// from; a path override or a stale build shows up here.
pub(crate) fn pin_disagreement(revs: &[String], stamp: &str) -> Result<(), String> {
    let Some(first) = revs.first() else {
        return Err("Cargo.toml pins no worldfnd/noir crate".to_string());
    };
    if let Some(odd) = revs.iter().find(|rev| *rev != first) {
        return Err(format!("Cargo.toml pins noir at both {first} and {odd}"));
    }
    if first != stamp {
        return Err(format!(
            "Cargo.toml pins noir at {first} but the compiler in this build was built from \
             {stamp}: a path override or a stale build"
        ));
    }
    Ok(())
}

/// The corpus checkout must sit at the stamped revision with a clean corpus directory.
pub(crate) fn check_checkout_matches_stamp(checkout: &Path) -> Result<(), String> {
    let head = git_output(checkout, &["rev-parse", "HEAD"]);
    let dirty: Vec<String> = git_output(checkout, &["status", "--porcelain", "--", CORPUS_SUBDIR])
        .map(|status| status.lines().map(str::to_string).collect())
        .unwrap_or_default();
    checkout_mismatch(checkout, head.as_deref(), noirc_driver::GIT_COMMIT, &dirty)
}

pub(crate) fn checkout_mismatch(
    checkout: &Path,
    head: Option<&str>,
    stamp: &str,
    dirty: &[String],
) -> Result<(), String> {
    let checkout = checkout.display();
    match head {
        None => Err(format!(
            "{checkout} is not a git checkout; the corpus must come from noir at {stamp}"
        )),
        Some(head) if head != stamp => Err(format!(
            "{checkout} is at {head} but the pinned compiler was built from {stamp}; check that \
             revision out there, or point NOIR_CHECKOUT at a worktree of it"
        )),
        Some(_) if !dirty.is_empty() => Err(format!(
            "{checkout} has local changes under {CORPUS_SUBDIR}: {}",
            dirty.join(", ")
        )),
        Some(_) => Ok(()),
    }
}

pub(crate) fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to);
        } else {
            std::fs::copy(&from, &to).unwrap();
        }
    }
}

/// The first non-empty line of a caught panic payload, for triage. Takes the payload itself, not
/// the `Box` around it: a `&Box<dyn Any>` also unsizes to `&dyn Any`, and that one never
/// downcasts to the message.
pub(crate) fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .and_then(|m| {
            m.lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "panic (payload is not a string)".to_string())
}

/// Run one step under its own panic guard: a panic becomes a `Panic` failure of that step alone.
pub(crate) fn run_step<T>(
    step: impl FnOnce() -> Result<T, (ComparableError, String)>,
) -> Result<T, StepOutcome> {
    match panic::catch_unwind(AssertUnwindSafe(step)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err((error, detail))) => Err(StepOutcome::failed(error, detail)),
        Err(payload) => {
            let message = panic_message(payload.as_ref());
            Err(StepOutcome::failed(
                ComparableError::new(FailureKind::Panic, normalize_text(&message)),
                message,
            ))
        }
    }
}

pub(crate) fn compile_error_of(error: &ValidationError) -> ComparableError {
    let kind = if error.is_dependency_compile_gap() {
        FailureKind::DependencyCompileGap
    } else {
        FailureKind::CompileError
    };
    ComparableError::new(kind, normalize_text(error.summary()))
}

fn interpret_failure(error: &InterpretError) -> (ComparableError, String) {
    (comparable_error_of(error), error.to_string())
}

/// The machine-specific roots a payload may mention, each with its stable stand-in: the program's
/// own directory, the Noir checkout, the referee and nargo's git-dependency cache (`~/nargo`,
/// where `nargo_toml` clones a program's git dependencies), in that order so the most specific
/// wins.
pub(crate) fn path_roots(program_dir: &Path) -> Vec<(PathBuf, &'static str)> {
    let mut roots = vec![
        (program_dir.to_path_buf(), "<pkg>"),
        (noir_checkout(), "<noir>"),
        (referee_dir(), "<referee>"),
    ];
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        roots.push((PathBuf::from(home).join("nargo"), "<nargo>"));
    }
    roots
}

/// Replace every root (and its canonical form) with its stand-in so payloads and details carry no
/// machine-specific path.
pub(crate) fn normalize_paths(text: &str, roots: &[(PathBuf, &str)]) -> String {
    let mut text = text.to_string();
    for (root, stand_in) in roots {
        let mut forms = vec![root.clone()];
        if let Ok(canonical) = std::fs::canonicalize(root) {
            if canonical != *root {
                forms.push(canonical);
            }
        }
        for form in forms {
            let form = form.to_string_lossy();
            let form = form.trim_end_matches('/');
            if form.is_empty() {
                continue;
            }
            text = text
                .replace(&format!("{form}/"), &format!("{stand_in}/"))
                .replace(form, stand_in);
        }
    }
    text
}

fn normalize_step(step: StepOutcome, roots: &[(PathBuf, &str)]) -> StepOutcome {
    match step {
        StepOutcome::Failed { error, detail } => StepOutcome::Failed {
            error: ComparableError::new(error.kind, normalize_paths(&error.payload, roots)),
            detail: normalize_paths(&detail, roots),
        },
        other => other,
    }
}

/// Run every step for `program` in place: a workspace manifest is recorded as not run, everything
/// else is taken through load, compile, projection, interpretation and the recorded-return check.
/// Running in place keeps path dependencies resolvable, as Noir's own test harness does; the
/// checkout's cleanliness is checked before a sweep and its sources are only read.
pub(crate) fn run_record(program: &CorpusProgram) -> RunRecord {
    if program.workspace {
        return RunRecord {
            source_hash: program.source_hash.clone(),
            load: StepOutcome::not_run(
                "workspace manifest: the referee runs single-package programs",
            ),
            compile: StepOutcome::not_run("not loaded"),
            interpret: StepOutcome::not_run("not compiled"),
            oracle: StepOutcome::not_run("not interpreted"),
            projection: StepOutcome::not_run("not compiled"),
            returned: None,
            projection_hash: None,
        };
    }
    let record = run_steps(&program.dir, program.source_hash.clone());
    let roots = path_roots(&program.dir);
    RunRecord {
        load: normalize_step(record.load, &roots),
        compile: normalize_step(record.compile, &roots),
        interpret: normalize_step(record.interpret, &roots),
        oracle: normalize_step(record.oracle, &roots),
        projection: normalize_step(record.projection, &roots),
        ..record
    }
}

fn run_steps(root: &Path, source_hash: String) -> RunRecord {
    let mut record = RunRecord {
        source_hash,
        load: StepOutcome::not_run("not attempted"),
        compile: StepOutcome::not_run("not loaded"),
        interpret: StepOutcome::not_run("not compiled"),
        oracle: StepOutcome::not_run("not interpreted"),
        projection: StepOutcome::not_run("not compiled"),
        returned: None,
        projection_hash: None,
    };

    let project = match run_step(|| {
        NoirProject::new(root.to_path_buf()).map_err(|e| {
            (
                ComparableError::new(FailureKind::ProjectLoad, normalize_text(&e)),
                e,
            )
        })
    }) {
        Ok(project) => {
            record.load = StepOutcome::Passed;
            project
        }
        Err(step) => {
            record.load = step;
            return record;
        }
    };

    let validated = match run_step(|| {
        compile_for_validation(&project).map_err(|e| (compile_error_of(&e), e.detail().to_string()))
    }) {
        Ok(validated) => {
            record.compile = StepOutcome::Passed;
            validated
        }
        Err(step) => {
            record.compile = step;
            return record;
        }
    };

    record.projection = match run_step(|| {
        Ok::<_, (ComparableError, String)>(projection_hash(&validated.program))
    }) {
        Ok(hash) => {
            record.projection_hash = Some(hash);
            StepOutcome::Passed
        }
        Err(step) => step,
    };

    let prover_src = std::fs::read_to_string(root.join("Prover.toml")).ok();
    let interpreted = run_step(|| {
        let inputs = match &prover_src {
            Some(src) => inputs_from_prover_toml(&validated.program, &validated.abi, src)
                .map_err(|e| interpret_failure(&e))?,
            None => Vec::new(),
        };
        interpret_with_inputs(&validated.program, inputs).map_err(|e| interpret_failure(&e))
    });
    match interpreted {
        Ok(value) => {
            record.interpret = StepOutcome::Passed;
            record.returned = Some(DiffValue::from_value(&value));
            record.oracle = oracle_step(&validated, prover_src.as_deref(), &value);
        }
        Err(step) => record.interpret = step,
    }
    record
}

/// Check the interpreter's return against the `return` recorded in `Prover.toml`. bn254 compares
/// exactly; goldilocks compares with `Field` values ignored, because the corpus records bn254's.
fn oracle_step(validated: &Validated, prover_src: Option<&str>, actual: &Value) -> StepOutcome {
    let Some(src) = prover_src else {
        return StepOutcome::not_run("no Prover.toml");
    };
    let recorded = run_step(|| {
        expected_return_from_prover_toml(&validated.program, &validated.abi, src)
            .map_err(|e| interpret_failure(&e))
    });
    match recorded {
        Err(step) => step,
        Ok(None) => StepOutcome::not_run("no recorded return"),
        Ok(Some(expected)) => compare_with_recorded(actual, &expected),
    }
}

pub(crate) fn compare_with_recorded(actual: &Value, expected: &Value) -> StepOutcome {
    let actual = DiffValue::from_value(actual);
    let expected = DiffValue::from_value(expected);
    if let Err(reason) = values_equivalent(&actual, &expected) {
        return StepOutcome::failed(
            ComparableError::new(FailureKind::OracleMismatch, normalize_text(&reason)),
            format!("interpreter returned {}", actual.render()),
        );
    }
    if cfg!(feature = "goldilocks") || actual == expected {
        StepOutcome::Passed
    } else {
        StepOutcome::failed(
            ComparableError::new(
                FailureKind::OracleMismatch,
                "field value differs from the recorded return",
            ),
            format!(
                "interpreter returned {}, recorded {}",
                actual.render(),
                expected.render()
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IntValue;
    use num_bigint::BigInt;

    fn write(dir: &Path, relative: &str, contents: &str) {
        let path = dir.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn source_hash_covers_manifest_inputs_and_sources_only() {
        let a = tempfile::tempdir().unwrap();
        write(a.path(), "src/main.nr", "fn main() {}");
        write(a.path(), "Nargo.toml", "[package]");
        write(a.path(), "Prover.toml", "x = 1");
        let b = tempfile::tempdir().unwrap();
        write(b.path(), "Prover.toml", "x = 1");
        write(b.path(), "Nargo.toml", "[package]");
        write(b.path(), "src/main.nr", "fn main() {}");
        write(b.path(), "target/out.json", "{}");
        write(b.path(), "README.md", "ignored");
        assert_eq!(source_hash(a.path()), source_hash(b.path()));

        write(b.path(), "src/lib.nr", "");
        assert_ne!(
            source_hash(a.path()),
            source_hash(b.path()),
            "an added source file must change the hash"
        );
        write(a.path(), "src/lib.nr", "");
        assert_eq!(source_hash(a.path()), source_hash(b.path()));
        write(a.path(), "Prover.toml", "x = 2");
        assert_ne!(
            source_hash(a.path()),
            source_hash(b.path()),
            "an edited input must change the hash"
        );
    }

    #[test]
    fn corpus_hash_follows_every_program() {
        let program = |name: &str, hash: &str| CorpusProgram {
            name: name.to_string(),
            dir: PathBuf::new(),
            workspace: false,
            source_hash: hash.to_string(),
        };
        let base = [program("a", "1"), program("b", "2")];
        assert_eq!(corpus_hash(&base), corpus_hash(&base.clone()));
        assert_ne!(
            corpus_hash(&base),
            corpus_hash(&[program("a", "1"), program("b", "3")])
        );
        assert_ne!(corpus_hash(&base), corpus_hash(&[program("a", "1")]));
        assert_ne!(
            corpus_hash(&base),
            corpus_hash(&[program("a", "1"), program("c", "2")])
        );
    }

    #[test]
    fn checkout_at_another_revision_is_refused() {
        let dir = Path::new("/tmp/noir");
        let err = checkout_mismatch(dir, Some("abc"), "def", &[]).unwrap_err();
        assert!(err.contains("abc") && err.contains("def"), "{err}");
        let err = checkout_mismatch(dir, None, "def", &[]).unwrap_err();
        assert!(err.contains("not a git checkout"), "{err}");
        let err = checkout_mismatch(dir, Some("def"), "def", &[" M x.nr".to_string()]).unwrap_err();
        assert!(err.contains("local changes"), "{err}");
        assert!(checkout_mismatch(dir, Some("def"), "def", &[]).is_ok());
    }

    #[test]
    fn disagreeing_pins_are_named() {
        let revs = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let err = pin_disagreement(&revs(&["a", "a", "b"]), "a").unwrap_err();
        assert!(err.contains("both a and b"), "{err}");
        let err = pin_disagreement(&revs(&["a", "a"]), "c").unwrap_err();
        assert!(
            err.contains("pins noir at a") && err.contains("built from c"),
            "{err}"
        );
        assert!(pin_disagreement(&revs(&[]), "a").is_err());
        assert!(pin_disagreement(&revs(&["a", "a"]), "a").is_ok());
    }

    /// The guard itself: every `worldfnd/noir` pin in `Cargo.toml` names the commit the compiler in
    /// this build stamped itself with, so no path override or stale build can produce a ledger.
    #[test]
    fn pinned_revs_agree_with_the_compiler_stamp() {
        let revs = pinned_revs();
        assert_eq!(
            revs.len(),
            11,
            "expected the eleven pinned noir crates: {revs:?}"
        );
        pin_disagreement(&revs, noirc_driver::GIT_COMMIT).unwrap();
    }

    #[test]
    fn a_panicking_step_is_recorded_as_a_panic_of_that_step() {
        let step = run_step(|| -> Result<(), (ComparableError, String)> { panic!("boom: {}", 42) })
            .unwrap_err();
        match step {
            StepOutcome::Failed { error, detail } => {
                assert_eq!(error.kind, FailureKind::Panic);
                assert_eq!(error.payload, "boom: 42");
                assert_eq!(detail, "boom: 42");
            }
            other => panic!("expected a failed step, got {other:?}"),
        }
        assert_eq!(
            run_step(|| Ok::<_, (ComparableError, String)>(7)).unwrap(),
            7
        );
    }

    #[test]
    fn machine_roots_are_normalized_out_of_payloads() {
        let roots = vec![
            (
                PathBuf::from("/home/me/noir/test_programs/execution_success/p"),
                "<pkg>",
            ),
            (PathBuf::from("/home/me/noir"), "<noir>"),
        ];
        let text = "manifest: /home/me/noir/test_programs/execution_success/p/Nargo.toml needs \
                    /home/me/noir/test_programs/test_libraries/dep (/home/me/noir)";
        assert_eq!(
            normalize_paths(text, &roots),
            "manifest: <pkg>/Nargo.toml needs <noir>/test_programs/test_libraries/dep (<noir>)"
        );
        let program = fixture_program("interp_basic");
        let roots = path_roots(&program.dir);
        assert_eq!(roots[0].1, "<pkg>");
        assert!(roots.iter().any(|(_, s)| *s == "<referee>"));
        // A git dependency's file lives in nargo's cache under the home directory, which differs
        // between machines; the ledger must not.
        let (nargo, _) = roots
            .iter()
            .find(|(_, s)| *s == "<nargo>")
            .expect("the nargo cache root");
        let text = format!(
            "files: std/hash/mod.nr, {}/github.com/noir-lang/poseidon/v0.3.0/src/poseidon2.nr",
            nargo.display()
        );
        assert_eq!(
            normalize_paths(&text, &roots),
            "files: std/hash/mod.nr, <nargo>/github.com/noir-lang/poseidon/v0.3.0/src/poseidon2.nr"
        );
    }

    fn fixture_program(name: &str) -> CorpusProgram {
        let dir = fixtures_dir().join(name);
        CorpusProgram {
            name: name.to_string(),
            workspace: false,
            source_hash: source_hash(&dir),
            dir,
        }
    }

    #[test]
    fn a_compile_failure_leaves_the_later_steps_not_run() {
        let record = run_record(&fixture_program("neg_reachable_error"));
        assert!(record.load.passed(), "{:?}", record.load);
        assert!(
            !record.compile.passed(),
            "a reachable type error must not compile"
        );
        assert!(
            matches!(record.interpret, StepOutcome::NotRun { .. }),
            "{:?}",
            record.interpret
        );
        assert!(
            matches!(record.oracle, StepOutcome::NotRun { .. }),
            "{:?}",
            record.oracle
        );
        assert!(matches!(record.projection, StepOutcome::NotRun { .. }));
        assert_eq!(record.returned, None);
        assert_eq!(record.projection_hash, None);
    }

    #[test]
    fn a_returning_fixture_records_its_value_and_projection() {
        let record = run_record(&fixture_program("interp_inputs_u64"));
        assert!(record.compile.passed(), "{:?}", record.compile);
        assert!(record.interpret.passed(), "{:?}", record.interpret);
        assert_eq!(
            record.returned,
            Some(DiffValue::Int {
                signed: false,
                bits: 64,
                value: BigInt::from(18446744069414584328u64),
            })
        );
        assert!(record.projection.passed(), "{:?}", record.projection);
        assert_eq!(record.projection_hash.as_deref().map(str::len), Some(64));
        assert_eq!(record.oracle, StepOutcome::not_run("no recorded return"));
        assert_eq!(record.source_hash.len(), 64);
    }

    #[test]
    fn a_workspace_manifest_is_a_not_run_row() {
        let program = CorpusProgram {
            name: "ws".to_string(),
            dir: PathBuf::new(),
            workspace: true,
            source_hash: "0".repeat(64),
        };
        let record = run_record(&program);
        assert!(matches!(record.load, StepOutcome::NotRun { .. }));
        assert_eq!(record.returned, None);
    }

    #[test]
    fn recorded_return_comparison_is_exact_on_bn254_and_field_opaque_on_goldilocks() {
        let int = |v: u64| {
            Value::Int(IntValue {
                signed: false,
                bits: 64,
                value: BigInt::from(v),
            })
        };
        assert!(compare_with_recorded(&int(7), &int(7)).passed());
        let mismatch = compare_with_recorded(&int(7), &int(8));
        assert_eq!(
            mismatch.failure().map(|e| &e.kind),
            Some(&FailureKind::OracleMismatch)
        );

        let field = |v: u64| Value::Field(FieldElement::from(v as u128));
        let differing = compare_with_recorded(&field(1), &field(2));
        if cfg!(feature = "goldilocks") {
            assert!(differing.passed(), "{differing:?}");
        } else {
            assert_eq!(
                differing.failure().map(|e| e.payload.as_str()),
                Some("field value differs from the recorded return")
            );
        }
    }

    #[test]
    fn enabled_features_follow_the_build() {
        let features = enabled_features();
        assert_eq!(
            features.contains(&"goldilocks".to_string()),
            cfg!(feature = "goldilocks")
        );
    }
}

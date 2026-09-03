//! Corpus loading, source hashes, provenance, and per-step run records.

use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::Command;

use acvm::{AcirField, FieldElement};
use fm::NormalizePath;
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

const CORPUS_SUBDIR: &str = "test_programs/execution_success";

pub(crate) fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `$NOIR_CHECKOUT`, or the sibling `../noir` checkout at the pinned compiler revision.
pub(crate) fn noir_checkout() -> PathBuf {
    match std::env::var_os("NOIR_CHECKOUT") {
        Some(dir) => PathBuf::from(dir),
        None => crate_dir().join("../noir"),
    }
}

pub(crate) fn corpus_dir() -> PathBuf {
    noir_checkout().join(CORPUS_SUBDIR)
}

pub(crate) fn fixtures_dir() -> PathBuf {
    crate_dir().join("fixtures")
}

pub(crate) fn field_tag() -> &'static str {
    if cfg!(feature = "goldilocks") {
        "goldilocks"
    } else {
        "bn254"
    }
}

fn enabled_features() -> Vec<String> {
    if cfg!(feature = "goldilocks") {
        vec!["goldilocks".to_string()]
    } else {
        Vec::new()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CorpusProgram {
    pub name: String,
    pub dir: PathBuf,
    /// `Nargo.toml` declares a workspace; the interpreter runs single-package programs only.
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

/// SHA-256 over the git-tracked files under `dir`, as `/`-separated relative path, length and
/// bytes, in path order; ignored and untracked files, build output included, do not count.
fn source_hash(dir: &Path) -> String {
    let listing = git_output(dir, &["ls-files", "-z", "--", "."])
        .unwrap_or_else(|| panic!("{} is not inside a git checkout", dir.display()));
    let mut files: Vec<&str> = listing
        .split('\0')
        .filter(|path| !path.is_empty())
        .collect();
    assert!(
        !files.is_empty(),
        "no tracked files under {}",
        dir.display()
    );
    files.sort_unstable();
    let mut hasher = Sha256::new();
    for path in files {
        let bytes = std::fs::read(dir.join(path))
            .unwrap_or_else(|e| panic!("cannot read {path} under {}: {e}", dir.display()));
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    hex(&hasher.finalize())
}

/// SHA-256 over every program's name and source hash, in name order.
fn corpus_hash(programs: &[CorpusProgram]) -> String {
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
fn git_output(dir: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// `rustc --version` resolved from this crate's directory, so its `rust-toolchain.toml` applies.
fn toolchain_version() -> String {
    Command::new("rustc")
        .arg("--version")
        .current_dir(crate_dir())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) fn provenance(corpus: &Path, programs: &[CorpusProgram]) -> DumpProvenance {
    let root = crate_dir();
    DumpProvenance {
        format_version: DUMP_FORMAT_VERSION,
        projection_version: PROJECTION_VERSION,
        field: field_tag().to_string(),
        field_modulus: FieldElement::modulus().to_string(),
        noir_rev: noirc_driver::GIT_COMMIT.to_string(),
        interpreter_rev: git_output(&root, &["rev-parse", "HEAD"])
            .unwrap_or_else(|| "unknown".to_string()),
        interpreter_dirty: git_output(&root, &["status", "--porcelain"])
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

/// The `rev` of every `worldfnd/noir` dependency in `Cargo.toml`, in file order.
fn pinned_revs() -> Vec<String> {
    let manifest = std::fs::read_to_string(crate_dir().join("Cargo.toml")).expect("Cargo.toml");
    manifest
        .lines()
        .filter(|line| line.contains("github.com/worldfnd/noir"))
        .filter_map(|line| {
            let (_, rest) = line.split_once("rev = \"")?;
            rest.split_once('"').map(|(rev, _)| rev.to_string())
        })
        .collect()
}

fn pin_disagreement(revs: &[String], stamp: &str) -> Result<(), String> {
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

/// Refuse `checkout` unless it is at the stamped revision with a clean corpus and path
/// dependencies.
pub(crate) fn check_checkout_matches_stamp(checkout: &Path) -> Result<(), String> {
    let head = git_output(checkout, &["rev-parse", "HEAD"]);
    let status = git_output(
        checkout,
        &[
            "status",
            "--porcelain",
            "--",
            CORPUS_SUBDIR,
            "test_programs/test_libraries",
        ],
    )
    .ok_or_else(|| format!("cannot check corpus cleanliness at {}", checkout.display()))?;
    let dirty: Vec<String> = status.lines().map(str::to_string).collect();
    checkout_mismatch(checkout, head.as_deref(), noirc_driver::GIT_COMMIT, &dirty)
}

fn checkout_mismatch(
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
            "{checkout} has local changes in the corpus or its path dependencies: {}",
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

/// The text of a panic payload. Takes the payload, not the `Box`: `&Box<dyn Any>` also unsizes
/// to `&dyn Any` and never downcasts to the message.
pub(crate) fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic (payload is not a string)".to_string())
}

/// Run one step under its own panic guard.
fn run_step<T>(
    step: impl FnOnce() -> Result<T, (ComparableError, String)>,
) -> Result<T, StepOutcome> {
    match panic::catch_unwind(AssertUnwindSafe(step)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err((error, detail))) => Err(StepOutcome::failed(error, detail)),
        Err(payload) => {
            let message = panic_message(payload.as_ref());
            let first_line = message
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or("panic");
            Err(StepOutcome::failed(
                ComparableError::new(FailureKind::Panic, normalize_text(first_line)),
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

/// Machine-specific roots and their stand-ins, most specific first; `~/nargo` is nargo's
/// git-dependency cache.
fn path_roots(program_dir: &Path) -> Vec<(PathBuf, &'static str)> {
    let mut roots = vec![
        (program_dir.to_path_buf(), "<pkg>"),
        (noir_checkout(), "<noir>"),
        (crate_dir(), "<crate>"),
    ];
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        roots.push((PathBuf::from(home).join("nargo"), "<nargo>"));
    }
    roots
}

/// Replace every root, in its given, lexically normalized and canonical forms, with its stand-in.
fn normalize_paths(text: &str, roots: &[(PathBuf, &str)]) -> String {
    let mut text = text.to_string();
    for (root, stand_in) in roots {
        let mut forms = vec![root.clone(), root.normalize()];
        forms.extend(std::fs::canonicalize(root));
        forms.sort();
        forms.dedup();
        for form in forms {
            let form = form.to_string_lossy();
            let form = form.trim_end_matches('/');
            if !form.is_empty() {
                text = replace_root(&text, form, stand_in);
            }
        }
    }
    text
}

/// Replace `root` where a separator, a delimiter or the end of the text follows it, never where it
/// is the prefix of a longer name.
fn replace_root(text: &str, root: &str, stand_in: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(root) {
        let end = start + root.len();
        let boundary = rest[end..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.')));
        out.push_str(&rest[..start]);
        out.push_str(if boundary { stand_in } else { root });
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
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

/// Run in place so path dependencies resolve; the sweep checks checkout cleanliness first.
pub(crate) fn run_record(program: &CorpusProgram) -> RunRecord {
    if program.workspace {
        return RunRecord {
            source_hash: program.source_hash.clone(),
            load: StepOutcome::not_run(
                "workspace manifest: the interpreter runs single-package programs",
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

/// Check the return against the `return` recorded in `Prover.toml`: exactly under bn254, with
/// `Field` values ignored under goldilocks because the corpus records bn254's. A recorded return
/// the field cannot decode leaves the check not run.
fn oracle_step(validated: &Validated, prover_src: Option<&str>, actual: &Value) -> StepOutcome {
    let Some(src) = prover_src else {
        return StepOutcome::not_run("no Prover.toml");
    };
    let recorded = run_step(|| {
        expected_return_from_prover_toml(&validated.program, &validated.abi, src)
            .map_err(|e| interpret_failure(&e))
    });
    match recorded {
        Err(StepOutcome::Failed { error, .. })
            if matches!(error.kind, FailureKind::Unsupported { .. }) =>
        {
            StepOutcome::not_run(format!("recorded return: {}", error.payload))
        }
        Err(step) => step,
        Ok(None) => StepOutcome::not_run("no recorded return"),
        Ok(Some(expected)) => compare_with_recorded(actual, &expected),
    }
}

fn compare_with_recorded(actual: &Value, expected: &Value) -> StepOutcome {
    let actual = DiffValue::from_value(actual);
    let expected = DiffValue::from_value(expected);
    if let Err(reason) = values_equivalent(&actual, &expected) {
        return StepOutcome::failed(
            ComparableError::new(FailureKind::OracleMismatch, normalize_text(&reason)),
            format!("interpreter returned {actual}"),
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
            format!("interpreter returned {actual}, recorded {expected}"),
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
    fn source_hash_covers_tracked_files_only() {
        let repo = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?}");
        };
        git(&["init", "-q"]);
        let dir = repo.path().join("p");
        write(&dir, "Nargo.toml", "[package]");
        write(&dir, "src/main.nr", "fn main() {}");
        write(repo.path(), ".gitignore", "ignored\n");
        git(&["add", "-A"]);
        let tracked = source_hash(&dir);

        write(&dir, "ignored", "x");
        write(&dir, "target/out.json", "{}");
        write(&dir, "untracked.nr", "");
        assert_eq!(source_hash(&dir), tracked);
        write(&dir, "src/main.nr", "fn main() { let _x = 1; }");
        assert_ne!(source_hash(&dir), tracked);
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
    fn pins_agree_with_each_other_and_with_the_compiler_stamp() {
        let revs = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(pin_disagreement(&revs(&["a", "a", "b"]), "a").is_err());
        assert!(pin_disagreement(&revs(&["a", "a"]), "c").is_err());
        assert!(pin_disagreement(&revs(&[]), "a").is_err());

        let pinned = pinned_revs();
        assert_eq!(pinned.len(), 11, "{pinned:?}");
        pin_disagreement(&pinned, noirc_driver::GIT_COMMIT).unwrap();
    }

    #[test]
    fn a_panicking_step_is_recorded_as_a_panic_of_that_step() {
        let step = run_step(|| -> Result<(), (ComparableError, String)> {
            panic!("boom: {}\nleft: 1\nright: 2", 42)
        })
        .unwrap_err();
        match step {
            StepOutcome::Failed { error, detail } => {
                assert_eq!(error.kind, FailureKind::Panic);
                assert_eq!(error.payload, "boom: 42");
                assert_eq!(detail, "boom: 42\nleft: 1\nright: 2");
            }
            other => panic!("expected a failed step, got {other:?}"),
        }
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
                    /home/me/noir/test_programs/test_libraries/dep (/home/me/noir), not \
                    /home/me/noir-other/x";
        assert_eq!(
            normalize_paths(text, &roots),
            "manifest: <pkg>/Nargo.toml needs <noir>/test_programs/test_libraries/dep (<noir>), \
             not /home/me/noir-other/x"
        );
        let program = fixture_program("interp_basic");
        let roots = path_roots(&program.dir);
        assert_eq!(roots[0].1, "<pkg>");
        assert!(roots.iter().any(|(_, s)| *s == "<crate>"));
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
    }

    #[test]
    fn an_unrepresentable_recorded_return_leaves_the_return_check_not_run() {
        let record = run_record(&fixture_program("interp_return_i64"));
        assert!(record.interpret.passed(), "{:?}", record.interpret);
        if cfg!(feature = "goldilocks") {
            assert!(
                matches!(&record.oracle, StepOutcome::NotRun { reason } if reason.starts_with("recorded return:")),
                "{:?}",
                record.oracle
            );
        } else {
            assert!(record.oracle.passed(), "{:?}", record.oracle);
        }
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
}

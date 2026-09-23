//! Differential oracle against Mavros.
//!
//! Mavros compiles a program with its own driver, and the interpreter runs the very monomorphized
//! program that driver lowers, so the two can only disagree in the back half. Mavros's witness
//! generator then runs twice: once with the return check off, which says whether Mavros executes
//! the program at all, and once declaring the interpreter's return value, which says whether Mavros
//! computes the same result. `Prover.toml`'s own `return`, where the corpus records one, is Noir's
//! executor output and is compared with the interpreter's as a third opinion.
//!
//! Each program runs in a child process, because a Mavros stack overflow or abort would otherwise
//! end the sweep. The sweep needs LLVM 22 for the Mavros build (see `mavros/flake.nix`):
//!
//! ```sh
//! MAVROS_ORACLE_CORPUS=<a copy of a test_programs directory> MAVROS_ORACLE_FIELD=bn254 \
//!     cargo test --release --features mavros-oracle --lib mavros_oracle::sweep -- --ignored --nocapture
//! ```
//!
//! The release build matters: a debug build's stack frames and speed make the heaviest programs
//! (ECDSA, the 2^17-gate benchmarks) overflow the child's stack or run past the budget.
//!
//! `MAVROS_ORACLE_CORPUS` names a copy, not a checkout, because Mavros writes `mavros_debug/`
//! into every package it compiles. `MAVROS_ORACLE_FIELD` defaults to bn254; `MAVROS_ORACLE_JOBS`
//! (default 8) bounds the children in flight and `MAVROS_ORACLE_TIMEOUT_SECS` (default 600)
//! kills one that runs longer. `MAVROS_ORACLE_GENERIC_BUILTINS=1` runs the sweep in Noir's
//! benchmark mode, the standard library's field-generic builtins in place of their bn254 twins,
//! and names its outputs `<field>-generic-builtins.{jsonl,md}`, so the two modes can be compared
//! program by program.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use acvm::{FieldConfig, FieldId};
use mavros_compiler::compiler::codegen::CodeGenOptions;
use mavros_compiler::compiler::codegen::hlssa_to_r1cs::R1CS;
use mavros_compiler::driver::{BytecodeArtifact, Driver, Error as DriverError};
use mavros_compiler::vm::interpreter;
use mavros_compiler::{Project, abi_helpers};
use noirc_abi::input_parser::{Format, InputValue};
use noirc_abi::{Abi, AbiType, InputMap, MAIN_RETURN_NAME, decode_scalar};
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};

use super::corpus::{crate_dir, panic_message};
use super::diff::comparable_error_of;
use super::{
    Value, expected_return_from_prover_toml, inputs_from_prover_toml, interpret_with_inputs,
};

/// Programs the sweep never starts: Mavros's VM has no execution budget and these do not terminate.
const SKIPPED: &[&str] = &["brillig_mem_layout_regression"];

/// Marks the one stdout line a child reports its record on.
const RECORD_PREFIX: &str = "MAVROS-ORACLE-RECORD: ";

/// What the interpreter and Mavros each made of one program.
#[derive(Serialize, Deserialize, Debug)]
struct Record {
    name: String,
    expect_failure: bool,
    /// `ok`, or the interpreter's failure kind and message; `not-run` when there is no program.
    interp: String,
    /// The interpreter's return value, rendered.
    interp_value: Option<String>,
    /// `Prover.toml`'s recorded `return` against the interpreter's: `absent`, `agrees`, `differs`
    /// or `unreadable`; `not-run` when the interpreter failed.
    recorded_return: String,
    /// How far Mavros's compiler got: `ok`, or the stage and reason it stopped.
    mavros_compile: String,
    /// Witness generation with the return check off.
    witgen_unchecked: String,
    /// Witness generation declaring the interpreter's return value.
    witgen_declared: String,
    verdict: String,
}

impl Record {
    fn new(name: String, expect_failure: bool) -> Self {
        Record {
            name,
            expect_failure,
            interp: "not-run".into(),
            interp_value: None,
            recorded_return: "not-run".into(),
            mavros_compile: "not-run".into(),
            witgen_unchecked: "not-run".into(),
            witgen_declared: "not-run".into(),
            verdict: String::new(),
        }
    }
}

/// The ABI input value that declares `value` as a return of `typ` in `field`: the inverse of the
/// interpreter's input bridge, so Mavros can check its own result against the interpreter's.
fn input_from_value(
    value: &Value,
    typ: &AbiType,
    field: FieldConfig,
) -> Result<InputValue, String> {
    let scalar = |pattern: BigUint| decode_scalar(pattern, typ, field).map_err(|e| e.to_string());
    match (value, typ) {
        (Value::Field(element), AbiType::Field) => scalar(element.as_biguint().clone()),
        (Value::Int(int), AbiType::Integer { .. }) => scalar(
            int.unsigned_repr()
                .to_biguint()
                .ok_or_else(|| format!("{int:?} has a negative bit pattern"))?,
        ),
        (Value::Bool(b), AbiType::Boolean) => scalar(BigUint::from(u8::from(*b))),
        (Value::Array(elements), AbiType::Array { length, typ }) => {
            if elements.len() != *length as usize {
                return Err(format!(
                    "{} elements for an array of {length}",
                    elements.len()
                ));
            }
            elements
                .iter()
                .map(|element| input_from_value(element, typ, field))
                .collect::<Result<_, _>>()
                .map(InputValue::Vec)
        }
        (Value::Tuple(cells), AbiType::Tuple { fields }) if cells.len() == fields.len() => cells
            .iter()
            .zip(fields)
            .map(|(cell, typ)| input_from_value(&cell.borrow(), typ, field))
            .collect::<Result<_, _>>()
            .map(InputValue::Vec),
        (Value::Tuple(cells), AbiType::Struct { fields, .. }) if cells.len() == fields.len() => {
            cells
                .iter()
                .zip(fields)
                .map(|(cell, (name, typ))| {
                    input_from_value(&cell.borrow(), typ, field).map(|v| (name.clone(), v))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()
                .map(InputValue::Struct)
        }
        (Value::Str(bytes), AbiType::String { .. }) => String::from_utf8(bytes.clone())
            .map(InputValue::String)
            .map_err(|e| format!("a returned string is not UTF-8: {e}")),
        (value, typ) => Err(format!("cannot declare {value:?} as a return of {typ:?}")),
    }
}

/// Run `f`, turning a panic into its message.
fn caught<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(|payload| panic_message(payload.as_ref()))
}

/// The first line of `text`, bounded, for a one-line table cell.
fn line(text: impl AsRef<str>) -> String {
    let first = text.as_ref().lines().next().unwrap_or_default();
    let mut bounded: String = first.chars().take(200).collect();
    if bounded.len() < first.len() {
        bounded.push('…');
    }
    bounded
}

/// Mavros's witness generation over `inputs`, checked against its own R1CS and return guard.
fn witgen(r1cs: &R1CS, artifact: &BytecodeArtifact, abi: &Abi, inputs: &InputMap) -> String {
    let params = match abi_helpers::ordered_params_from_btreemap(abi, inputs) {
        Ok(params) => params,
        Err(e) => return format!("params: {}", line(e)),
    };
    let run = caught(|| {
        interpreter::run(
            &artifact.binary,
            r1cs.witness_layout,
            r1cs.constraints_layout,
            &params,
            None,
        )
    });
    match run {
        Ok(Ok(result)) => {
            let satisfied = r1cs.check_witgen_output(
                &result.out_wit_pre_comm,
                &result.out_wit_post_comm,
                &result.out_a,
                &result.out_b,
                &result.out_c,
            );
            let guard = abi_helpers::check_return_guard(
                abi,
                &r1cs.witness_layout,
                &params,
                &result.out_wit_pre_comm,
            );
            match (satisfied, guard) {
                (true, Ok(())) => "ok".into(),
                (false, _) => "unsat".into(),
                (true, Err(e)) => format!("guard: {}", line(e)),
            }
        }
        Ok(Err(trap)) => format!("trap: {}", line(trap.to_string())),
        Err(panic) => format!("panic: {}", line(panic)),
    }
}

/// Compile `dir` with Mavros under `field`, interpret the program it lowers, and run its witness
/// generation with and without the interpreter's return value declared.
fn examine(
    dir: &Path,
    name: String,
    expect_failure: bool,
    field: FieldId,
    generic_builtins: bool,
) -> Record {
    let mut record = Record::new(name, expect_failure);
    let prover_toml = std::fs::read_to_string(dir.join("Prover.toml")).ok();

    let project = match Project::new(dir.to_path_buf()) {
        Ok(project) => project,
        Err(e) => {
            record.mavros_compile = format!("project: {}", line(e.to_string()));
            return record;
        }
    };
    let mut driver = Driver::new(project, false);
    driver.set_noir_field(field);
    driver.set_noir_generic_builtins(generic_builtins);
    // A failure to lower the monomorphized program is Mavros's; the program itself survives it,
    // so the interpreter still gets its turn.
    let lowering_failure = match caught(|| driver.run_noir_compiler()) {
        Ok(Ok(())) => None,
        Ok(Err(DriverError::NoirCompilerError(diagnostics))) => {
            let first = diagnostics
                .iter()
                .find(|d| d.is_error())
                .map_or_else(String::new, |d| d.message.clone());
            record.mavros_compile = format!("frontend-reject: {}", line(first));
            return record;
        }
        Ok(Err(e)) => Some(format!("lowering-fail: {}", line(e.to_string()))),
        Err(panic) => Some(format!("lowering-panic: {}", line(panic))),
    };
    let Some((program, abi)) = driver.frontend_output() else {
        record.mavros_compile = lowering_failure.unwrap_or_else(|| "no program".into());
        return record;
    };
    let abi = abi.clone();

    // The interpreter on the program Mavros lowers, with Mavros's stdlib replacements in it.
    let interp = caught(|| {
        let inputs = match &prover_toml {
            Some(src) => inputs_from_prover_toml(program, &abi, src, field)?,
            None => Vec::new(),
        };
        interpret_with_inputs(program, inputs, field)
    });
    let interp: Result<Value, String> = match interp {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(e)) => Err(format!(
            "{:?}: {}",
            comparable_error_of(&e).kind,
            line(e.to_string())
        )),
        Err(panic) => Err(format!("Panic: {}", line(panic))),
    };
    record.interp = match &interp {
        Ok(_) => "ok".into(),
        Err(e) => e.clone(),
    };
    record.interp_value = interp.as_ref().ok().map(|v| line(format!("{v:?}")));
    record.recorded_return = match (&prover_toml, &interp) {
        (Some(src), Ok(value)) => {
            match caught(|| expected_return_from_prover_toml(program, &abi, src, field)) {
                Ok(Ok(Some(recorded))) if recorded == *value => "agrees".into(),
                Ok(Ok(Some(recorded))) => {
                    format!("differs: recorded {}", line(format!("{recorded:?}")))
                }
                Ok(Ok(None)) => "absent".into(),
                Ok(Err(e)) => format!("unreadable: {}", line(e.to_string())),
                Err(panic) => format!("unreadable: {}", line(panic)),
            }
        }
        (None, _) => "absent".into(),
        (_, Err(_)) => "not-run".into(),
    };
    if let Some(failure) = lowering_failure {
        record.mavros_compile = failure;
        return record;
    }

    let compiled = caught(|| -> Result<(R1CS, BytecodeArtifact), DriverError> {
        driver.make_struct_access_static()?;
        driver.monomorphize()?;
        driver.spill_witness()?;
        let r1cs = driver.generate_r1cs()?;
        let artifact = driver.compile_bytecode_artifact(CodeGenOptions {
            check_constraints: true,
            include_debug_info: false,
        })?;
        Ok((r1cs, artifact))
    });
    let (r1cs, artifact) = match compiled {
        Ok(Ok(compiled)) => compiled,
        Ok(Err(
            e @ (DriverError::AssertConstantFailed(_)
            | DriverError::Refused(_)
            | DriverError::UnsatisfiableProgram(_)),
        )) => {
            record.mavros_compile = format!("reject: {}", line(e.to_string()));
            return record;
        }
        Ok(Err(e)) => {
            record.mavros_compile = format!("fail: {}", line(e.to_string()));
            return record;
        }
        Err(panic) => {
            record.mavros_compile = format!("panic: {}", line(panic));
            return record;
        }
    };
    record.mavros_compile = "ok".into();

    // The parameters only: a recorded `return` the field cannot hold must not stop the run, as
    // the interpreter's own input bridge has it.
    let parameters_only = Abi {
        return_type: None,
        ..abi.clone()
    };
    let mut inputs = match &prover_toml {
        Some(src) => match Format::Toml.parse(src, &parameters_only, FieldConfig::new(field)) {
            Ok(inputs) => inputs,
            Err(e) => {
                record.witgen_unchecked = format!("inputs: {}", line(e.to_string()));
                return record;
            }
        },
        None => InputMap::new(),
    };
    record.witgen_unchecked = witgen(&r1cs, &artifact, &abi, &inputs);
    record.witgen_declared = match (&interp, &abi.return_type) {
        (Ok(value), Some(ret)) => {
            match input_from_value(value, &ret.abi_type, FieldConfig::new(field)) {
                Ok(declared) => {
                    inputs.insert(MAIN_RETURN_NAME.to_string(), declared);
                    witgen(&r1cs, &artifact, &abi, &inputs)
                }
                Err(e) => format!("unencodable: {}", line(e)),
            }
        }
        (Ok(_), None) => "no-return".into(),
        (Err(_), _) => "not-run".into(),
    };
    record
}

/// One word for what the two sides say together. A side that could not process the program
/// (a panic, a lowering failure, no program) has a gap, which is neither acceptance nor
/// rejection; a Mavros rejection is a refused compile or a witness generation that traps.
fn verdict(record: &Record) -> String {
    let interp_ok = record.interp == "ok";
    let interp_gap = record.interp.starts_with("Unsupported");
    let interp_internal =
        record.interp.starts_with("Internal") || record.interp.starts_with("Panic");
    let compile = record.mavros_compile.as_str();
    let mavros_gap = [
        "panic",
        "fail",
        "lowering-fail",
        "lowering-panic",
        "project",
        "no program",
    ]
    .iter()
    .any(|prefix| compile.starts_with(prefix))
        || record.witgen_unchecked.starts_with("panic");
    let mavros_rejects =
        compile.starts_with("reject") || (compile == "ok" && record.witgen_unchecked != "ok");
    let mavros_accepts = compile == "ok" && record.witgen_unchecked == "ok";

    if compile.starts_with("frontend-reject") {
        return "frontend-reject".into();
    }
    if mavros_gap {
        return if interp_gap || interp_internal {
            "both-gap"
        } else {
            "mavros-gap"
        }
        .into();
    }
    if interp_internal {
        return "interp-internal".into();
    }
    if record.expect_failure {
        return match (interp_ok, mavros_rejects, mavros_accepts) {
            (false, true, _) if interp_gap => "interp-gap".into(),
            (false, true, _) => "agree-reject".into(),
            (false, _, true) if interp_gap => "interp-gap".into(),
            (false, _, true) => "MAVROS-ACCEPTS".into(),
            (true, true, _) => "INTERP-ACCEPTS".into(),
            (true, _, true) => "both-accept".into(),
            _ => "unclassified".into(),
        };
    }
    if !interp_ok {
        return if interp_gap {
            "interp-gap".into()
        } else if mavros_accepts {
            "INTERP-REJECTS".into()
        } else {
            "both-reject".into()
        };
    }
    if !mavros_accepts {
        return "MAVROS-REJECTS".into();
    }
    match record.witgen_declared.as_str() {
        "ok" | "no-return" => "agree".into(),
        declared if declared.starts_with("unencodable") => "unencodable-return".into(),
        _ => "RETURN-MISMATCH".into(),
    }
}

/// The field the sweep compiles and interprets under.
fn sweep_field() -> FieldId {
    std::env::var("MAVROS_ORACLE_FIELD")
        .map(|name| name.parse().expect("MAVROS_ORACLE_FIELD"))
        .unwrap_or(FieldId::Bn254)
}

/// Whether the sweep runs in Noir's benchmark mode (`MAVROS_ORACLE_GENERIC_BUILTINS=1`).
fn sweep_generic_builtins() -> bool {
    std::env::var("MAVROS_ORACLE_GENERIC_BUILTINS").is_ok_and(|value| value == "1")
}

/// The stem of the sweep's output files: the field, and the mode when it is not the default.
fn sweep_output_stem(field: FieldId, generic_builtins: bool) -> String {
    if generic_builtins {
        format!("{field}-generic-builtins")
    } else {
        field.to_string()
    }
}

/// One program for [`sweep`], named by `MAVROS_ORACLE_DIR`; it prints a single record line.
#[test]
#[ignore = "one program of the Mavros oracle sweep; driven through MAVROS_ORACLE_DIR"]
fn child() {
    let Some(dir) = std::env::var_os("MAVROS_ORACLE_DIR").map(PathBuf::from) else {
        return;
    };
    let name = std::env::var("MAVROS_ORACLE_NAME").unwrap_or_else(|_| dir.display().to_string());
    let expect_failure = std::env::var("MAVROS_ORACLE_EXPECT_FAILURE").is_ok_and(|v| v == "1");
    let field = sweep_field();
    let generic_builtins = sweep_generic_builtins();
    // Noir's frontend and the interpreter both recurse on the program's depth.
    let record = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let mut record = examine(&dir, name, expect_failure, field, generic_builtins);
            record.verdict = verdict(&record);
            record
        })
        .expect("spawn")
        .join()
        .expect("examine panicked outside its guards");
    println!(
        "{RECORD_PREFIX}{}",
        serde_json::to_string(&record).expect("record serializes")
    );
}

/// A binary package of the corpus, with the name its row carries.
struct Job {
    dir: PathBuf,
    name: String,
    expect_failure: bool,
}

fn jobs(corpus: &Path) -> Vec<Job> {
    let mut jobs = Vec::new();
    for (sub, expect_failure) in [("execution_success", false), ("execution_failure", true)] {
        let base = corpus.join(sub);
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|d| d.join("Nargo.toml").is_file())
            .collect();
        dirs.sort();
        for dir in dirs {
            let fixture = dir.file_name().unwrap().to_string_lossy().into_owned();
            if SKIPPED.contains(&fixture.as_str()) {
                continue;
            }
            let members = Project::binary_package_roots(&dir).unwrap_or_else(|_| vec![dir.clone()]);
            for member in members {
                let suffix = member
                    .strip_prefix(&dir)
                    .unwrap_or(&member)
                    .display()
                    .to_string();
                let name = if suffix.is_empty() {
                    format!("{sub}/{fixture}")
                } else {
                    format!("{sub}/{fixture}/{suffix}")
                };
                jobs.push(Job {
                    dir: member,
                    name,
                    expect_failure,
                });
            }
        }
    }
    jobs
}

/// Run one job in a child process, killing it past `timeout`.
fn run_job(job: &Job, field: FieldId, generic_builtins: bool, timeout: Duration) -> Record {
    let exe = std::env::current_exe().expect("test binary path");
    let mut child = Command::new(exe)
        .args([
            "mavros_oracle::child",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("MAVROS_ORACLE_DIR", &job.dir)
        .env("MAVROS_ORACLE_NAME", &job.name)
        .env(
            "MAVROS_ORACLE_EXPECT_FAILURE",
            if job.expect_failure { "1" } else { "0" },
        )
        .env("MAVROS_ORACLE_FIELD", field.name())
        .env(
            "MAVROS_ORACLE_GENERIC_BUILTINS",
            if generic_builtins { "1" } else { "0" },
        )
        // Mavros's project loader picks its stdlib replacements by this field.
        .env("MAVROS_NOIR_FIELD", field.name())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn child");
    let stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .find_map(|l| {
                // libtest writes `test <name> ... ` on the line the record starts on.
                l.find(RECORD_PREFIX)
                    .map(|at| l[at + RECORD_PREFIX.len()..].to_string())
            })
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait on child") {
            break Some(status);
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let line = reader.join().ok().flatten();
    match (line, status) {
        (Some(json), _) => serde_json::from_str(&json).expect("record parses"),
        (None, status) => {
            let mut record = Record::new(job.name.clone(), job.expect_failure);
            record.verdict = match status {
                None => format!("timeout after {}s", timeout.as_secs()),
                Some(status) => format!("crash: {status}"),
            };
            record.mavros_compile = record.verdict.clone();
            record
        }
    }
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// Sweep `$MAVROS_ORACLE_CORPUS` (a copy of a `test_programs` directory) through the interpreter
/// and Mavros, write `target/mavros-oracle/<field>[-generic-builtins].{jsonl,md}`, and print the
/// verdict totals.
#[test]
#[ignore = "differential oracle: interpreter vs Mavros over the corpus"]
fn sweep() {
    let corpus = PathBuf::from(
        std::env::var_os("MAVROS_ORACLE_CORPUS")
            .expect("MAVROS_ORACLE_CORPUS names a copy of a test_programs directory"),
    );
    let field = sweep_field();
    let generic_builtins = sweep_generic_builtins();
    let parallel: usize = std::env::var("MAVROS_ORACLE_JOBS")
        .ok()
        .and_then(|j| j.parse().ok())
        .unwrap_or(8);
    let timeout = Duration::from_secs(
        std::env::var("MAVROS_ORACLE_TIMEOUT_SECS")
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or(600),
    );
    let jobs = jobs(&corpus);
    assert!(!jobs.is_empty(), "no programs under {}", corpus.display());
    eprintln!(
        "{} programs from {} under {}, {parallel} at a time",
        jobs.len(),
        corpus.display(),
        sweep_output_stem(field, generic_builtins)
    );

    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let records: Vec<Mutex<Option<Record>>> = jobs.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..parallel {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(job) = jobs.get(i) else { break };
                    let record = run_job(job, field, generic_builtins, timeout);
                    let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                    eprintln!("[{n}/{}] {} -> {}", jobs.len(), job.name, record.verdict);
                    *records[i].lock().unwrap() = Some(record);
                }
            });
        }
    });
    let records: Vec<Record> = records
        .into_iter()
        .map(|r| r.into_inner().unwrap().expect("every job reports"))
        .collect();

    let out_dir = crate_dir().join("target/mavros-oracle");
    std::fs::create_dir_all(&out_dir).unwrap();
    let stem = sweep_output_stem(field, generic_builtins);
    let mut jsonl = std::fs::File::create(out_dir.join(format!("{stem}.jsonl"))).unwrap();
    for record in &records {
        writeln!(jsonl, "{}", serde_json::to_string(record).unwrap()).unwrap();
    }

    let mut totals: BTreeMap<(bool, String), usize> = BTreeMap::new();
    for record in &records {
        let bucket = record
            .verdict
            .split(':')
            .next()
            .unwrap_or_default()
            .to_string();
        *totals.entry((record.expect_failure, bucket)).or_default() += 1;
    }
    let mut md = format!(
        "# Interpreter vs Mavros under {stem}\n\n| Corpus | Verdict | Programs |\n| --- | --- | --- |\n"
    );
    for ((expect_failure, verdict), count) in &totals {
        let corpus = if *expect_failure {
            "execution_failure"
        } else {
            "execution_success"
        };
        md.push_str(&format!("| {corpus} | {verdict} | {count} |\n"));
    }
    md.push_str("\n| Program | Verdict | Interpreter | Recorded return | Mavros compile | Witgen, return unchecked | Witgen, interpreter's return declared |\n| --- | --- | --- | --- | --- | --- | --- |\n");
    for record in &records {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            cell(&record.name),
            cell(&record.verdict),
            cell(&record.interp),
            cell(&record.recorded_return),
            cell(&record.mavros_compile),
            cell(&record.witgen_unchecked),
            cell(&record.witgen_declared),
        ));
    }
    let report = out_dir.join(format!("{stem}.md"));
    std::fs::write(&report, md).unwrap();

    println!(
        "\n=== interpreter vs Mavros over {} programs under {stem} ===",
        records.len()
    );
    for ((expect_failure, verdict), count) in &totals {
        let corpus = if *expect_failure {
            "failure"
        } else {
            "success"
        };
        println!("  {count:4}  {corpus:7}  {verdict}");
    }
    println!("  report: {}", report.display());
}

/// `input_from_value` is the inverse of the interpreter's input bridge: every shape a return
/// can take reads back as the value it declared, struct fields in declaration order.
#[test]
fn a_declared_return_reads_back_through_the_input_bridge() {
    use super::IntValue;
    use super::input::value_from_input;
    use acvm::FieldValue;
    use noirc_abi::Sign;
    use noirc_frontend::monomorphization::ast::Type;
    use noirc_frontend::shared::Signedness;
    use num_bigint::BigInt;
    use std::rc::Rc;

    let field = FieldId::Bn254;
    let config = FieldConfig::new(field);
    let u8_value = |v: u8| Value::Int(IntValue::canonical(false, 8, BigInt::from(v)));
    let u8_abi = AbiType::Integer {
        sign: Sign::Unsigned,
        width: 8,
    };
    let u8_type = Type::Integer(Signedness::Unsigned, 8);
    let cases: Vec<(Value, AbiType, Type)> = vec![
        (
            Value::Field(-FieldValue::one(field)),
            AbiType::Field,
            Type::Field,
        ),
        (
            Value::Int(IntValue::canonical(true, 8, BigInt::from(-5))),
            AbiType::Integer {
                sign: Sign::Signed,
                width: 8,
            },
            Type::Integer(Signedness::Signed, 8),
        ),
        (Value::Bool(true), AbiType::Boolean, Type::Bool),
        (
            Value::Array(vec![u8_value(1), u8_value(2)]),
            AbiType::Array {
                length: 2,
                typ: Box::new(u8_abi.clone()),
            },
            Type::Array(2, Rc::new(u8_type.clone())),
        ),
        (
            Value::tuple(vec![Value::Bool(false), u8_value(9)]),
            AbiType::Tuple {
                fields: vec![AbiType::Boolean, u8_abi.clone()],
            },
            Type::Tuple(vec![Type::Bool, u8_type.clone()]),
        ),
        // Declaration order `y, x`, which the ABI keeps and a name-keyed map would sort.
        (
            Value::tuple(vec![Value::Field(FieldValue::one(field)), u8_value(3)]),
            AbiType::Struct {
                path: "Point".into(),
                fields: vec![("y".into(), AbiType::Field), ("x".into(), u8_abi.clone())],
            },
            Type::Tuple(vec![Type::Field, u8_type.clone()]),
        ),
        (
            Value::Str(b"hi".to_vec()),
            AbiType::String { length: 2 },
            Type::String(2),
        ),
    ];
    for (value, abi, typ) in cases {
        let declared =
            input_from_value(&value, &abi, config).unwrap_or_else(|e| panic!("{value:?}: {e}"));
        let back = value_from_input(&declared, &abi, &typ, field)
            .unwrap_or_else(|e| panic!("{value:?}: {e}"));
        assert_eq!(back, value, "{abi:?}");
    }
    let short = Value::Array(vec![u8_value(1)]);
    let two = AbiType::Array {
        length: 2,
        typ: Box::new(u8_abi),
    };
    assert!(input_from_value(&short, &two, config).is_err());
}

/// Every verdict, from the record that earns it.
#[test]
fn every_verdict_bucket_is_reached_from_its_record() {
    let record =
        |expect_failure: bool, interp: &str, compile: &str, unchecked: &str, declared: &str| {
            Record {
                interp: interp.into(),
                mavros_compile: compile.into(),
                witgen_unchecked: unchecked.into(),
                witgen_declared: declared.into(),
                ..Record::new("program".into(), expect_failure)
            }
        };
    let gap = "Unsupported { construct: \"x\" }: x";
    for (expected, expect_failure, interp, compile, unchecked, declared) in [
        ("agree", false, "ok", "ok", "ok", "ok"),
        ("agree", false, "ok", "ok", "ok", "no-return"),
        (
            "RETURN-MISMATCH",
            false,
            "ok",
            "ok",
            "ok",
            "guard: return differs",
        ),
        (
            "unencodable-return",
            false,
            "ok",
            "ok",
            "ok",
            "unencodable: x",
        ),
        (
            "MAVROS-REJECTS",
            false,
            "ok",
            "reject: unsat",
            "not-run",
            "not-run",
        ),
        ("MAVROS-REJECTS", false, "ok", "ok", "trap: x", "not-run"),
        (
            "INTERP-REJECTS",
            false,
            "AssertionFailed: x",
            "ok",
            "ok",
            "not-run",
        ),
        (
            "both-reject",
            false,
            "AssertionFailed: x",
            "reject: unsat",
            "not-run",
            "not-run",
        ),
        ("interp-gap", false, gap, "ok", "ok", "not-run"),
        (
            "interp-internal",
            false,
            "Internal: x",
            "ok",
            "ok",
            "not-run",
        ),
        ("interp-internal", false, "Panic: x", "ok", "ok", "not-run"),
        ("mavros-gap", false, "ok", "panic: x", "not-run", "not-run"),
        (
            "mavros-gap",
            false,
            "ok",
            "lowering-fail: x",
            "not-run",
            "not-run",
        ),
        (
            "mavros-gap",
            false,
            "ok",
            "no program",
            "not-run",
            "not-run",
        ),
        ("mavros-gap", false, "ok", "ok", "panic: x", "not-run"),
        (
            "mavros-gap",
            false,
            "AssertionFailed: x",
            "lowering-fail: x",
            "not-run",
            "not-run",
        ),
        (
            "both-gap",
            false,
            gap,
            "lowering-panic: x",
            "not-run",
            "not-run",
        ),
        (
            "both-gap",
            false,
            "Internal: x",
            "panic: x",
            "not-run",
            "not-run",
        ),
        (
            "frontend-reject",
            false,
            "not-run",
            "frontend-reject: x",
            "not-run",
            "not-run",
        ),
        (
            "agree-reject",
            true,
            "AssertionFailed: x",
            "reject: unsat",
            "not-run",
            "not-run",
        ),
        (
            "agree-reject",
            true,
            "AssertionFailed: x",
            "ok",
            "trap: x",
            "not-run",
        ),
        (
            "mavros-gap",
            true,
            "AssertionFailed: x",
            "ok",
            "panic: x",
            "not-run",
        ),
        (
            "MAVROS-ACCEPTS",
            true,
            "AssertionFailed: x",
            "ok",
            "ok",
            "not-run",
        ),
        ("INTERP-ACCEPTS", true, "ok", "ok", "unsat", "ok"),
        ("both-accept", true, "ok", "ok", "ok", "ok"),
        ("interp-gap", true, gap, "ok", "ok", "not-run"),
    ] {
        let record = record(expect_failure, interp, compile, unchecked, declared);
        assert_eq!(verdict(&record), expected, "{record:?}");
    }
}

/// The interpreter's return for the package at `root`, through the pure-Noir frontend.
fn pure_noir_value(root: &Path, field: FieldId, generic_builtins: bool) -> Value {
    use super::loader::NoirProject;
    use super::validation_frontend::compile_for_validation_with;

    let toml = std::fs::read_to_string(root.join("Prover.toml")).unwrap_or_default();
    let noir = NoirProject::new(root.to_path_buf()).unwrap();
    let validated = compile_for_validation_with(&noir, field, generic_builtins).unwrap();
    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .unwrap();
    interpret_with_inputs(&validated.program, inputs, validated.field_id).unwrap()
}

/// A copy of the fixture `name` in a fresh directory, since Mavros writes into the package.
fn fixture_copy(tmp: &tempfile::TempDir, name: &str) -> PathBuf {
    let root = tmp.path().join("pkg");
    super::corpus::copy_dir(&crate_dir().join("fixtures").join(name), &root);
    root
}

/// Mavros's driver and the pure-Noir frontend lower a stdlib-free fixture to programs that
/// interpret the same, and the record says so.
#[test]
fn mavros_program_interprets_like_the_pure_noir_one() {
    let tmp = tempfile::tempdir().unwrap();
    let root = fixture_copy(&tmp, "interp_inputs_u64");
    let expected = pure_noir_value(&root, FieldId::Bn254, false);

    let record = examine(
        &root,
        "interp_inputs_u64".into(),
        false,
        FieldId::Bn254,
        false,
    );
    assert_eq!(record.interp, "ok", "{record:?}");
    assert_eq!(record.interp_value, Some(line(format!("{expected:?}"))));
    assert_eq!(verdict(&record), "agree", "{record:?}");
}

/// The benchmark mode reaches Mavros's compile as well as the interpreter's: both hash a `u64`
/// as two limbs and agree, while the plain mode hashes it in one write.
#[test]
fn the_benchmark_mode_reaches_mavros() {
    let tmp = tempfile::tempdir().unwrap();
    let root = fixture_copy(&tmp, "interp_generic_twins");
    let limbs = pure_noir_value(&root, FieldId::Bn254, true);
    assert_ne!(limbs, pure_noir_value(&root, FieldId::Bn254, false));

    let record = examine(
        &root,
        "interp_generic_twins".into(),
        false,
        FieldId::Bn254,
        true,
    );
    assert_eq!(
        record.interp_value,
        Some(line(format!("{limbs:?}"))),
        "{record:?}"
    );
    assert_eq!(verdict(&record), "agree", "{record:?}");
}

/// A run-time failure is an agreed rejection only when the corpus expects one.
#[test]
fn a_failing_program_is_agreed_on_only_in_the_failure_corpus() {
    let tmp = tempfile::tempdir().unwrap();
    let root = fixture_copy(&tmp, "neg_assert_fail");
    let as_failure = examine(&root, "neg_assert_fail".into(), true, FieldId::Bn254, false);
    assert!(
        as_failure.interp.starts_with("AssertionFailed"),
        "{as_failure:?}"
    );
    assert_eq!(verdict(&as_failure), "agree-reject", "{as_failure:?}");

    let as_success = examine(
        &root,
        "neg_assert_fail".into(),
        false,
        FieldId::Bn254,
        false,
    );
    assert_eq!(verdict(&as_success), "both-reject", "{as_success:?}");
}

/// A child past its budget is killed and recorded as a timeout, not as any agreement.
#[test]
fn a_child_past_its_budget_is_a_timeout_record() {
    let tmp = tempfile::tempdir().unwrap();
    let job = Job {
        dir: fixture_copy(&tmp, "interp_basic"),
        name: "interp_basic".into(),
        expect_failure: false,
    };
    let record = run_job(&job, FieldId::Bn254, false, Duration::ZERO);
    assert_eq!(record.verdict, "timeout after 0s");
    assert_eq!(record.mavros_compile, record.verdict);
    assert_eq!(record.interp, "not-run");
}

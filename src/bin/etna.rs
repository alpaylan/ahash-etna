// ETNA workload runner for ahash.
//
// Usage: cargo run --release --bin etna -- <tool> <property>
//   tool:     etna | proptest | quickcheck | crabcheck | hegel
//   property: NullPaddingDistinct | All
//
// Each invocation prints exactly one JSON line on stdout:
//   {status, tests, discards, time, counterexample, error, tool, property}
// Exit status is always 0 except for argument parse errors (exit 2).

use ahash::etna::{property_null_padding_distinct, PropertyResult};
use crabcheck::quickcheck as crabcheck_qc;
use hegel::{generators as hgen, HealthCheck, Hegel, Settings as HegelSettings, TestCase};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestCaseError, TestRunner};
use quickcheck::{Arbitrary as QcArbitrary, Gen, QuickCheck, ResultStatus, TestResult};
use std::fmt;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Default, Clone, Copy)]
struct Metrics {
    inputs: u64,
    elapsed_us: u128,
}

impl Metrics {
    fn combine(self, other: Metrics) -> Metrics {
        Metrics {
            inputs: self.inputs + other.inputs,
            elapsed_us: self.elapsed_us + other.elapsed_us,
        }
    }
}

type Outcome = (Result<(), String>, Metrics);

fn to_err(r: PropertyResult) -> Result<(), String> {
    match r {
        PropertyResult::Pass | PropertyResult::Discard => Ok(()),
        PropertyResult::Fail(m) => Err(m),
    }
}

const ALL_PROPERTIES: &[&str] = &["NullPaddingDistinct"];

fn run_all<F: FnMut(&str) -> Outcome>(mut f: F) -> Outcome {
    let mut total = Metrics::default();
    let mut final_status: Result<(), String> = Ok(());
    for p in ALL_PROPERTIES {
        let (r, m) = f(p);
        total = total.combine(m);
        if r.is_err() && final_status.is_ok() {
            final_status = r;
        }
    }
    (final_status, total)
}

// ============================================================================
// Input wrappers
// ============================================================================

#[derive(Clone, Copy)]
struct LenPair {
    n: u8,
    m: u8,
}

impl fmt::Debug for LenPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.n, self.m)
    }
}
impl fmt::Display for LenPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

// Cap input lengths to keep the property cheap (and to keep collision search
// concentrated around the read_small / large_update boundaries where the
// historical bug was observable).
const MAX_LEN: u8 = 64;

// ============================================================================
// etna (deterministic witness-shaped inputs)
// ============================================================================

fn run_etna_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_etna_property);
    }
    let t0 = Instant::now();
    let result = match property {
        // Canonical witness from `tests/etna_witnesses.rs`.
        "NullPaddingDistinct" => to_err(property_null_padding_distinct((0, 1))),
        _ => {
            return (
                Err(format!("Unknown property for etna: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    (result, Metrics { inputs: 1, elapsed_us })
}

// ============================================================================
// proptest
// ============================================================================

fn len_pair_strategy() -> BoxedStrategy<LenPair> {
    (0u8..=MAX_LEN, 0u8..=MAX_LEN)
        .prop_map(|(n, m)| LenPair { n, m })
        .boxed()
}

fn run_proptest_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_proptest_property);
    }
    let counter = Arc::new(AtomicU64::new(0));
    let t0 = Instant::now();
    let mut runner = TestRunner::new(ProptestConfig::default());
    let c = counter.clone();
    let result: Result<(), String> = match property {
        "NullPaddingDistinct" => runner
            .run(&len_pair_strategy(), move |args| {
                c.fetch_add(1, Ordering::Relaxed);
                let cex = format!("({:?})", args);
                let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    property_null_padding_distinct((args.n, args.m))
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => Err(TestCaseError::fail(cex)),
                }
            })
            .map_err(|e| match e {
                proptest::test_runner::TestError::Fail(r, _) => r.to_string(),
                other => other.to_string(),
            }),
        _ => {
            return (
                Err(format!("Unknown property for proptest: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = counter.load(Ordering::Relaxed);
    (result, Metrics { inputs, elapsed_us })
}

// ============================================================================
// quickcheck (forked, fn-pointer based)
// ============================================================================

impl QcArbitrary for LenPair {
    fn arbitrary(g: &mut Gen) -> Self {
        let n = (<u8 as QcArbitrary>::arbitrary(g)) % (MAX_LEN + 1);
        let m = (<u8 as QcArbitrary>::arbitrary(g)) % (MAX_LEN + 1);
        LenPair { n, m }
    }
}

static QC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn qc_run<F>(prop: F) -> TestResult
where
    F: FnOnce() -> PropertyResult + std::panic::UnwindSafe,
{
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let res = std::panic::catch_unwind(prop);
    match res {
        Ok(PropertyResult::Pass) => TestResult::passed(),
        Ok(PropertyResult::Discard) => TestResult::discard(),
        Ok(PropertyResult::Fail(_)) | Err(_) => TestResult::failed(),
    }
}

fn qc_null_padding_distinct(args: LenPair) -> TestResult {
    qc_run(move || property_null_padding_distinct((args.n, args.m)))
}

fn run_quickcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_quickcheck_property);
    }
    QC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let result = match property {
        "NullPaddingDistinct" => QuickCheck::new()
            .tests(200)
            .max_tests(2000)
            .max_time(Duration::from_secs(86_400))
            .quicktest(qc_null_padding_distinct as fn(LenPair) -> TestResult),
        _ => {
            return (
                Err(format!("Unknown property for quickcheck: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = QC_COUNTER.load(Ordering::Relaxed);
    let metrics = Metrics { inputs, elapsed_us };
    let status = match result.status {
        ResultStatus::Finished => Ok(()),
        ResultStatus::Failed { arguments } => Err(format!("({})", arguments.join(" "))),
        ResultStatus::Aborted { err } => Err(format!("aborted: {err:?}")),
        ResultStatus::TimedOut => Err("timed out".to_string()),
        ResultStatus::GaveUp => Err(format!(
            "gave up: passed={}, discarded={}",
            result.n_tests_passed, result.n_tests_discarded
        )),
    };
    (status, metrics)
}

// ============================================================================
// crabcheck
// ============================================================================

use crabcheck::quickcheck::Arbitrary as CcArbitrary;
use rand9::Rng as CcRng;

impl<R: CcRng> CcArbitrary<R> for LenPair {
    fn generate(rng: &mut R, _n: usize) -> Self {
        let n = (rng.random::<u8>()) % (MAX_LEN + 1);
        let m = (rng.random::<u8>()) % (MAX_LEN + 1);
        LenPair { n, m }
    }
}

static CC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn cc_null_padding_distinct(v: LenPair) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_null_padding_distinct((v.n, v.m)) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn run_crabcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_crabcheck_property);
    }
    CC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let cfg = crabcheck_qc::Config { tests: 200 };
    let result = match property {
        "NullPaddingDistinct" => crabcheck_qc::quickcheck_with_config(
            cfg,
            cc_null_padding_distinct as fn(LenPair) -> Option<bool>,
        ),
        _ => {
            return (
                Err(format!("Unknown property for crabcheck: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = CC_COUNTER.load(Ordering::Relaxed);
    let metrics = Metrics { inputs, elapsed_us };
    let status = match result.status {
        crabcheck_qc::ResultStatus::Finished => Ok(()),
        crabcheck_qc::ResultStatus::Failed { arguments } => {
            Err(format!("({})", arguments.join(" ")))
        }
        crabcheck_qc::ResultStatus::TimedOut => Err("timed out".to_string()),
        crabcheck_qc::ResultStatus::GaveUp => Err(format!(
            "gave up: passed={}, discarded={}",
            result.passed, result.discarded
        )),
        crabcheck_qc::ResultStatus::Aborted { error } => Err(format!("aborted: {error}")),
    };
    (status, metrics)
}

// ============================================================================
// hegel
// ============================================================================

static HG_COUNTER: AtomicU64 = AtomicU64::new(0);

fn hegel_settings() -> HegelSettings {
    HegelSettings::new()
        .test_cases(200)
        .suppress_health_check(HealthCheck::all())
}

fn hg_draw_len(tc: &TestCase) -> u8 {
    tc.draw(hgen::integers::<u32>().min_value(0).max_value(MAX_LEN as u32)) as u8
}

fn run_hegel_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_hegel_property);
    }
    HG_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let settings = hegel_settings();
    let run_result = std::panic::catch_unwind(AssertUnwindSafe(|| match property {
        "NullPaddingDistinct" => {
            Hegel::new(|tc: TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let n = hg_draw_len(&tc);
                let m = hg_draw_len(&tc);
                let cex = format!("({} {})", n, m);
                let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    property_null_padding_distinct((n, m))
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("{cex}"),
                }
            })
            .settings(settings.clone())
            .run();
        }
        _ => panic!("__unknown_property:{property}"),
    }));
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = HG_COUNTER.load(Ordering::Relaxed);
    let metrics = Metrics { inputs, elapsed_us };
    let status = match run_result {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "hegel panicked with non-string payload".to_string()
            };
            if let Some(rest) = msg.strip_prefix("__unknown_property:") {
                return (
                    Err(format!("Unknown property for hegel: {rest}")),
                    Metrics::default(),
                );
            }
            Err(msg
                .strip_prefix("Property test failed: ")
                .unwrap_or(&msg)
                .to_string())
        }
    };
    (status, metrics)
}

// ============================================================================
// dispatch + main
// ============================================================================

fn run(tool: &str, property: &str) -> Outcome {
    match tool {
        "etna" => run_etna_property(property),
        "proptest" => run_proptest_property(property),
        "quickcheck" => run_quickcheck_property(property),
        "crabcheck" => run_crabcheck_property(property),
        "hegel" => run_hegel_property(property),
        _ => (Err(format!("Unknown tool: {tool}")), Metrics::default()),
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn emit_json(
    tool: &str,
    property: &str,
    status: &str,
    metrics: Metrics,
    counterexample: Option<&str>,
    error: Option<&str>,
) {
    let cex = counterexample.map_or("null".to_string(), json_str);
    let err = error.map_or("null".to_string(), json_str);
    println!(
        "{{\"status\":{},\"tests\":{},\"discards\":0,\"time\":{},\"counterexample\":{},\"error\":{},\"tool\":{},\"property\":{}}}",
        json_str(status),
        metrics.inputs,
        json_str(&format!("{}us", metrics.elapsed_us)),
        cex,
        err,
        json_str(tool),
        json_str(property),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <tool> <property>", args[0]);
        eprintln!("Tools: etna | proptest | quickcheck | crabcheck | hegel");
        eprintln!("Properties: NullPaddingDistinct | All");
        std::process::exit(2);
    }
    let (tool, property) = (args[1].as_str(), args[2].as_str());

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let caught = std::panic::catch_unwind(AssertUnwindSafe(|| run(tool, property)));
    std::panic::set_hook(previous_hook);

    let (result, metrics) = match caught {
        Ok(outcome) => outcome,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = payload.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "panic with non-string payload".to_string()
            };
            emit_json(
                tool,
                property,
                "aborted",
                Metrics::default(),
                None,
                Some(&format!("adapter panic: {msg}")),
            );
            return;
        }
    };

    match result {
        Ok(()) => emit_json(tool, property, "passed", metrics, None, None),
        Err(msg) => emit_json(tool, property, "failed", metrics, Some(&msg), None),
    }
}

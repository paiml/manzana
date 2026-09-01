//! Binds `#[provable_contracts_macros::contract(..)]` attributes to real
//! contract files.
//!
//! # Why this exists
//!
//! `#[contract("name", equation = "eq")]` expands to
//! `const _: Option<&str> = option_env!("CONTRACT_NAME_EQ")`. The macro's own
//! documentation says "Missing env var = compile error", but the code uses
//! `option_env!`, not `env!` -- so with no build script the constant is
//! `None`, the attribute is `#[allow(dead_code)]`, and it proves **nothing**.
//!
//! manzana shipped `#[contract("manzana-tensor-v1", equation = "new")]` on
//! `Tensor::new` while `manzana-tensor-v1` existed nowhere on disk and
//! `binding.yaml` declared `bindings: []`. The annotation was decorative: a
//! claim of verification with no verification behind it, which is the same
//! defect class as the incident this branch exists to fix.
//!
//! This script closes it. Every `#[contract(..)]` in `src/` must name a
//! contract file that exists and an equation that file defines, or the build
//! FAILS. Binding is then exported so the constant resolves to `Some(..)`.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::exit;

/// Fail the build with a diagnostic.
///
/// `eprintln!` + `exit(1)` rather than `panic!`: this crate sets
/// `panic = "deny"`, and a build script has a legitimate reason to stop the
/// build without earning an exemption from that lint.
fn fail(message: &str) -> ! {
    eprintln!("\nbuild.rs: {message}\n");
    exit(1);
}

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=contracts");

    let contracts_dir = Path::new("contracts");
    let mut bound = BTreeSet::new();

    // Collect (contract, equation) pairs actually referenced in the source.
    let mut referenced: Vec<(String, String, String)> = Vec::new();
    collect_refs(Path::new("src"), &mut referenced);

    for (contract, equation, origin) in &referenced {
        let path = contracts_dir.join(format!("{contract}.yaml"));
        let Ok(text) = fs::read_to_string(&path) else {
            fail(&format!(
                "contract binding failure in {origin}: \
                 #[contract(\"{contract}\", equation = \"{equation}\")] names {}, \
                 which does not exist. An annotation pointing at a missing \
                 contract asserts a verification that cannot happen.",
                path.display()
            ));
        };

        // The equation must be declared under `equations:` in that file.
        if !declares_equation(&text, equation) {
            fail(&format!(
                "contract binding failure in {origin}: contract \"{contract}\" \
                 exists but declares no equation \"{equation}\". The binding \
                 would compile and verify nothing."
            ));
        }

        let key = env_key(contract, equation);
        println!("cargo:rustc-env={key}=bound");
        bound.insert(key);
    }

    println!(
        "cargo:warning=provable-contracts: {} binding(s) resolved",
        bound.len()
    );
}

/// `manzana-tensor-v1` + `new` -> `CONTRACT_MANZANA_TENSOR_V1_NEW`
fn env_key(contract: &str, equation: &str) -> String {
    let norm = |s: &str| {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect::<String>()
    };
    format!("CONTRACT_{}_{}", norm(contract), norm(equation))
}

fn declares_equation(yaml: &str, equation: &str) -> bool {
    let mut in_equations = false;
    for line in yaml.lines() {
        if line.starts_with("equations:") {
            in_equations = true;
            continue;
        }
        // Any other top-level key ends the block.
        if in_equations && !line.starts_with(char::is_whitespace) && !line.trim().is_empty() {
            in_equations = false;
        }
        if in_equations {
            let t = line.trim_end();
            if t.starts_with("  ") && !t.starts_with("    ") && t.trim_start().starts_with(equation)
            {
                let after = t.trim_start().trim_start_matches(equation);
                if after.starts_with(':') {
                    return true;
                }
            }
        }
    }
    false
}

fn collect_refs(dir: &Path, out: &mut Vec<(String, String, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_refs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            for line in text.lines() {
                let Some(rest) = line.split_once("contract(").map(|(_, r)| r) else {
                    continue;
                };
                if !line.contains("equation") {
                    continue;
                }
                // `"name", equation = "eq")]` splits on '"' to
                // ["", name, ", equation = ", eq, ")]"]; skip(1) drops the
                // leading empty, next() takes `name`, and the equation is then
                // the NEXT-but-one element -- nth(1), not nth(2).
                let mut parts = rest.split('"').skip(1);
                let (Some(contract), Some(equation)) = (parts.next(), parts.nth(1)) else {
                    continue;
                };
                out.push((
                    contract.to_string(),
                    equation.to_string(),
                    path.display().to_string(),
                ));
            }
        }
    }
}

use std::{hint::black_box, time::Instant};

use git_toolbox::github::codeowners::CodeOwners;

fn main() {
    let data = fixture(5_000);
    let codeowners = CodeOwners::<()>::try_from_bufread(data.as_bytes()).unwrap();
    let paths = [
        "apps/service-1/src/lib.rs",
        "apps/service-20/src/main.rs",
        "docs/guide/setup.md",
        "scripts/release.sh",
        "deep/nested/path/file.txt",
    ];

    bench("parse_5000_rules", 20, || {
        black_box(CodeOwners::<()>::try_from_bufread(data.as_bytes()).unwrap());
    });
    bench("find_owners_5000_rules", 100_000, || {
        for path in paths {
            black_box(codeowners.find_owners(black_box(path)));
        }
    });
    bench("debug_5000_rules", 20_000, || {
        for path in paths {
            black_box(codeowners.debug(black_box(path)).count());
        }
    });
}

fn bench(name: &str, iterations: usize, mut f: impl FnMut()) {
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();
    let per_iter = elapsed / iterations as u32;
    println!("{name}: total={elapsed:?} iter={iterations} per_iter={per_iter:?}");
}

fn fixture(rule_count: usize) -> String {
    let mut out = String::with_capacity(rule_count * 32);
    for i in 0..rule_count {
        out.push_str(&format!("apps/service-{i}/ @team-{i}\n"));
    }
    out.push_str("docs/** @docs-team\n");
    out.push_str("*.rs @rust-team\n");
    out.push_str("* @fallback\n");
    out
}

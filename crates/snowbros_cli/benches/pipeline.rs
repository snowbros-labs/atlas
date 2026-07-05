//! Cold vs warm pipeline benchmarks on a generated synthetic project.
//!
//! Run with `cargo bench -p snowbros-atlas`.

// criterion macros generate undocumented items.
#![allow(missing_docs)]

use std::fs;
use std::path::Path;

use camino::Utf8PathBuf;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use snowbros_engine::pipeline;

/// Generates a synthetic TS project: `n` files in an import chain plus
/// package imports — representative of parse + resolve + graph load.
fn generate_project(root: &Path, n: usize) {
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{ "dependencies": { "react": "^19.0.0" } }"#,
    )
    .unwrap();
    for i in 0..n {
        let import = if i + 1 < n {
            format!("import {{ f{} }} from \"./m{}\";", i + 1, i + 1)
        } else {
            String::new()
        };
        // ~200 lines per file — closer to real-world module size.
        let body = format!(
            "import React from \"react\";\n{import}\n\
             export function f{i}(): number {{\n\
             {}\n  return {i};\n}}\n",
            (0..200)
                .map(|k| format!("  const x{k}: number = {k} * 2 + 1;\n"))
                .collect::<String>()
        );
        fs::write(src.join(format!("m{i}.ts")), body).unwrap();
    }
}

/// Benchmarks cold (no cache) vs warm (primed cache) pipeline runs.
fn bench_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline");
    group.sample_size(20);

    for n in [50usize, 200] {
        let dir = tempfile::tempdir().unwrap();
        generate_project(dir.path(), n);
        let root = Utf8PathBuf::from(dir.path().to_str().unwrap());

        group.bench_with_input(BenchmarkId::new("cold", n), &root, |b, root| {
            b.iter(|| pipeline::build(root, false).unwrap());
        });

        // Prime the cache once, then measure warm runs.
        pipeline::build(&root, true).unwrap();
        group.bench_with_input(BenchmarkId::new("warm", n), &root, |b, root| {
            b.iter(|| pipeline::build(root, true).unwrap());
        });
    }
    group.finish();
}

criterion_group!(benches, bench_pipeline);
criterion_main!(benches);

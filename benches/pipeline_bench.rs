use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use dowsing_core::candidates::generate_candidates;
use dowsing_core::extraction::extract_functions;
use dowsing_core::fingerprint::generate_fingerprints;
use dowsing_core::normalization::normalize_function;
use dowsing_core::parser::parse_python_source;
use dowsing_core::types::NormalizationLevel;
use rustpython_parser::ast::Stmt;
use rustpython_parser::source_code::RandomLocator;
use std::path::PathBuf;

/// Generate synthetic Python source with N functions.
fn generate_synthetic(n: usize) -> String {
    let mut src = String::new();
    for i in 0..n {
        // Alternate between a few templates to create clusters
        let template = i % 5;
        match template {
            0 => {
                src.push_str(&format!(
                    "def func_{i}(x, y):\n    result = validate(x)\n    data = process(result, y)\n    return save(data)\n\n"
                ));
            }
            1 => {
                src.push_str(&format!(
                    "def func_{i}(items):\n    output = []\n    for item in items:\n        if item > 0:\n            output.append(transform(item))\n    return output\n\n"
                ));
            }
            2 => {
                src.push_str(&format!(
                    "def func_{i}(a, b, c):\n    if a > b:\n        return a + c\n    elif b > c:\n        return b + a\n    else:\n        return c + b\n\n"
                ));
            }
            3 => {
                src.push_str(&format!(
                    "def func_{i}(data):\n    try:\n        parsed = parse(data)\n        validated = check(parsed)\n        return format_output(validated)\n    except ValueError:\n        return None\n\n"
                ));
            }
            _ => {
                src.push_str(&format!(
                    "def func_{i}(value):\n    cleaned = value.strip()\n    normalized = cleaned.lower()\n    return normalized\n\n"
                ));
            }
        }
    }
    src
}

fn line_for_offset(source: &str, offset: rustpython_parser::text_size::TextSize) -> usize {
    let mut locator = RandomLocator::new(source);
    locator.locate(offset).row.to_usize()
}

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");

    for n in [100, 500] {
        let source = generate_synthetic(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &source, |b, src| {
            let path = PathBuf::from("bench.py");
            b.iter(|| {
                let result = parse_python_source(black_box(src), &path).unwrap();
                black_box(result.module);
            });
        });
    }

    group.finish();
}

fn bench_extract(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract");

    for n in [100, 500] {
        let source = generate_synthetic(n);
        let path = PathBuf::from("bench.py");
        let result = parse_python_source(&source, &path).unwrap();
        let body = result.module.unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(n), &body, |b, body| {
            b.iter(|| {
                let funcs = extract_functions(black_box(body), &source, &path);
                black_box(funcs);
            });
        });
    }

    group.finish();
}

fn bench_normalize_and_fingerprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("normalize_fingerprint");

    for n in [100, 500] {
        let source = generate_synthetic(n);
        let path = PathBuf::from("bench.py");
        let result = parse_python_source(&source, &path).unwrap();
        let body = result.module.unwrap();
        let funcs = extract_functions(&body, &source, &path);

        group.bench_with_input(
            BenchmarkId::from_parameter(n),
            &(&body, &funcs),
            |b, (body, funcs)| {
                b.iter(|| {
                    let mut norms = Vec::new();
                    let mut fps = Vec::new();
                    for (i, func_info) in funcs.iter().enumerate() {
                        for stmt in body.iter() {
                            if let Stmt::FunctionDef(f) = stmt {
                                if f.name.as_str() == func_info.function_name
                                    && line_for_offset(&source, f.range.start())
                                        == func_info.start_line
                                {
                                    let params: Vec<String> =
                                        f.args.args.iter().map(|a| a.def.arg.to_string()).collect();
                                    let norm = normalize_function(
                                        &f.body,
                                        &params,
                                        &f.name,
                                        i,
                                        NormalizationLevel::Balanced,
                                        None,
                                    );
                                    let fp = generate_fingerprints(&norm, func_info);
                                    norms.push(norm);
                                    fps.push(fp);
                                    break;
                                }
                            }
                        }
                    }
                    black_box((&norms, &fps));
                });
            },
        );
    }

    group.finish();
}

fn bench_candidates(c: &mut Criterion) {
    let mut group = c.benchmark_group("candidates");

    for n in [100, 500] {
        let source = generate_synthetic(n);
        let path = PathBuf::from("bench.py");
        let result = parse_python_source(&source, &path).unwrap();
        let body = result.module.unwrap();
        let funcs = extract_functions(&body, &source, &path);

        // Build fingerprints
        let mut fps = Vec::new();
        for (i, func_info) in funcs.iter().enumerate() {
            for stmt in body.iter() {
                if let Stmt::FunctionDef(f) = stmt {
                    if f.name.as_str() == func_info.function_name
                        && line_for_offset(&source, f.range.start()) == func_info.start_line
                    {
                        let params: Vec<String> =
                            f.args.args.iter().map(|a| a.def.arg.to_string()).collect();
                        let norm = normalize_function(
                            &f.body,
                            &params,
                            &f.name,
                            i,
                            NormalizationLevel::Balanced,
                            None,
                        );
                        let fp = generate_fingerprints(&norm, func_info);
                        fps.push(fp);
                        break;
                    }
                }
            }
        }

        group.bench_with_input(BenchmarkId::from_parameter(n), &fps, |b, fps| {
            b.iter(|| {
                let candidates = generate_candidates(black_box(fps), 12);
                black_box(candidates);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_parse,
    bench_extract,
    bench_normalize_and_fingerprint,
    bench_candidates,
);
criterion_main!(benches);

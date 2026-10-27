use criterion::{Criterion, criterion_group, criterion_main};
use sporec_parser::{formatter::format_module, lexer::Lexer, parse};
use std::hint::black_box;

fn source(criterion: &mut Criterion) {
    let source = (0..128)
        .map(|n| format!("fn value_{n}(x: I64) -> I64 {{ x + {n} }}\n"))
        .collect::<String>();
    let module = parse(&source).unwrap();
    assert_eq!(module.items.len(), 128);
    let formatted = format_module(&module);
    assert_eq!(format_module(&parse(&formatted).unwrap()), formatted);
    criterion.bench_function("lex 128 functions", |bench| {
        bench.iter(|| Lexer::new(black_box(&source)).tokenize().unwrap());
    });
    criterion.bench_function("parse 128 functions", |bench| {
        bench.iter(|| parse(black_box(&source)).unwrap());
    });
    criterion.bench_function("format 128 functions", |bench| {
        bench.iter(|| format_module(black_box(&module)));
    });
}

criterion_group!(benches, source);
criterion_main!(benches);

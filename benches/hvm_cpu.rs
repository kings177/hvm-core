use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hvmc::ast::*;
use hvmc::*;
use std::fs;
use std::time::Duration;

// Loads file and generate net from hvm-core syntax
fn load_from_core(file: &str, size: usize, replace: Option<(&str, &str)>) -> (run::Book, run::Net) {
  let code = fs::read_to_string(file).unwrap();
  let code = if let Some((from, to)) = replace { code.replace(from, to) } else { code };
  let book = ast::book_to_runtime(&ast::do_parse_book(&code));
  let mut net = run::Net::new(size);
  net.boot(name_to_val("main"));
  (book, net)
}

// Loads file and generate net from hvm-lang syntax
fn load_from_lang(file: &str, size: usize, replace: Option<(&str, &str)>) -> (run::Book, run::Net) {
  let prelude = fs::read_to_string("./benches/programs/prelude.hvm").unwrap();
  let code = prelude + "\n" + &fs::read_to_string(file).unwrap();
  let code = if let Some((from, to)) = replace { code.replace(from, to) } else { code };

  let mut book = hvm_lang::term::parser::parse_definition_book(&code).unwrap();
  let main = book.check_has_main().unwrap();
  let book = hvm_lang::compile_book(&mut book).unwrap();
  let book = ast::book_to_runtime(&book);

  let mut net = run::Net::new(size);
  net.boot(main.to_internal());
  (book, net)
}

fn church_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("church");
  for n in [20] {
    group.throughput(criterion::Throughput::Elements(n));
    let (book, net) = load_from_lang("./benches/programs/church_mul.hvm", 1 << 12, Some(("{n}", &n.to_string())));
    group.bench_with_input(criterion::BenchmarkId::new("multiplication", n), &n, |b, &_n| {
      b.iter_batched(|| net.clone(), |mut net| black_box(net.normal(&book)), criterion::BatchSize::SmallInput);
    });
    // Church exponentiation
    // n=24 uses at most 1107 elements in heap(1 << 12 = 4096)
    let (book, net) = load_from_core("./benches/programs/church_exp.hvmc", 1 << 12, Some(("{n}", &n.to_string())));
    group.bench_with_input(criterion::BenchmarkId::new("exponentiation", n), &n, |b, &_n| {
      b.iter_batched(|| net.clone(), |mut net| black_box(net.normal(&book)), criterion::BatchSize::SmallInput);
    });
  }
}

fn tree_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("tree");
  // Allocates a big tree
  for n in [12] {
    let (book, net) = load_from_core("./benches/programs/alloc_big_tree.hvmc", 16 << n, Some(("{n}", &n.to_string())));
    group.bench_with_input(criterion::BenchmarkId::new("allocation", n), &n, |b, &_n| {
      b.iter_batched(|| net.clone(), |mut net| black_box(net.normal(&book)), criterion::BatchSize::SmallInput);
    });
  }
}

fn binary_counter_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("binary-counter");
  // Decrements a BitString until it is zero
  for n in [12] {
    let (book, net) = load_from_core("./benches/programs/dec_bits.hvmc", 16 << n, Some(("{n}", &n.to_string())));
    group.bench_with_input(criterion::BenchmarkId::new("single", n), &n, |b, &_n| {
      b.iter_batched(|| net.clone(), |mut net| black_box(net.normal(&book)), criterion::BatchSize::SmallInput);
    });
  }
  // Decrements 2^N BitStrings until they reach zero (ex3)
  for n in [6] {
    let (book, net) = load_from_core("./benches/programs/dec_bits_tree.hvmc", 128 << n, Some(("{n}", &n.to_string())));
    group.bench_with_input(criterion::BenchmarkId::new("many", n), &n, |b, &_n| {
      b.iter_batched(|| net.clone(), |mut net| black_box(net.normal(&book)), criterion::BatchSize::SmallInput);
    });
  }
}

fn fusion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("fusion");
  let (book, net) = load_from_lang("./benches/programs/neg_fusion.hvm", 1 << 8, None);
  group.bench_function(criterion::BenchmarkId::new("neg", "256"), |b| {
    b.iter_batched(|| net.clone(), |mut net| black_box(net.normal(&book)), criterion::BatchSize::SmallInput);
  });
}

fn interact_benchmark(c: &mut Criterion) {
  use ast::Tree::*;
  let mut group = c.benchmark_group("interact");
  group.sample_size(1000);

  let cases = [
    ("era-era", (Era, Era)),
    ("era-con", (Era, Ctr { lab: 0, lft: Era.into(), rgt: Era.into() })),
    ("con-con", ((Ctr { lab: 0, lft: Era.into(), rgt: Era.into() }), Ctr { lab: 0, lft: Era.into(), rgt: Era.into() })),
    ("con-dup", ((Ctr { lab: 0, lft: Era.into(), rgt: Era.into() }), Ctr { lab: 2, lft: Era.into(), rgt: Era.into() })),
  ];
  
  for (name, redex) in cases {
    let mut net = run::Net::new(10);
    let book = run::Book::new();
    ast::net_to_runtime(&mut net, &ast::Net { root: Era, rdex: vec![redex] });
    let (rdx_a, rdx_b) = net.rdex[0];
    group.bench_function(name, |b| {
      b.iter_batched(
        || net.clone(),
        |mut net| black_box(net.interact(&book, rdx_a, rdx_b)),
        criterion::BatchSize::SmallInput
      );
    });
  }
}

criterion_group! {
  name = benches;
  config = Criterion::default()
    .measurement_time(Duration::from_millis(1000))
    .warm_up_time(Duration::from_millis(500));
  targets =
    church_benchmark,
    tree_benchmark,
    binary_counter_benchmark,
    fusion_benchmark,
    interact_benchmark,
}
criterion_main!(benches);
//! Test the `hvm64` binary, including its CLI interface.

use std::{
  error::Error,
  fs,
  io::Read,
  process::{Command, ExitStatus, Stdio},
};

use hvm64_ast::{Net, Tree};
use hvm64_host::Host;
use hvm64_runtime as run;
use insta::assert_snapshot;

fn get_arithmetic_program_path() -> String {
  env!("CARGO_MANIFEST_DIR").to_owned() + "/examples/arithmetic.hvm"
}

fn execute_hvm64(args: &[&str]) -> Result<(ExitStatus, String), Box<dyn Error>> {
  // Spawn the command
  let mut child =
    Command::new(env!("CARGO_BIN_EXE_hvm64")).args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

  // Capture the output of the command
  let mut stdout = child.stdout.take().ok_or("Couldn't capture stdout!")?;
  let mut stderr = child.stderr.take().ok_or("Couldn't capture stderr!")?;

  // Wait for the command to finish and get the exit status
  let status = child.wait()?;

  // Read the output
  let mut output = String::new();
  stdout.read_to_string(&mut output)?;
  stderr.read_to_string(&mut output)?;

  // Print the output of the command
  Ok((status, output))
}

#[test]
fn test_cli_reduce() {
  // Test normal-form expressions
  assert_snapshot!(
    execute_hvm64(&["reduce", "-m", "100M", "--", "1"]).unwrap().1,
    @r###"
  1
  "###
  );
  // Test non-normal form expressions
  assert_snapshot!(
    execute_hvm64(&["reduce", "-m", "100M", "--", "a & 3 ~ $([*] $(4 a))"]).unwrap().1,
    @r###"
  12
  "###
  );
  // Test multiple expressions
  assert_snapshot!(
    execute_hvm64(&["reduce", "-m", "100M", "--", "a & 3 ~ $([*] $(4 a))", "a & 64 ~ $([/] $(2 a))"]).unwrap().1,
    @r###"
  12
  32
  "###
  );

  // Test loading file and reducing expression
  let arithmetic_program = get_arithmetic_program_path();

  assert_snapshot!(
    execute_hvm64(&[
      "reduce", "-m", "100M",
      &arithmetic_program,
      "--", "a & @mul ~ (3 (4 a))"
    ]).unwrap().1,
    @r###"
  12
  "###
  );

  assert_snapshot!(
    execute_hvm64(&[
      "reduce", "-m", "100M",
      &arithmetic_program,
      "--", "a & @mul ~ (3 (4 a))", "a & @div ~ (64 (2 a))"
    ]).unwrap().1,
    @r###"
  12
  32
  "###
  )
}

#[test]
fn test_cli_run_with_args() {
  let arithmetic_program = get_arithmetic_program_path();

  // Test simple program running
  assert_snapshot!(
    execute_hvm64(&[
      "run", "-m", "100M",
      &arithmetic_program,
    ]).unwrap().1,
    @r###"
  (#1{$([/] $(a b)) $([%] $(c d))} (#2{a c} {b d}))
  "###
  );

  // Test partial argument passing
  assert_snapshot!(
    execute_hvm64(&[
      "run", "-m", "100M",
      &arithmetic_program,
      "64"
    ]).unwrap().1,
    @r###"
  (#2{$([/64] a) $([%64] b)} {a b})
  "###
  );

  // Test passing all arguments.
  assert_snapshot!(
    execute_hvm64(&[
      "run", "-m", "100M",
      &arithmetic_program,
      "64",
      "3"
    ]).unwrap().1,
    @r###"
  {21 1}
  "###
  );
}

#[test]
fn test_cli_transform() {
  let arithmetic_program = get_arithmetic_program_path();

  // Test simple program running
  assert_snapshot!(
    execute_hvm64(&[
      "transform",
      "-Opre-reduce",
      &arithmetic_program,
    ]).unwrap().1,
    @r###"
  @add = ($([+] $(a b)) (a b))

  @div = ($([/] $(a b)) (a b))

  @main = (#1{$([/] $(a b)) $([%] $(c d))} (#2{a c} {b d}))

  @mod = ($([%] $(a b)) (a b))

  @mul = ($([*] $(a b)) (a b))

  @sub = ($([-] $(a b)) (a b))
  "###
  );

  assert_snapshot!(
    execute_hvm64(&[
      "transform",
      "-Opre-reduce",
      "--pre-reduce-skip", "main",
      &arithmetic_program,
    ]).unwrap().1,
    @r###"
  @add = ($([+] $(a b)) (a b))

  @div = ($([/] $(a b)) (a b))

  @main = (#1{a b} (#2{c d} {e f}))
    & @mod ~ (b (d f))
    & @div ~ (a (c e))

  @mod = ($([%] $(a b)) (a b))

  @mul = ($([*] $(a b)) (a b))

  @sub = ($([-] $(a b)) (a b))
  "###
  );
}

#[test]
fn test_cli_errors() {
  // Test passing all arguments.
  assert_snapshot!(
    execute_hvm64(&[
      "run", "this-file-does-not-exist.hvm"
    ]).unwrap().1,
    @r###"
 Input file "this-file-does-not-exist.hvm" not found
 "###
  );
  assert_snapshot!(
    execute_hvm64(&[
      "reduce", "this-file-does-not-exist.hvm"
    ]).unwrap().1,
    @r###"
 Input file "this-file-does-not-exist.hvm" not found
 "###
  );
}

#[test]
fn test_apply_tree() {
  fn eval_with_args(fun: &str, args: &[&str]) -> Net {
    let area = run::Heap::new_exact(16).unwrap();

    let mut fun: Net = fun.parse().unwrap();
    for arg in args {
      let arg: Tree = arg.parse().unwrap();
      fun.apply_tree(arg)
    }

    let host = Host::default();
    let mut rnet = run::Net::new(&area);
    let root_port = run::Trg::port(run::Port::new_var(rnet.root.addr()));
    host.encode_net(&mut rnet, root_port, &fun);
    rnet.normal();
    host.readback(&rnet)
  }
  assert_snapshot!(
    eval_with_args("(a a)", &["(a a)"]),
    @"(a a)"
  );
  assert_snapshot!(
    eval_with_args("b & (a b) ~ a", &["(a a)"]),
    @"a"
  );
  assert_snapshot!(
    eval_with_args("(z0 z0)", &["(z1 z1)"]),
    @"(a a)"
  );
  assert_snapshot!(
    eval_with_args("(* 1)", &["(a a)"]),
    @"1"
  );
  assert_snapshot!(
    eval_with_args("($([+] $(a b)) (a b))", &["1", "2"]),
    @"3"
  );
  assert_snapshot!(
    eval_with_args("($([*] $(a b)) (a b))", &["2", "3"]),
    @"6"
  );
  assert_snapshot!(
    eval_with_args("($([*] $(a b)) (a b))", &["2"]),
    @"($([*2] a) a)"
  );
}

#[test]
fn test_cli_compile() {
  if !Command::new(env!("CARGO_BIN_EXE_hvm64"))
    .args(["compile", &get_arithmetic_program_path()])
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .spawn()
    .unwrap()
    .wait()
    .unwrap()
    .success()
  {
    panic!("{:?}", "compilation failed");
  };

  let (status, output) = execute_hvm64(&["run", "-i", "examples/arithmetic", "/dev/null", "40", "3"]).unwrap();

  assert_snapshot!(format_args!("{status}\n{output}"), @r###"
  exit status: 0
  {13 1}
  "###);

  fs::remove_file("examples/arithmetic").unwrap();
}

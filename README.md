# ctflgrdiff
Does side-by-side diffs of control flow graphs by comparing basic blocks in a
way that can ignore all differences in block and value names. It only cares
about the structure of the code when doing the comparison.

## Building
You will need a Rust toolchain and LLVM 14 in order to support LLVM IR diffing.
To build:

```
cargo build
```

## Usage
First, create the two files you wish to diff:

```
cat > foo_int.c <<EOI
int foo(int x, int y) {
 return x * y + y;
}
EOI

cat > foo_long.c <<EOI
int foo(long x, long y) {
 return (int)(x * y + y);
}
EOI

clang -emit-llvm -c foo_int.c foo_long.c
```

Then you can diff the two files:

```
ctflgrdiff -f ll-bc foo_int.bc foo_long.bc
```

The `demo` directory contains example pairs of C code.

## Supported Binary Formats

- `ll-bc`: LLVM bitcode; note that LLVM 15+ use a different pointer format that
   will trigger LLVM 14 to segfault

# ctflgrdiff
Does side-by-side diffs of control flow graphs by comparing basic blocks in a
way that can ignore all differences in block and value names. It only cares
about the structure of the code when doing the comparison. It uses an algorithm
similar to [Needleman–Wunsch
algorithm](https://en.wikipedia.org/wiki/Needleman%E2%80%93Wunsch_algorithm).

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
- `arm64` aka `aarch64` aka `armv8`: 64-bit ARM code in a binary
- `arm32` aka `aarch32` aka `armv7`: 32-bit ARM code in a binary
- `avr`: ATmel AVR code in a binary; note that it cannot be in a fat MachO binary
- `x86` aka `x86-32` aka `x86_32` aka `i386` aka `i686`: 32-bit Intel code in a binary
- `x64` aka `x86-64` aka `x86_64`: 64-bit Intel code in a binary

For all formats _in a binary_, an ELF, MachO, or PE (Windows) executable,
library, or object file can be provided. An archive (`.a`) file containing ELF,
MachO, or PE object files is also supported. MachO multi-architecture (aka
_fat_) binaries are supported and only the instruction set requested will be
used.

Where appropriate, function names will go through C++ and Rust symbol
demangling.

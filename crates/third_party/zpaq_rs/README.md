# zpaq_rs

Safe Rust bindings for [libzpaq](http://mattmahoney.net/dc/zpaq.html) (Matt Mahoney's ZPAQ compression library).

Provides compression, decompression, streaming compression, and crypto utilities (SHA-1, SHA-256, AES-CTR, scrypt key stretching) via a statically linked C++ shim.

[![CI](https://github.com/turtle261/zpaq-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/turtle261/zpaq-rs/actions/workflows/rust.yml)
[![docs.rs](https://docs.rs/zpaq_rs/badge.svg)](https://docs.rs/zpaq_rs)

---

## Requirements

- Rust 1.85+ (edition 2024)
- A C++17-capable compiler: **clang++** (preferred, auto-detected) or **g++** / MSVC

The `cc` crate drives compilation at build time; no external libraries need to be installed.

---

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
zpaq_rs = "1.0"
```

### Compress and decompress

```rust
use zpaq_rs::{compress_to_vec, decompress_to_vec};

let data = b"hello zpaq";
let compressed = compress_to_vec(data, "1")?;
let restored   = decompress_to_vec(&compressed)?;
assert_eq!(restored, data);
```

### Streaming (Read/Write)

```rust
use std::io::Cursor;
use zpaq_rs::{compress_stream, decompress_stream};

let mut compressed = Vec::new();
compress_stream(Cursor::new(b"hello"), &mut compressed, "2", None, None)?;

let mut restored = Vec::new();
decompress_stream(Cursor::new(&compressed), &mut restored)?;
```

### Compressed size only (no allocation)

```rust
// Single-threaded
let sz = zpaq_rs::compress_size(b"some data", "3")?;

// Multi-threaded
let sz = zpaq_rs::compress_size_parallel(b"some data", "3", 4)?;
```

### Full ZPAQ archive operations (`add` / `list` / `extract`)

The crate can run the real `zpaq.cpp` JIDAC engine in-process, providing full
archive interoperability (multi-file archives, append updates, dedupe, list,
extract, `-threads`, and other CLI options).

```rust
use zpaq_rs::{zpaq_add, zpaq_command, zpaq_list};

// Equivalent to: zpaq add backup.zpaq ./data -method 3 -threads 4
zpaq_add("backup.zpaq", &["./data"], "3", 4)?;

// Equivalent to: zpaq list backup.zpaq
let listing = zpaq_list("backup.zpaq", &[])?;
println!("{}{}", listing.stdout, listing.stderr);

// Any CLI-compatible command is available:
// zpaq extract backup.zpaq -to ./restore
zpaq_command(&["extract", "backup.zpaq", "-to", "./restore"])?;
```

### Byte-level archive entries 
When you need to work directly with raw bytes (without staging temp input
files), use the in-memory entry APIs:

```rust
use zpaq_rs::{ArchiveEntry, archive_append_entries_file, archive_from_entries, archive_read_file_bytes};

let archive = archive_from_entries(
    &[
        ArchiveEntry { path: "a.txt", data: b"hello", comment: None },
        ArchiveEntry { path: "b.bin", data: &[1, 2, 3], comment: None },
    ],
    "3",
)?;

let bytes = archive_read_file_bytes(&archive, "a.txt")?;
assert_eq!(bytes, b"hello");

archive_append_entries_file(
    "my.zpaq",
    &[ArchiveEntry { path: "a.txt", data: b"updated", comment: None }],
    "3",
)?;
```

### Streaming compressor (per-byte bit counting)

```rust
use zpaq_rs::StreamingCompressor;

let mut sc = StreamingCompressor::new("2")?;
for &b in b"hello" {
    sc.push(b)?;
}
println!("bits: {:.2}", sc.bits());
```

---

## Method strings

| Value | Description |
|-------|-------------|
| `"1"` | Fast |
| `"2"` | Balanced |
| `"3"` | Better |
| `"4"` | Maximum |
| `"5"` | Ultra |
| `"x4.3ci1"` | Example explicit method |

Explicit method strings (starting with `x`, `s`, `i`, or a digit) allow fine-grained algorithm control. See the [ZPAQ specification](http://mattmahoney.net/dc/zpaq206.pdf) for details.

---

## Crypto utilities

```rust
// scrypt key stretching (N=16384, r=8, p=1 — same as zpaq encrypted archives)
let stretched = zpaq_rs::stretch_key(key32, salt32)?;

// Cryptographically strong random bytes
let bytes = zpaq_rs::random_bytes(32)?;

// SHA-1 / SHA-256
let digest = zpaq_rs::sha1(b"abc")?;
let digest = zpaq_rs::sha256(b"abc")?;
```

---

## Feature flags

| Flag | Effect |
|------|--------|
| `nojit` | Compiles libzpaq with `NOJIT`, disabling the x86 JIT. Required on NetBSD and OpenBSD. |

---

## Platform notes

| Platform | Status | Notes |
|----------|--------|-------|
| Linux (glibc) | ✓ | Tested in CI |
| Linux (musl) | ✓ | Tested in CI via Alpine |
| macOS | ✓ | Tested in CI |
| Windows | ✓ | Tested in CI; requires `advapi32` (linked automatically) |
| FreeBSD | ✓ | Tested in CI |
| OpenBSD | ✓ | Tested in CI; enable `nojit` feature |
| NetBSD | ✓ | Tested in CI; enable `nojit` feature, LTO disabled |

On NetBSD and OpenBSD, set `CARGO_FEATURE_NOJIT=1` (or use `--features nojit`) to disable the JIT back-end. This may also be required on a **hardened** Linux Kernel -- that is, if it enforces W^X.

---

## Compiler selection

The build script selects a C++ compiler in this order:

1. `ZPAQ_RS_CXX` environment variable (explicit override)
2. `CXX` environment variable (honoured by the `cc` crate)
3. `clang++` if available on `$PATH`
4. Default compiler for the platform (g++ / MSVC)

---

## License

This is (mostly) Public Domain Software.

CC0-1.0 — see [LICENSE](LICENSE). 

`libzpaq` and `zpaq.cpp` are distributed under their own licenses; see [zpaq/COPYING](zpaq/COPYING).


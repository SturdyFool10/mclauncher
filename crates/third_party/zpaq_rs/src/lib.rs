//! Safe Rust bindings for [libzpaq](https://github.com/zpaq/zpaq) — the ZPAQ
//! compression library by Matt Mahoney.
//!
//! This crate wraps a small C++ shim (`zpaq_rs_ffi.cpp`) that is compiled at
//! build time via the `cc` crate.  No dynamic libraries are required at
//! runtime; everything is statically linked.
//!
//! # Compression method strings
//!
//! Most functions accept a `method` parameter that controls the ZPAQ
//! compression algorithm and level.  Recognised values:
//!
//! | Value | Meaning |
//! |-------|---------|
//! | `"1"` | Fast (level 1) |
//! | `"2"` | Balanced (level 2) |
//! | `"3"` | Better (level 3) |
//! | `"4"` | Maximum (level 4) |
//! | `"5"` | Ultra (level 5) |
//! | `"x4.3ci1"` | Example explicit method string |
//!
//! Higher numeric levels compress better but are slower and use more memory.
//! Explicit method strings (starting with `x`, `s`, `i`, `0`–`9`) allow
//! fine-grained control; see the [zpaq specification](http://mattmahoney.net/dc/zpaq206.pdf)
//! for details.
//!
//! # Feature flags
//!
//! * **`nojit`** — Compiles `libzpaq` with `NOJIT` defined, disabling the JIT
//!   x86 back-end.  Required on platforms without a functional x86 JIT (NetBSD,
//!   OpenBSD).  Enabled automatically by the CI for those targets.
//!
//! # Quick start
//!
//! ```rust
//! use zpaq_rs::{compress_to_vec, decompress_to_vec};
//!
//! let original = b"hello zpaq";
//! let compressed = compress_to_vec(original, "1").unwrap();
//! let restored   = decompress_to_vec(&compressed).unwrap();
//! assert_eq!(restored, original);
//! ```
//!
//! For large data or streaming use cases prefer [`compress_stream`] /
//! [`decompress_stream`], which accept any [`std::io::Read`] / [`std::io::Write`].
//!
//! Use [`compress_size`] / [`compress_size_stream`] when you only need the
//! compressed byte count and not the compressed data itself (avoids allocation).
//!
//! # Performance notes
//!
//! * Rust LTO applies within Rust crates; C++ LTO applies within the C++ object
//!   files.  Cross-language inlining (Rust ↔ C++) is toolchain-dependent.
//! * [`compress_size`] / [`compress_size_stream`] use a C++-side counting writer
//!   to avoid per-byte FFI round-trips and are the fastest way to obtain $C(x)$
//!   for information-theoretic metrics.
//! * [`compress_size_parallel`] / [`compress_size_stream_parallel`] split the
//!   input into ZPAQ blocks and compress them in parallel, which can be faster
//!   on multi-core machines for large inputs.

mod sys;

use std::collections::VecDeque;
use std::ffi::CString;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::slice;
use std::sync::{Arc, Mutex};

/// Convenience alias for `std::result::Result<T, ZpaqError>`.
pub type Result<T> = std::result::Result<T, ZpaqError>;

/// Errors returned by this crate.
#[derive(Debug)]
pub enum ZpaqError {
    /// An error originating inside the C++ `libzpaq` / FFI shim.
    ///
    /// The inner string is the message reported by the C++ error channel.  It
    /// may be empty (shown as `"unknown error"`) if libzpaq did not supply one.
    Ffi(String),
    /// A method or path string contained an interior NUL byte (`\0`).
    ///
    /// ZPAQ method strings and file paths are passed to C++ as NUL-terminated
    /// strings, so any input containing `\0` is rejected before crossing the FFI
    /// boundary.
    NulInString,
}

impl std::fmt::Display for ZpaqError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZpaqError::Ffi(s) => write!(f, "libzpaq: {s}"),
            ZpaqError::NulInString => write!(f, "string contained NUL byte"),
        }
    }
}

impl std::error::Error for ZpaqError {}

fn last_error_string() -> Option<String> {
    unsafe {
        let len = sys::zpaq_last_error_len();
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u8; len];
        let copied = sys::zpaq_last_error_copy(buf.as_mut_ptr() as *mut c_char, len);
        buf.truncate(copied);
        String::from_utf8(buf).ok()
    }
}

fn last_stdout_string() -> Option<String> {
    unsafe {
        let len = sys::zpaq_last_stdout_len();
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u8; len];
        let copied = sys::zpaq_last_stdout_copy(buf.as_mut_ptr() as *mut c_char, len);
        buf.truncate(copied);
        String::from_utf8(buf).ok()
    }
}

fn last_stderr_string() -> Option<String> {
    unsafe {
        let len = sys::zpaq_last_stderr_len();
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u8; len];
        let copied = sys::zpaq_last_stderr_copy(buf.as_mut_ptr() as *mut c_char, len);
        buf.truncate(copied);
        String::from_utf8(buf).ok()
    }
}

fn err_from_last() -> ZpaqError {
    last_error_string()
        .map(ZpaqError::Ffi)
        .unwrap_or_else(|| ZpaqError::Ffi("unknown error".to_string()))
}

fn clear_last_error() {
    unsafe { sys::zpaq_clear_last_error() };
}

fn clear_last_output() {
    unsafe { sys::zpaq_clear_last_output() };
}

/// Captured output of an embedded `zpaq` command.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ZpaqCommandOutput {
    /// Captured standard output from the command.
    pub stdout: String,
    /// Captured standard error from the command.
    pub stderr: String,
}

fn zpaq_command_inner(args: &[String]) -> Result<ZpaqCommandOutput> {
    clear_last_error();
    clear_last_output();

    let mut cargs = Vec::with_capacity(args.len() + 1);
    cargs.push(CString::new("zpaq").map_err(|_| ZpaqError::NulInString)?);
    for arg in args {
        cargs.push(CString::new(arg.as_str()).map_err(|_| ZpaqError::NulInString)?);
    }
    let ptrs: Vec<*const c_char> = cargs.iter().map(|s| s.as_ptr()).collect();

    let rc = unsafe { sys::zpaq_jidac_run(ptrs.len() as c_int, ptrs.as_ptr()) };
    if rc != 0 {
        return Err(err_from_last());
    }

    Ok(ZpaqCommandOutput {
        stdout: last_stdout_string().unwrap_or_default(),
        stderr: last_stderr_string().unwrap_or_default(),
    })
}

/// A [`Write`] implementation that discards written bytes while counting them.
///
/// Useful when you need the compressed (or decompressed) size without
/// allocating a buffer.  For compressed-size measurements prefer the dedicated
/// [`compress_size`] / [`compress_size_stream`] functions, which use a C++-side
/// counting writer and avoid per-write FFI overhead entirely.
///
/// # Example
///
/// ```rust
/// use std::io::Write;
/// use zpaq_rs::CountingWriter;
///
/// let mut cw = CountingWriter::default();
/// cw.write_all(b"hello").unwrap();
/// assert_eq!(cw.bytes_written(), 5);
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct CountingWriter {
    bytes: u64,
}

impl CountingWriter {
    /// Returns the total number of bytes written so far.
    pub fn bytes_written(&self) -> u64 {
        self.bytes
    }
}

impl Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(buf.len() as u64)
            .ok_or_else(|| std::io::Error::other("byte counter overflow"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// ---------------- Callback plumbing ----------------

struct ReadCtx<R: Read + Send> {
    reader: R,
}

struct WriteCtx<W: Write + Send> {
    writer: W,
}

unsafe extern "C" fn read_cb<R: Read + Send>(
    ctx: *mut std::os::raw::c_void,
    buf: *mut c_char,
    n: c_int,
) -> c_int {
    unsafe {
        let ctx = &mut *(ctx as *mut ReadCtx<R>);
        let slice = slice::from_raw_parts_mut(buf as *mut u8, n as usize);
        match ctx.reader.read(slice) {
            Ok(bytes) => bytes as c_int,
            Err(e) => {
                set_callback_error(&e.to_string());
                sys::RUST_CALLBACK_ERROR
            }
        }
    }
}

unsafe extern "C" fn write_cb<W: Write + Send>(
    ctx: *mut std::os::raw::c_void,
    buf: *const c_char,
    n: c_int,
) -> c_int {
    unsafe {
        let ctx = &mut *(ctx as *mut WriteCtx<W>);
        let slice = slice::from_raw_parts(buf as *const u8, n as usize);
        match ctx.writer.write_all(slice) {
            Ok(()) => 0,
            Err(e) => {
                set_callback_error(&e.to_string());
                sys::RUST_CALLBACK_ERROR
            }
        }
    }
}

// ---------------- Streaming compressor ----------------

#[derive(Default)]
struct StreamReader {
    buf: VecDeque<u8>,
}

impl StreamReader {
    fn push(&mut self, b: u8) {
        self.buf.push_back(b);
    }
}

impl Read for StreamReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let mut n = 0usize;
        while n < out.len() {
            match self.buf.pop_front() {
                Some(b) => {
                    out[n] = b;
                    n += 1;
                }
                None => break,
            }
        }
        Ok(n)
    }
}

/// Byte-at-a-time ZPAQ compressor that reports the running encoded bit count.
///
/// Unlike the block-oriented [`compress_stream`], `StreamingCompressor` feeds
/// one byte at a time to the underlying ZPAQ `Compressor` and queries the
/// internal bit counter after each byte.  This is useful for measuring how many
/// bits are required to encode each symbol incrementally — for example when
/// computing per-symbol information content.
///
/// # Method string restrictions
///
/// Streaming mode only supports:
/// * Numeric levels **1, 2, or 3** — levels 4 and 5 use block pre-processing
///   that is incompatible with per-byte feeding.
/// * Explicit method strings that start with `x`, `s`, `i`, or a digit and do
///   **not** enable block pre-processing.
///
/// Attempting to create a compressor with level 4, 5, or a method that requires
/// block preprocessing will return [`ZpaqError::Ffi`].
///
/// # Example
///
/// ```rust
/// use zpaq_rs::StreamingCompressor;
///
/// let mut sc = StreamingCompressor::new("2").unwrap();
/// for &b in b"hello" {
///     sc.push(b).unwrap();
/// }
/// println!("bits so far: {:.2}", sc.bits());
/// ```
pub struct StreamingCompressor {
    compressor: *mut sys::Compressor,
    reader: *mut sys::RustReader,
    writer: *mut sys::RustWriter,
    reader_ctx: *mut ReadCtx<StreamReader>,
    writer_ctx: *mut WriteCtx<CountingWriter>,
}

unsafe impl Send for StreamingCompressor {}

impl StreamingCompressor {
    /// Creates a new streaming compressor using the given method string.
    ///
    /// Allocates and initialises the underlying `libzpaq::Compressor`, sets up
    /// internal reader/writer callbacks, writes the ZPAQ block tag, and opens
    /// the first segment ready to receive bytes via [`push`](Self::push).
    ///
    /// Returns [`ZpaqError::Ffi`] if the method is unsupported in streaming mode
    /// (e.g. numeric levels 4–5) or if any C++ initialisation step fails.
    pub fn new(method: &str) -> Result<Self> {
        let method_trim = method.trim();
        if method_trim.is_empty() {
            return Err(ZpaqError::Ffi("method string is empty".into()));
        }
        let numeric = method_trim.parse::<i32>().ok();
        let level = numeric.filter(|v| (1..=3).contains(v));
        if numeric.is_some() && level.is_none() {
            return Err(ZpaqError::Ffi(
                "streaming numeric levels support 1..3 only; use x/s/i/0 with no preprocessing"
                    .into(),
            ));
        }

        let compressor = unsafe { sys::zpaq_compressor_new() };
        if compressor.is_null() {
            return Err(ZpaqError::Ffi("zpaq_compressor_new failed".into()));
        }

        let reader_ctx = Box::into_raw(Box::new(ReadCtx {
            reader: StreamReader::default(),
        }));
        let writer_ctx = Box::into_raw(Box::new(WriteCtx {
            writer: CountingWriter::default(),
        }));

        let reader =
            unsafe { sys::zpaq_reader_new(reader_ctx.cast(), None, Some(read_cb::<StreamReader>)) };
        if reader.is_null() {
            unsafe {
                sys::zpaq_compressor_free(compressor);
                drop(Box::from_raw(reader_ctx));
                drop(Box::from_raw(writer_ctx));
            }
            return Err(ZpaqError::Ffi("zpaq_reader_new failed".into()));
        }

        let writer = unsafe {
            sys::zpaq_writer_new(
                writer_ctx.cast(),
                Some(put_cb::<CountingWriter>),
                Some(write_cb::<CountingWriter>),
            )
        };
        if writer.is_null() {
            unsafe {
                sys::zpaq_reader_free(reader);
                sys::zpaq_compressor_free(compressor);
                drop(Box::from_raw(reader_ctx));
                drop(Box::from_raw(writer_ctx));
            }
            return Err(ZpaqError::Ffi("zpaq_writer_new failed".into()));
        }

        let rc_out = unsafe { sys::zpaq_compressor_set_output(compressor, writer) };
        if rc_out != 0 {
            unsafe {
                sys::zpaq_writer_free(writer);
                sys::zpaq_reader_free(reader);
                sys::zpaq_compressor_free(compressor);
                drop(Box::from_raw(reader_ctx));
                drop(Box::from_raw(writer_ctx));
            }
            return Err(err_from_last());
        }

        let rc_in = unsafe { sys::zpaq_compressor_set_input(compressor, reader) };
        if rc_in != 0 {
            unsafe {
                sys::zpaq_writer_free(writer);
                sys::zpaq_reader_free(reader);
                sys::zpaq_compressor_free(compressor);
                drop(Box::from_raw(reader_ctx));
                drop(Box::from_raw(writer_ctx));
            }
            return Err(err_from_last());
        }

        let rc_tag = unsafe { sys::zpaq_compressor_write_tag(compressor) };
        if rc_tag != 0 {
            unsafe {
                sys::zpaq_writer_free(writer);
                sys::zpaq_reader_free(reader);
                sys::zpaq_compressor_free(compressor);
                drop(Box::from_raw(reader_ctx));
                drop(Box::from_raw(writer_ctx));
            }
            return Err(err_from_last());
        }

        let rc_block = if let Some(level) = level {
            unsafe { sys::zpaq_compressor_start_block_level(compressor, level) }
        } else {
            let method_c = CString::new(method_trim).map_err(|_| ZpaqError::NulInString)?;
            unsafe { sys::zpaq_compressor_start_block_method(compressor, method_c.as_ptr()) }
        };
        if rc_block != 0 {
            unsafe {
                sys::zpaq_writer_free(writer);
                sys::zpaq_reader_free(reader);
                sys::zpaq_compressor_free(compressor);
                drop(Box::from_raw(reader_ctx));
                drop(Box::from_raw(writer_ctx));
            }
            return Err(err_from_last());
        }

        let rc_seg =
            unsafe { sys::zpaq_compressor_start_segment(compressor, ptr::null(), ptr::null()) };
        if rc_seg != 0 {
            unsafe {
                sys::zpaq_writer_free(writer);
                sys::zpaq_reader_free(reader);
                sys::zpaq_compressor_free(compressor);
                drop(Box::from_raw(reader_ctx));
                drop(Box::from_raw(writer_ctx));
            }
            return Err(err_from_last());
        }

        Ok(Self {
            compressor,
            reader,
            writer,
            reader_ctx,
            writer_ctx,
        })
    }

    /// Feeds one byte into the compressor and advances the internal state.
    ///
    /// Returns [`ZpaqError::Ffi`] if the underlying `libzpaq::Compressor::compress`
    /// call fails (e.g. due to an I/O error in the underlying writer callback).
    pub fn push(&mut self, b: u8) -> Result<()> {
        unsafe {
            let ctx = &mut *self.reader_ctx;
            ctx.reader.push(b);
        }
        let rc = unsafe { sys::zpaq_compressor_compress(self.compressor, 1) };
        if rc < 0 {
            return Err(err_from_last());
        }
        Ok(())
    }

    /// Returns the number of bits written to the compressed output so far.
    ///
    /// This reflects the running total emitted by `libzpaq`'s internal bit
    /// counter and accounts for both the block header and all bytes fed via
    /// [`push`](Self::push).  The value is a `f64` because `libzpaq` tracks
    /// fractional bits internally.
    pub fn bits(&self) -> f64 {
        unsafe { sys::zpaq_compressor_get_bits(self.compressor) }
    }
}

impl Drop for StreamingCompressor {
    fn drop(&mut self) {
        unsafe {
            sys::zpaq_writer_free(self.writer);
            sys::zpaq_reader_free(self.reader);
            sys::zpaq_compressor_free(self.compressor);
            drop(Box::from_raw(self.reader_ctx));
            drop(Box::from_raw(self.writer_ctx));
        }
    }
}

unsafe extern "C" fn put_cb<W: Write + Send>(ctx: *mut std::os::raw::c_void, c: c_int) -> c_int {
    unsafe {
        let ctx = &mut *(ctx as *mut WriteCtx<W>);
        let byte = [c as u8];
        match ctx.writer.write_all(&byte) {
            Ok(()) => 0,
            Err(e) => {
                set_callback_error(&e.to_string());
                sys::RUST_CALLBACK_ERROR
            }
        }
    }
}

fn set_callback_error(msg: &str) {
    if let Ok(cstr) = CString::new(msg) {
        unsafe { sys::zpaq_set_last_error(cstr.as_ptr()) };
    }
}

struct FfiReader<R: Read + Send> {
    raw: *mut sys::RustReader,
    ctx: *mut ReadCtx<R>,
}

impl<R: Read + Send> FfiReader<R> {
    fn new(reader: R) -> Result<Self> {
        let ctx = Box::into_raw(Box::new(ReadCtx { reader }));
        let raw = unsafe { sys::zpaq_reader_new(ctx as *mut _, None, Some(read_cb::<R>)) };
        if raw.is_null() {
            unsafe {
                drop(Box::from_raw(ctx));
            }
            return Err(err_from_last());
        }
        Ok(Self { raw, ctx })
    }
}

impl<R: Read + Send> Drop for FfiReader<R> {
    fn drop(&mut self) {
        unsafe {
            sys::zpaq_reader_free(self.raw);
            drop(Box::from_raw(self.ctx));
        }
    }
}

struct FfiWriter<W: Write + Send> {
    raw: *mut sys::RustWriter,
    ctx: *mut WriteCtx<W>,
}

impl<W: Write + Send> FfiWriter<W> {
    fn new(writer: W) -> Result<Self> {
        let ctx = Box::into_raw(Box::new(WriteCtx { writer }));
        let raw =
            unsafe { sys::zpaq_writer_new(ctx as *mut _, Some(put_cb::<W>), Some(write_cb::<W>)) };
        if raw.is_null() {
            unsafe {
                drop(Box::from_raw(ctx));
            }
            return Err(err_from_last());
        }
        Ok(Self { raw, ctx })
    }
}

impl<W: Write + Send> Drop for FfiWriter<W> {
    fn drop(&mut self) {
        unsafe {
            sys::zpaq_writer_free(self.raw);
            drop(Box::from_raw(self.ctx));
        }
    }
}

#[derive(Clone, Default)]
struct SharedVecWriter {
    inner: Arc<Mutex<Vec<u8>>>,
}

impl SharedVecWriter {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn bytes(&self) -> Vec<u8> {
        self.inner.lock().expect("poisoned writer buffer").clone()
    }
}

impl Write for SharedVecWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner
            .lock()
            .map_err(|_| std::io::Error::other("poisoned writer buffer"))?
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct SinkWriter;

impl Write for SinkWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn start_block_for_method(compressor: *mut sys::Compressor, method: &str) -> Result<()> {
    let method_trim = method.trim();
    if method_trim.is_empty() {
        return Err(ZpaqError::Ffi("method string is empty".into()));
    }
    let rc = if let Ok(level) = method_trim.parse::<i32>() {
        if !(1..=5).contains(&level) {
            return Err(ZpaqError::Ffi(
                "numeric method level must be in 1..=5".into(),
            ));
        }
        unsafe { sys::zpaq_compressor_start_block_level(compressor, level) }
    } else {
        let method_c = CString::new(method_trim).map_err(|_| ZpaqError::NulInString)?;
        unsafe { sys::zpaq_compressor_start_block_method(compressor, method_c.as_ptr()) }
    };

    if rc == 0 {
        Ok(())
    } else {
        Err(err_from_last())
    }
}

/// One in-archive file entry represented as raw bytes.
#[derive(Debug, Clone, Copy)]
pub struct ArchiveEntry<'a> {
    /// File path to store in the archive.
    pub path: &'a str,
    /// File contents.
    pub data: &'a [u8],
    /// Optional per-segment comment.
    pub comment: Option<&'a str>,
}

/// Creates a ZPAQ stream archive in memory from raw byte entries.
///
/// This performs no scratch-file I/O and writes each entry with its `path`
/// stored as the segment filename.
pub fn archive_from_entries(entries: &[ArchiveEntry<'_>], method: &str) -> Result<Vec<u8>> {
    clear_last_error();
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let compressor = unsafe { sys::zpaq_compressor_new() };
    if compressor.is_null() {
        return Err(err_from_last());
    }

    let out_shared = SharedVecWriter::new();
    let out_writer = FfiWriter::new(out_shared.clone())?;

    let set_out = unsafe { sys::zpaq_compressor_set_output(compressor, out_writer.raw) };
    if set_out != 0 {
        unsafe { sys::zpaq_compressor_free(compressor) };
        return Err(err_from_last());
    }

    let rc_tag = unsafe { sys::zpaq_compressor_write_tag(compressor) };
    if rc_tag != 0 {
        unsafe { sys::zpaq_compressor_free(compressor) };
        return Err(err_from_last());
    }

    start_block_for_method(compressor, method)?;

    for entry in entries {
        let filename_c = CString::new(entry.path).map_err(|_| ZpaqError::NulInString)?;
        let comment_c = match entry.comment {
            Some(text) => Some(CString::new(text).map_err(|_| ZpaqError::NulInString)?),
            None => None,
        };

        let rc_seg = unsafe {
            sys::zpaq_compressor_start_segment(
                compressor,
                filename_c.as_ptr(),
                comment_c
                    .as_ref()
                    .map(|s| s.as_ptr())
                    .unwrap_or(ptr::null()),
            )
        };
        if rc_seg != 0 {
            unsafe { sys::zpaq_compressor_free(compressor) };
            return Err(err_from_last());
        }

        let input = FfiReader::new(std::io::Cursor::new(entry.data))?;
        let rc_in = unsafe { sys::zpaq_compressor_set_input(compressor, input.raw) };
        if rc_in != 0 {
            unsafe { sys::zpaq_compressor_free(compressor) };
            return Err(err_from_last());
        }

        loop {
            let rc = unsafe { sys::zpaq_compressor_compress(compressor, 1 << 20) };
            if rc < 0 {
                unsafe { sys::zpaq_compressor_free(compressor) };
                return Err(err_from_last());
            }
            if rc == 0 {
                break;
            }
        }

        let rc_end_seg = unsafe { sys::zpaq_compressor_end_segment(compressor, ptr::null()) };
        if rc_end_seg != 0 {
            unsafe { sys::zpaq_compressor_free(compressor) };
            return Err(err_from_last());
        }
    }

    let rc_end_block = unsafe { sys::zpaq_compressor_end_block(compressor) };
    unsafe { sys::zpaq_compressor_free(compressor) };
    if rc_end_block != 0 {
        return Err(err_from_last());
    }

    drop(out_writer);
    Ok(out_shared.bytes())
}

/// Appends raw byte entries to an archive file path without creating scratch files.
pub fn archive_append_entries_file(
    archive_path: &str,
    entries: &[ArchiveEntry<'_>],
    method: &str,
) -> Result<()> {
    let payload = archive_from_entries(entries, method)?;
    if payload.is_empty() {
        return Ok(());
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(archive_path)
        .map_err(|e| ZpaqError::Ffi(format!("open archive for append failed: {e}")))?;
    file.write_all(&payload)
        .map_err(|e| ZpaqError::Ffi(format!("append archive write failed: {e}")))
}

/// Reads the newest segment whose stored filename matches `path` from an
/// in-memory archive stream.
fn archive_read_file_bytes_single_stream(archive: &[u8], path: &str) -> Result<Option<Vec<u8>>> {
    clear_last_error();

    let reader = FfiReader::new(std::io::Cursor::new(archive))?;
    let decompresser = unsafe { sys::zpaq_decompresser_new() };
    if decompresser.is_null() {
        return Err(err_from_last());
    }

    let set_in = unsafe { sys::zpaq_decompresser_set_input(decompresser, reader.raw) };
    if set_in != 0 {
        unsafe { sys::zpaq_decompresser_free(decompresser) };
        return Err(err_from_last());
    }

    let mut found: Option<Vec<u8>> = None;
    let mut mem_out = 0.0f64;

    loop {
        let rc_block = unsafe { sys::zpaq_decompresser_find_block(decompresser, &mut mem_out) };
        if rc_block < 0 {
            unsafe { sys::zpaq_decompresser_free(decompresser) };
            return Err(err_from_last());
        }
        if rc_block == 0 {
            break;
        }

        loop {
            let filename_shared = SharedVecWriter::new();
            let filename_writer = FfiWriter::new(filename_shared.clone())?;
            let rc_filename =
                unsafe { sys::zpaq_decompresser_find_filename(decompresser, filename_writer.raw) };
            if rc_filename < 0 {
                unsafe { sys::zpaq_decompresser_free(decompresser) };
                return Err(err_from_last());
            }
            if rc_filename == 0 {
                break;
            }
            drop(filename_writer);

            let comment_writer = FfiWriter::new(SinkWriter)?;
            let rc_comment =
                unsafe { sys::zpaq_decompresser_read_comment(decompresser, comment_writer.raw) };
            if rc_comment != 0 {
                unsafe { sys::zpaq_decompresser_free(decompresser) };
                return Err(err_from_last());
            }
            drop(comment_writer);

            let mut filename_bytes = filename_shared.bytes();
            while filename_bytes.last().copied() == Some(0) {
                filename_bytes.pop();
            }
            let filename = String::from_utf8_lossy(&filename_bytes);
            let is_target = filename == path;

            let output_shared = SharedVecWriter::new();
            let output_sink: Box<dyn Write + Send> = if is_target {
                Box::new(output_shared.clone())
            } else {
                Box::new(SinkWriter)
            };
            let output_writer = FfiWriter::new(output_sink)?;

            let rc_set_out =
                unsafe { sys::zpaq_decompresser_set_output(decompresser, output_writer.raw) };
            if rc_set_out != 0 {
                unsafe { sys::zpaq_decompresser_free(decompresser) };
                return Err(err_from_last());
            }

            loop {
                let rc_decompress =
                    unsafe { sys::zpaq_decompresser_decompress(decompresser, 1 << 20) };
                if rc_decompress < 0 {
                    unsafe { sys::zpaq_decompresser_free(decompresser) };
                    return Err(err_from_last());
                }
                if rc_decompress == 0 {
                    break;
                }
            }

            let mut segment_end = [0u8; 21];
            let rc_end = unsafe {
                sys::zpaq_decompresser_read_segment_end(decompresser, segment_end.as_mut_ptr())
            };
            if rc_end != 0 {
                unsafe { sys::zpaq_decompresser_free(decompresser) };
                return Err(err_from_last());
            }

            drop(output_writer);

            if is_target {
                found = Some(output_shared.bytes());
            }
        }
    }

    unsafe { sys::zpaq_decompresser_free(decompresser) };
    Ok(found)
}

/// Reads the newest segment whose stored filename matches `path` from an
/// archive byte slice.
///
/// Supports concatenated ZPAQ streams (e.g. repeated append operations).
pub fn archive_read_file_bytes(archive: &[u8], path: &str) -> Result<Vec<u8>> {
    const ZPAQ_TAG: [u8; 13] = [
        0x37, 0x6b, 0x53, 0x74, 0xa0, 0x31, 0x83, 0xd3, 0x8c, 0xb2, 0x28, 0xb0, 0xd3,
    ];

    if archive.is_empty() {
        return Err(ZpaqError::Ffi("archive is empty".into()));
    }

    let mut starts = Vec::new();
    for index in 0..=archive.len().saturating_sub(ZPAQ_TAG.len()) {
        if archive[index..].starts_with(&ZPAQ_TAG) {
            starts.push(index);
        }
    }
    if starts.is_empty() {
        return Err(ZpaqError::Ffi("no ZPAQ stream header found".into()));
    }

    let mut latest = None;
    for (position, start) in starts.iter().copied().enumerate() {
        let end = starts.get(position + 1).copied().unwrap_or(archive.len());
        if end <= start {
            continue;
        }
        if let Some(bytes) = archive_read_file_bytes_single_stream(&archive[start..end], path)? {
            latest = Some(bytes);
        }
    }

    latest.ok_or_else(|| ZpaqError::Ffi(format!("file path not found in archive: {path}")))
}

/// Reads bytes for `path` from an archive file.
pub fn archive_read_file_bytes_from_file(archive_path: &str, path: &str) -> Result<Vec<u8>> {
    let archive = std::fs::read(archive_path)
        .map_err(|e| ZpaqError::Ffi(format!("read archive file failed: {e}")))?;
    archive_read_file_bytes(&archive, path)
}

// ---------------- Public API ----------------

/// Compresses `input` into a `Vec<u8>` using the given ZPAQ method string.
///
/// This is a convenience wrapper around [`compress_stream`] that owns both the
/// input and output buffers.  For large data, prefer [`compress_stream`] to
/// avoid double-buffering, or [`compress_size`] if you only need the size.
///
/// # Example
///
/// ```rust
/// let compressed = zpaq_rs::compress_to_vec(b"hello zpaq", "1").unwrap();
/// assert!(!compressed.is_empty());
/// ```
pub fn compress_to_vec(input: &[u8], method: &str) -> Result<Vec<u8>> {
    let cursor = std::io::Cursor::new(input);
    let mut out = Vec::new();
    compress_stream(cursor, &mut out, method, None, None)?;
    Ok(out)
}

/// Returns the compressed size of `input` in bytes without materialising the
/// compressed data.
///
/// Uses a C++-side counting writer so no allocation is needed for the compressed
/// bytes.  This is the fastest single-threaded way to compute $C(x)$ for
/// information-theoretic metrics such as NCD.
///
/// # Example
///
/// ```rust
/// let sz = zpaq_rs::compress_size(b"aaaaaaaaa", "1").unwrap();
/// assert!(sz > 0);
/// ```
pub fn compress_size(input: &[u8], method: &str) -> Result<u64> {
    compress_size_stream(std::io::Cursor::new(input), method, None, None)
}

/// Returns the compressed size of `input` in bytes using multiple threads.
///
/// Splits the input into ZPAQ blocks (based on the method's block size) and
/// compresses them in parallel using `libzpaq::compressBlock`.  For
/// `threads <= 1` this falls back to the single-threaded path.
///
/// Equivalent to [`compress_size_stream_parallel`] with a [`std::io::Cursor`]
/// over `input`.
pub fn compress_size_parallel(input: &[u8], method: &str, threads: usize) -> Result<u64> {
    compress_size_stream_parallel(std::io::Cursor::new(input), method, None, None, threads)
}

/// Returns the compressed size of data from `reader` in bytes without
/// materialising the compressed output.
///
/// Uses a C++-side counting writer to avoid Rust-side per-write overhead.
/// `filename` and `comment` are optional ZPAQ segment metadata fields; pass
/// `None` for both unless you are building interoperable archives.
pub fn compress_size_stream<R: Read + Send>(
    reader: R,
    method: &str,
    filename: Option<&str>,
    comment: Option<&str>,
) -> Result<u64> {
    clear_last_error();
    let method_c = CString::new(method).map_err(|_| ZpaqError::NulInString)?;
    let filename_c = match filename {
        Some(s) => Some(CString::new(s).map_err(|_| ZpaqError::NulInString)?),
        None => None,
    };
    let comment_c = match comment {
        Some(s) => Some(CString::new(s).map_err(|_| ZpaqError::NulInString)?),
        None => None,
    };
    let reader = FfiReader::new(reader)?;
    let mut out_size: u64 = 0;
    let rc = unsafe {
        sys::zpaq_compress_size(
            reader.raw,
            method_c.as_ptr(),
            filename_c
                .as_ref()
                .map(|c| c.as_ptr())
                .unwrap_or(ptr::null()),
            comment_c
                .as_ref()
                .map(|c| c.as_ptr())
                .unwrap_or(ptr::null()),
            1,
            &mut out_size as *mut u64,
        )
    };
    if rc == 0 {
        Ok(out_size)
    } else {
        Err(err_from_last())
    }
}

/// Returns the compressed size of data from `reader` in bytes using multiple
/// threads.
///
/// Splits the input into ZPAQ blocks and compresses them in parallel.  Falls
/// back to the single-threaded path when `threads <= 1`.
pub fn compress_size_stream_parallel<R: Read + Send>(
    reader: R,
    method: &str,
    filename: Option<&str>,
    comment: Option<&str>,
    threads: usize,
) -> Result<u64> {
    clear_last_error();
    let method_c = CString::new(method).map_err(|_| ZpaqError::NulInString)?;
    let filename_c = match filename {
        Some(s) => Some(CString::new(s).map_err(|_| ZpaqError::NulInString)?),
        None => None,
    };
    let comment_c = match comment {
        Some(s) => Some(CString::new(s).map_err(|_| ZpaqError::NulInString)?),
        None => None,
    };
    let reader = FfiReader::new(reader)?;
    let mut out_size: u64 = 0;
    let rc = unsafe {
        sys::zpaq_compress_size_parallel(
            reader.raw,
            method_c.as_ptr(),
            filename_c
                .as_ref()
                .map(|c| c.as_ptr())
                .unwrap_or(ptr::null()),
            comment_c
                .as_ref()
                .map(|c| c.as_ptr())
                .unwrap_or(ptr::null()),
            1,
            threads as i32,
            &mut out_size as *mut u64,
        )
    };
    if rc == 0 {
        Ok(out_size)
    } else {
        Err(err_from_last())
    }
}

/// Returns the archive size (in bytes) that `zpaq add` would produce for a
/// single file on disk.
///
/// Runs the real `zpaq.cpp` JIDAC pipeline in-process — including fragment
/// deduplication and index overhead — with `archive=""` so output is discarded
/// but the byte-count accounting is identical to a real archive write.
///
/// This is the correct metric to compare against
/// `zpaq add my.arc <file>; du -b my.arc`.
///
/// `path` must be a valid filesystem path to an existing file.  `threads`
/// controls the number of parallel compression threads; `0` lets libzpaq
/// choose.
///
/// # Errors
///
/// Returns [`ZpaqError::Ffi`] if the file cannot be opened or if the JIDAC
/// pipeline encounters an error.
pub fn zpaq_add_archive_size_file(path: &str, method: &str, threads: usize) -> Result<u64> {
    clear_last_error();
    let path_c = CString::new(path).map_err(|_| ZpaqError::NulInString)?;
    let method_c = CString::new(method).map_err(|_| ZpaqError::NulInString)?;
    let mut out_size: u64 = 0;
    let rc = unsafe {
        sys::zpaq_jidac_add_archive_size_file(
            path_c.as_ptr(),
            method_c.as_ptr(),
            threads as c_int,
            &mut out_size as *mut u64,
        )
    };
    if rc == 0 {
        Ok(out_size)
    } else {
        Err(err_from_last())
    }
}

/// Runs an embedded `zpaq` command in-process and captures its output.
///
/// `args` must contain only the command arguments, exactly as you would pass
/// after `zpaq` on the shell.
///
/// # Examples
///
/// ```rust,no_run
/// let out = zpaq_rs::zpaq_command(&["list", "archive.zpaq"])?;
/// println!("{}", out.stdout);
/// # Ok::<(), zpaq_rs::ZpaqError>(())
/// ```
pub fn zpaq_command(args: &[&str]) -> Result<ZpaqCommandOutput> {
    let owned: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    zpaq_command_inner(&owned)
}

/// Equivalent of `zpaq add <archive> <inputs...> -method <method> -threads <threads>`.
///
/// This uses the real JIDAC engine from `zpaq.cpp`, so append semantics,
/// deduplication, and archive metadata are fully interoperable with the `zpaq`
/// binary.
pub fn zpaq_add(
    archive: &str,
    inputs: &[&str],
    method: &str,
    threads: usize,
) -> Result<ZpaqCommandOutput> {
    if inputs.is_empty() {
        return Err(ZpaqError::Ffi(
            "zpaq add requires at least one input path".to_string(),
        ));
    }
    let mut args = Vec::with_capacity(inputs.len() + 7);
    args.push("add".to_string());
    args.push(archive.to_string());
    for input in inputs {
        args.push((*input).to_string());
    }
    args.push("-method".to_string());
    args.push(method.to_string());
    args.push("-threads".to_string());
    args.push(threads.to_string());
    zpaq_command_inner(&args)
}

/// Equivalent of `zpaq extract <archive> [files...]`.
pub fn zpaq_extract(archive: &str, files: &[&str]) -> Result<ZpaqCommandOutput> {
    let mut args = Vec::with_capacity(files.len() + 2);
    args.push("extract".to_string());
    args.push(archive.to_string());
    for file in files {
        args.push((*file).to_string());
    }
    zpaq_command_inner(&args)
}

/// Equivalent of `zpaq list <archive> [files...]`.
pub fn zpaq_list(archive: &str, files: &[&str]) -> Result<ZpaqCommandOutput> {
    let mut args = Vec::with_capacity(files.len() + 2);
    args.push("list".to_string());
    args.push(archive.to_string());
    for file in files {
        args.push((*file).to_string());
    }
    zpaq_command_inner(&args)
}

/// Decompresses a complete ZPAQ stream held in `input` and returns the
/// original data as a `Vec<u8>`.
///
/// This is a convenience wrapper around [`decompress_stream`] that owns both
/// the input and output.  For large data, prefer [`decompress_stream`] to
/// avoid double-buffering.
///
/// # Example
///
/// ```rust
/// let c = zpaq_rs::compress_to_vec(b"hello zpaq", "1").unwrap();
/// let d = zpaq_rs::decompress_to_vec(&c).unwrap();
/// assert_eq!(d, b"hello zpaq");
/// ```
pub fn decompress_to_vec(input: &[u8]) -> Result<Vec<u8>> {
    let cursor = std::io::Cursor::new(input);
    let mut out = Vec::new();
    decompress_stream(cursor, &mut out)?;
    Ok(out)
}

/// Returns the decompressed size of the ZPAQ stream in `input` without
/// materialising the output.
///
/// Wrapper around [`decompress_size_stream`] with a [`std::io::Cursor`].
pub fn decompress_size(input: &[u8]) -> Result<u64> {
    decompress_size_stream(std::io::Cursor::new(input))
}

/// Returns the decompressed size of the ZPAQ stream from `reader` without
/// materialising output.
///
/// Uses a C++-side counting writer so no allocation is needed for the
/// decompressed bytes.
pub fn decompress_size_stream<R: Read + Send>(reader: R) -> Result<u64> {
    clear_last_error();
    let reader = FfiReader::new(reader)?;
    let mut out_size: u64 = 0;
    let rc = unsafe { sys::zpaq_decompress_size(reader.raw, &mut out_size as *mut u64) };
    if rc == 0 {
        Ok(out_size)
    } else {
        Err(err_from_last())
    }
}

/// Compresses data from `reader` and writes the ZPAQ archive to `writer`.
///
/// `method` is the ZPAQ method string (e.g. `"1"`, `"x4.3ci1"`).
/// `filename` and `comment` are optional segment metadata; pass `None` for
/// both in typical usage.
///
/// # Example
///
/// ```rust
/// use std::io::Cursor;
/// let mut out = Vec::new();
/// zpaq_rs::compress_stream(Cursor::new(b"hello"), &mut out, "1", None, None).unwrap();
/// assert!(!out.is_empty());
/// ```
pub fn compress_stream<R: Read + Send, W: Write + Send>(
    reader: R,
    writer: W,
    method: &str,
    filename: Option<&str>,
    comment: Option<&str>,
) -> Result<()> {
    clear_last_error();
    let method_c = CString::new(method).map_err(|_| ZpaqError::NulInString)?;
    let filename_c = match filename {
        Some(s) => Some(CString::new(s).map_err(|_| ZpaqError::NulInString)?),
        None => None,
    };
    let comment_c = match comment {
        Some(s) => Some(CString::new(s).map_err(|_| ZpaqError::NulInString)?),
        None => None,
    };

    let reader = FfiReader::new(reader)?;
    let writer = FfiWriter::new(writer)?;

    let rc = unsafe {
        sys::zpaq_compress(
            reader.raw,
            writer.raw,
            method_c.as_ptr(),
            filename_c
                .as_ref()
                .map(|c| c.as_ptr())
                .unwrap_or(ptr::null()),
            comment_c
                .as_ref()
                .map(|c| c.as_ptr())
                .unwrap_or(ptr::null()),
            1,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(err_from_last())
    }
}

/// Decompresses a ZPAQ archive from `reader` and writes raw data to `writer`.
///
/// # Example
///
/// ```rust
/// use std::io::Cursor;
/// let compressed = zpaq_rs::compress_to_vec(b"hello", "1").unwrap();
/// let mut out = Vec::new();
/// zpaq_rs::decompress_stream(Cursor::new(&compressed), &mut out).unwrap();
/// assert_eq!(out, b"hello");
/// ```
pub fn decompress_stream<R: Read + Send, W: Write + Send>(reader: R, writer: W) -> Result<()> {
    clear_last_error();
    let reader = FfiReader::new(reader)?;
    let writer = FfiWriter::new(writer)?;
    let rc = unsafe { sys::zpaq_decompress(reader.raw, writer.raw) };
    if rc == 0 {
        Ok(())
    } else {
        Err(err_from_last())
    }
}

/// Derives a 32-byte key from `key32` and `salt32` using scrypt.
///
/// Uses libzpaq's fixed scrypt parameters: N = 16 384, r = 8, p = 1.
/// Both input arrays must be exactly 32 bytes.
///
/// This is the same key-stretching used by `zpaq` encrypted archives.
pub fn stretch_key(key32: [u8; 32], salt32: [u8; 32]) -> Result<[u8; 32]> {
    clear_last_error();
    let mut out = [0u8; 32];
    let rc = unsafe { sys::zpaq_stretch_key(out.as_mut_ptr(), key32.as_ptr(), salt32.as_ptr()) };
    if rc == 0 {
        Ok(out)
    } else {
        Err(err_from_last())
    }
}

/// Returns `len` cryptographically strong random bytes.
///
/// On Unix delegates to `/dev/urandom`; on Windows uses `CryptGenRandom`.
/// Returns [`ZpaqError::Ffi`] if the platform RNG is unavailable.
pub fn random_bytes(len: usize) -> Result<Vec<u8>> {
    clear_last_error();
    let mut buf = vec![0u8; len];
    let rc = unsafe { sys::zpaq_random(buf.as_mut_ptr(), len as c_int) };
    if rc == 0 {
        Ok(buf)
    } else {
        Err(err_from_last())
    }
}

/// Computes the SHA-1 digest of `bytes` using the libzpaq implementation.
///
/// Returns the 20-byte raw digest.  For new designs prefer [`sha256`];
/// SHA-1 is exposed because it is used internally by ZPAQ segment checksums.
pub fn sha1(bytes: &[u8]) -> Result<[u8; 20]> {
    clear_last_error();
    let s = unsafe { sys::zpaq_sha1_new() };
    if s.is_null() {
        return Err(err_from_last());
    }
    unsafe {
        sys::zpaq_sha1_write(s, bytes.as_ptr() as *const c_char, bytes.len() as i64);
    }
    let mut out = [0u8; 20];
    let rc = unsafe { sys::zpaq_sha1_result(s, out.as_mut_ptr()) };
    unsafe { sys::zpaq_sha1_free(s) };
    if rc == 0 {
        Ok(out)
    } else {
        Err(err_from_last())
    }
}

/// Computes the SHA-256 digest of `bytes` using the libzpaq implementation.
///
/// Returns the 32-byte raw digest.
pub fn sha256(bytes: &[u8]) -> Result<[u8; 32]> {
    clear_last_error();
    let s = unsafe { sys::zpaq_sha256_new() };
    if s.is_null() {
        return Err(err_from_last());
    }
    unsafe {
        for &b in bytes {
            sys::zpaq_sha256_put(s, b as c_int);
        }
    }
    let mut out = [0u8; 32];
    let rc = unsafe { sys::zpaq_sha256_result(s, out.as_mut_ptr()) };
    unsafe { sys::zpaq_sha256_free(s) };
    if rc == 0 {
        Ok(out)
    } else {
        Err(err_from_last())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_payloads() -> Vec<Vec<u8>> {
        vec![
            b"".to_vec(),
            b"hello zpaq".to_vec(),
            (0..1024).map(|i| (i * 31 % 251) as u8).collect(),
            (0..20_000)
                .map(|i| (i * 1315423911u64 as usize % 256) as u8)
                .collect(),
        ]
    }

    #[test]
    fn roundtrip_small() {
        let data = b"hello zpaq";
        let c = compress_to_vec(data, "1").expect("compress");
        let d = decompress_to_vec(&c).expect("decompress");
        assert_eq!(d, data);
    }

    #[test]
    fn roundtrip_random() {
        let data: Vec<u8> = (0..10_000).map(|i| (i * 31 % 251) as u8).collect();
        let c = compress_to_vec(&data, "2").expect("compress");
        let d = decompress_to_vec(&c).expect("decompress");
        assert_eq!(d, data);
    }

    #[test]
    fn compress_size_matches_vec_len() {
        for method in ["1", "2", "3", "4", "5", "x4.3ci1"] {
            for data in test_payloads() {
                let c = compress_to_vec(&data, method).expect("compress");
                let sz = compress_size(&data, method).expect("compress_size");
                assert_eq!(sz as usize, c.len(), "method={method}");
            }
        }
    }

    #[test]
    fn decompress_size_matches_output_len() {
        for method in ["1", "2", "x4.3ci1"] {
            for data in test_payloads() {
                let c = compress_to_vec(&data, method).expect("compress");
                let out = decompress_to_vec(&c).expect("decompress");
                let sz = decompress_size(&c).expect("decompress_size");
                assert_eq!(sz as usize, out.len(), "method={method}");
            }
        }
    }

    #[test]
    fn sha_vectors() {
        // "abc" test vectors
        let s1 = sha1(b"abc").expect("sha1");
        assert_eq!(hex::encode(s1), "a9993e364706816aba3e25717850c26c9cd0d89d");
        let s256 = sha256(b"abc").expect("sha256");
        assert_eq!(
            hex::encode(s256),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn random_bytes_len() {
        let b = random_bytes(256).expect("random_bytes");
        assert_eq!(b.len(), 256);
    }

    #[test]
    fn nul_in_method_errors() {
        let err = compress_stream(
            std::io::Cursor::new(b"hello"),
            Vec::<u8>::new(),
            "a\0b",
            None,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, ZpaqError::NulInString));
    }

    #[test]
    fn callback_error_propagates() {
        struct FailingReader;

        impl std::io::Read for FailingReader {
            fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("boom"))
            }
        }

        let err = compress_size_stream(FailingReader, "1", None, None).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("callback failed"));
    }
}

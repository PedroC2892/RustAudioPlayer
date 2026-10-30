# Progress

## M0 — Toolchain and Rust basics: DONE

Deliverable: CLI that takes a file path and prints its size (`cargo run -- Cargo.toml`).

### Understood
- `cargo init`, `check`, `build`, `run`, `clippy`, `fmt`; `Cargo.toml` plays the role of a Makefile.
- `std::env::args()` is an iterator (`Args`), not an array; `collect()` into `Vec<String>` consumes it.
- `Vec` implements `Index`, `Args` does not.
- `Option<T>` replaces `NULL` ("maybe a value, no reason for absence"): `Vec::get`.
- `Result<T, E>` replaces `-1` + `errno` ("value or a failure with a reason"): `fs::metadata`.
- A `Result` must be opened (`match`) before calling methods of the inner value.
- `match` arms can be blocks; the last expression without `;` is the arm's value.
- `&Path` / `PathBuf` mirror `&str` / `String` (borrowed vs owned).
- `fs::metadata` follows symlinks; asking the `Metadata` avoids a second `stat` (TOCTOU).

### Errors that taught something
- E0382: use after move (`collect()` takes `self`).
- E0107: wrong number of generic arguments (`Result` needs `<T, E>`).
- E0599: method not found on `Result` (the method lives on `Metadata`).
- E0608: cannot index into `Args`.
- E0277: `FileType` has no `Display`; `{:?}` uses `Debug`.

### Open doubts / polish
- Error message for a missing file still says "Invalid file_size"; should describe the failed action.
- Directory case prints the `Debug` form of `FileType`; prefer a human message.
- `Path::is_file` block was removed in favor of `Metadata::is_file`.

## M1 — Ownership, borrowing, errors: DONE

Deliverable: `main` returns `io::Result<()>`; no `exit(1)` left; missing file, directory and missing argument all reported through `Result`.
Gate: `cargo clippy -- -D warnings` and `cargo fmt --check` clean. `cargo audit` not installed; nothing to audit until the first dependency (M2).

### Understood
- `?` on `Ok(v)` unwraps `v` and continues; on `Err(e)` it returns `Err(e)` from the current function.
- `?` only compiles in a function returning `Result` (or `Option`), because it expands to an early `return`.
- A function that returns `Result` lets the caller decide how to handle the error; `exit` takes that decision away.
- `return Err(io::Error::new(ErrorKind::..., "msg"))` builds and returns an error of our own.
- `&str` accepts literals and `&String`; parameters that only read text should take `&str` (deref coercion).
- `String` owns heap bytes; `&str` is a borrowed view.

### Errors that taught something
- E0308: mismatched types (`Result<u64, _>` annotation after `?` unwrapped it).
- E0277: `?` in a function that does not return `Result`.
- `unused_imports` lint caught by `cargo check`/`clippy`.

### Open doubts / polish
- `Option::ok_or` to replace the missing-argument `match` (optional).
- A dedicated error type instead of reusing `io::Error` for usage errors (later).
- Move semantics and `&mut` only touched lightly; revisit in M5.

## M2 — Play audio from the terminal: DONE

Deliverable: `cargo run -- song.mp3` plays the file until it ends; a non-audio file ends with `Error: DecoderError(UnrecognizedFormat)`, no panic.
Gate: `cargo clippy -- -D warnings` clean; `cargo audit` run by the developer (output not recorded).

### Understood
- Adding a dependency with `cargo add`; reading crate docs on docs.rs and adapting snippets.
- `rodio` pipeline: file -> `Decoder` -> player/sink (queue and controls) -> `DeviceSink` (connection to the sound device).
- RAII: dropping the device sink stops the sound, so it must outlive playback; the compiler does not catch this.
- `Box<dyn Error>` as a common error type; `?` converts through `From`.
- `?` converts to the error type of the enclosing function (the `main` signature decided it).
- `Option::ok_or` turns `None` into an `Err`, so `?` can be used.
- `io::Result<T>` is an alias with the error fixed; `Result<T, E>` lets us choose.
- Clippy `needless_borrow`.

### Errors that taught something
- E0107 again (alias with one generic parameter).
- E0308: missing `Ok(())` tail expression.
- E0277: `?` could not convert `&str` into `io::Error` because `main` still returned `io::Result`.

### Open doubts / polish
- Usage message in `ok_or` is vague; the earlier "Usage: prog <file_path>" was clearer.
- ALSA/JACK noise on stderr is harmless backend probing.
- Why `Box` is needed around `dyn Error` (size known at compile time) only touched lightly.

## M3 — Duration and position: DONE (terminal loop printing position / total)

## Tutoring ended (2026-10-29)

The tutoring plan was stopped after M3 and the full GUI player (M4–M7) was implemented
directly. See `CLAUDE.md` for the current architecture. This file is kept as a record of
the Rust concepts learned in M0–M3.

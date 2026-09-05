# GMusic compatibility patch for block 0.1.6

## Provenance

The source, tests, and README come from the crates.io `block` 0.1.6 package by
Steven Sheldon. The package declares the MIT license in `Cargo.toml`.
The upstream repository is <https://github.com/SSheldon/rust-block>.
The published package does not include a separate license file; `LICENSE.txt`
provides the standard MIT notice with the package author's attribution.

GMusic uses this local package through `[patch.crates-io]`. Souvlaki 0.8.3 and
its Cocoa dependencies still require this API. Keep the patch local and pinned
rather than depending on an unreviewed moving Git branch.

## Changes

- Replace the empty `Class` enum with opaque `c_void`. An empty enum cannot
  describe an existing foreign static. Rust reports this as `uninhabited_static`
  and a future compatibility error.
- Use `ptr::addr_of!` for the runtime class address. Do not construct a reference
  to the opaque foreign object or read its value.
- Make the existing C ABI explicit on foreign declarations and callbacks.
  This removes `missing_abi` warnings without changing the calling convention.
- Set the original Rust edition explicitly to 2015.
- Remove the unpublished `test_utils` path from the dev dependency. Resolve the
  published `objc_test_utils` package through the included standalone lockfile.

The public API and block memory layout remain unchanged. Preserve the upstream
README rather than applying the app's Markdown formatting rules to it.

## Verification

Run from the repository root on macOS:

```sh
cargo rustc --locked --manifest-path src-tauri/Cargo.toml -p block -- -D warnings
cargo test --locked --manifest-path src-tauri/vendor/block/Cargo.toml
cargo check --locked --manifest-path src-tauri/Cargo.toml --all-targets --future-incompat-report
cargo test --locked --manifest-path src-tauri/Cargo.toml
```

The standalone suite exercises native block invocation, arguments, heap copies,
and stack-to-heap lifetime, plus the documentation examples. CI runs the same
package checks. Generated vendor build output must stay ignored.

## Removal

Remove this directory and the Cargo patch when Souvlaki no longer depends on the
legacy crate, or when an API-compatible upstream release fixes these warnings.
Update the lockfile and repeat the native media tests and compatibility report.

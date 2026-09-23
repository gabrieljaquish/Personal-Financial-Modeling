# packaging/

The inputs of the two release channels (`ARCHITECTURE.md` §3 and §9 item 5,
`DECISIONS.md` ADR-003, `SECURITY.md` §12). `cargo xtask dist` reads them; nothing here is
compiled.

| File | What it is |
|---|---|
| `Info.plist` | Template of the `.app` wrapper's `Contents/Info.plist`. `dist` substitutes every `@NAME@` placeholder from `crates/pfp-app/src/identity.rs` (identifier, executable and bundle name), the workspace version and the deployment target, drops the template's comments, and refuses to package if a placeholder is left |
| `entitlements.plist` | The executable's entitlements: **the empty set**. It is not passed to `codesign`: no entitlements option and an empty dictionary mean the same thing. A test in `xtask` fails if a key is ever added. Adding one needs an ADR first |
| `README.md` | This file: the wrapper, the DMG layout and what is still missing |

## The application identifier is a placeholder

`APP_IDENTIFIER` in `crates/pfp-app/src/identity.rs` is the only place the identifier is
written. Its current value is a **placeholder** and must be replaced before the first signed
release: a Developer ID signature binds it into the code-signing requirement, and every
installed copy names its state directory after it (`~/Library/Application Support/<identifier>`).
Nothing has been signed with an identity or installed anywhere yet, so changing it now costs
one line.

## The `.app` wrapper (three files at most)

```text
pfp.app/
  Contents/
    Info.plist        CFBundleIdentifier, CFBundleExecutable, LSUIElement = true, LSMinimumSystemVersion
    MacOS/pfp         the executable, byte-identical to the bare channel's
    Resources/*.icns  NOT YET: there is no icon (see below)
```

The wrapper is a notarization container and a Finder launcher, nothing else (ADR-003 guard
clause): no second executable, no bundled runtime, no WebView, no native window, no Dock icon.

**Icon.** None exists yet, so `CFBundleIconFile` is absent and Finder shows the generic
application icon. Adding one means `Contents/Resources/<name>.icns`, the `CFBundleIconFile` key
in the template, and a resource that the bundle's signature must seal.

## Order of operations

`cargo xtask dist` builds, records the SHA-256 of the executable **before `codesign`**, signs
it **once** (ad hoc: `codesign --sign - --options runtime --timestamp=none --identifier
<identifier>`; no identity, no keychain, no entitlements), and then packages **both channels
from that one signed file**:

- `pfp-<version>-<label>.tar.gz`: the bare executable, `LICENSE`, `NOTICE`;
- `pfp-<version>-<label>-app.tar.gz` and the `app/` directory: `pfp.app`, `LICENSE`, `NOTICE`.

Every archive is deterministic: members sorted, owner `0:0` with no names, every mtime
`SOURCE_DATE_EPOCH`, modes `0755`/`0644`, plain ustar (no pax headers, extended attributes or
AppleDouble files), `gzip -n`. `<label>` is `macos-universal-adhoc` only for a `lipo` of both
targets; a one-target build is `macos-arm64-single-arch-adhoc` (or `x86_64`) everywhere.

**Known result: the bundle does not verify with that order.** `codesign --verify --strict
pfp.app` reports *code has no resources but signature indicates they must be present*. A
bundle's main executable is normally signed *as the bundle*, which seals `Info.plist` and a
`_CodeSignature/CodeResources` into the executable's signature; an executable carrying that
seal then fails `codesign --verify` on its own (*invalid Info.plist*). So "one byte-identical
signed executable in both channels" and "both channels verify" cannot both hold. `dist`
records the result in `dist-manifest.json` (`signing.verify_app_bundle`) and does not gate on
it; which property to keep is a decision for the signing block (for example: the
reproducibility claim is made on the **pre-codesign** digest, and the bundle's copy is signed
as a bundle).

## Reproducing a build

The reproducibility digest is the executable's **pre-codesign** SHA-256 in
`dist-manifest.json` (`executable.reproducibility_digest`); the ad-hoc signature over it is
deterministic, so the signed file and both tarballs follow. To rebuild it:

- check out the commit in `source.commit` (any directory; the path is remapped to `/pfp`);
- use the Rust toolchain in `toolchain.rustc`, the same macOS SDK (`toolchain.macos_sdk_in_binary`)
  and linker, and the same `gzip` (the tarballs' compressed bytes depend on it);
- run `cargo xtask dist` with the same targets and `--source-date-epoch` equal to
  `source.source_date_epoch`. `$CARGO_HOME` and the target directory may be anywhere: both are
  remapped (`/cargo`, `/target`).

**No rustc wrapper, so no `cargo-auditable`.** Cargo hashes the *absolute path* of
`RUSTC_WORKSPACE_WRAPPER` into the `-C metadata` of every workspace crate, and `cargo
auditable build` sets that wrapper to its own install path. With it, the executable's symbols,
code, `LC_UUID` and signature depended on where the tool was installed (by default
`/Users/<user>/.cargo/bin`), so a build could not be reproduced from another account; a
version pin does not pin the bytes. No install path is the same for every verifier without
`sudo` or a world-writable directory, and the dependency list it embedded over-reported (it
named crates that Cargo's feature unification lists but the release never compiles). The
CycloneDX SBOM from `cargo xtask sbom` is the dependency record instead. `dist` sets
`RUSTC_WRAPPER` and `RUSTC_WORKSPACE_WRAPPER` to the empty string (which also overrides any
Cargo configuration file), and refuses an executable that carries the `__DATA,.dep-v0` section
`cargo-auditable` writes.

The CI rebuild job (`.github/workflows/release.yml`) varies the checkout path, the target
directory and `$CARGO_HOME`, on the same runner image and account. A difference that only
another account, SDK, Xcode or `gzip` would cause is outside what it can see.

## The DMG (CI only)

The DMG is built **only** by `.github/workflows/release.yml`, never on a developer machine:

```text
<volume "pfp-<version>-<label>">
  pfp.app
  Applications -> /Applications
  LICENSE
  NOTICE
```

It is made from the `app/` directory that `dist` produced (and already scanned) plus the
`/Applications` link, with `hdiutil create -fs HFS+ -format UDZO`, then `hdiutil verify`.
The DMG, the SBOMs and the release `SHA256SUMS` over everything are written to a separate
release directory (`release-<label>`), never into `dist`'s own output (`dist-<label>`), so the
rebuild job compares exactly what `dist` produced and a perfect rebuild reports
byte-identical.
`cargo xtask check-magic` cannot read a DMG, so it runs over that input directory immediately
before `hdiutil`; the link is not followed. There is no background image and no Finder window
layout: a layout means a Finder-written `.DS_Store`, which needs scripting Finder, which this
pipeline does not do. A DMG is not reproducible (`hdiutil` writes identifiers and times into
it); the reproducibility claim is the executable's pre-codesign digest.

## Not here yet (the signing block, `PLAN.md` §4.1)

Developer ID signing, notarization and stapling scripts, the stapling experiment (ADR-003
item 4), `spctl` / `stapler validate`, and the Ed25519 signature over `SHA256SUMS`
(`cargo xtask sign-checksums` fails closed until then). No signing identity, certificate or key
of any kind belongs in this directory, now or later: credentials live in a protected CI
environment (`SECURITY.md` §12).

# Security Policy

## Supported versions

Vellum is **pre-alpha** (Increment 0). No release is yet supported for production use, and
there are no security guarantees during this phase. Once a stable line is published, this
table will list supported versions.

| Version | Supported |
| ------- | --------- |
| 0.0.x   | ⚠️ pre-alpha, no guarantees |

## Reporting a vulnerability

Please report security issues **privately** — do not open a public issue.

Email **security@akaisys.com** with:

- a description of the issue and its impact,
- steps to reproduce (proof-of-concept if possible),
- affected crate/package and version or commit.

We aim to acknowledge reports within a few business days and will keep you informed of
remediation progress. Responsible disclosure is appreciated; we will credit reporters who
wish to be named once a fix is released.

## Security posture

The `core` crate is `#![forbid(unsafe_code)]` and the project minimizes dependencies by
design. Supply-chain checks (`cargo deny`, `cargo audit`) run in CI.

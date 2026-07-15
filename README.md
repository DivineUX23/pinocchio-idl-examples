# pinocchio-idl-examples

Example Pinocchio programs annotated with [pinocchio-idl](https://github.com/DivineUX23/pinocchio-idl).

Each subdirectory is a self-contained Cargo crate demonstrating real-world usage of the `#[p_instruction]`, `#[p_state]`, `#[p_constant]`, and `#[p_error]` proc-macro attributes, as well as the `pinocchio-idl build` CLI for IDL generation.

---

## Examples

| Program | Description |
|---|---|
| [`pinocchio-fundraiser`](pinocchio-fundraiser/) | A token fundraising program with initialize, contribute, checker, and refund instructions. Demonstrates PDA derivation with explicit bump seeds, ATA constraints, multi-instruction state management, and time-based logic. |
| [`pinocchio-escrow`](pinocchio-escrow/) | A clean, minimalist token escrow program with make, take, and refund instructions. Demonstrates simple state initialization, exchange routing, and account closure. |

---

## Prerequisites

- Rust toolchain with the `sbf-solana-solana` or `bpf-unknown-unknown` target
- `pinocchio-idl` CLI installed:

```bash
cargo install --git https://github.com/DivineUX23/pinocchio-idl.git pinocchio-idl-cli
```

---

## Generating an IDL

From inside any example directory:

```bash
pinocchio-idl build
```

This produces `idl.json` in the current directory, compatible with both the Anchor IDL specification and [Codama](https://github.com/codama-idl/codama).

---

## Repository Layout

```text
pinocchio-idl-examples/
├── pinocchio-fundraiser/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── constants.rs
│       ├── state/
│       │   ├── mod.rs
│       │   ├── fundraiser.rs
│       │   └── contributor.rs
│       └── instructions/
│           ├── mod.rs
│           ├── initialize.rs
│           ├── contribute.rs
│           ├── checker.rs
│           └── refund.rs
└── pinocchio-escrow/
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── constants.rs
        ├── error.rs
        ├── state/
        │   ├── mod.rs
        │   └── escrow.rs
        └── instructions/
            ├── mod.rs
            ├── make.rs
            ├── take.rs
            └── refund.rs
```

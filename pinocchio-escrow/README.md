# pinocchio-escrow

A clean, minimalist Escrow Solana program built with [Pinocchio](https://github.com/febo/pinocchio) and annotated with [pinocchio-idl](https://github.com/DivineUX23/pinocchio-idl) for IDL generation.

This program serves as a reference implementation demonstrating the use of `#[p_instruction]`, `#[p_state]`, and `#[p_constant]` attributes across instructions that initialize, execute, and refund an escrow exchange.

---

## Program Design

The escrow exchange flow operates as follows:
- **`make`**: The Maker initializes an `Escrow` account and deposits tokens of `mint_a` into a Program-Owned vault.
- **`take`**: A Taker executes the exchange by sending the requested tokens of `mint_b` directly to the Maker, receiving the deposited `mint_a` tokens from the Program vault in return. The Escrow state is closed.
- **`refund`**: The Maker cancels the exchange prior to execution, retrieving their deposited `mint_a` tokens from the Program vault. The Escrow state is closed.

### Instructions

| ID | Name     | Description |
|----|----------|-------------|
| 0  | `make`   | Initializes the escrow account and deposits tokens into the vault. |
| 1  | `take`   | Completes the trade, routing requested tokens to the maker and vault tokens to the taker. |
| 2  | `refund` | Cancels the escrow and returns the maker's deposited tokens. |

### Accounts

| Account   | Type      | Description |
|-----------|-----------|-------------|
| `Escrow`  | PDA State | Stores maker, mints, requested amounts, and derivation seeds. |

---

## Usage

### Build and IDL Generation

To compile the program and generate the Anchor-compatible IDL, run:

```bash
cargo build-sbf
pinocchio-idl generate
```

This creates a structured `idl.json` in the root of the workspace.

---

## Architecture and Structure

```text
pinocchio-escrow/
├── Cargo.toml
└── src/
    ├── lib.rs              # Program entrypoint and instruction router
    ├── constants.rs        # Program constants (e.g., seed prefixes)
    ├── error.rs            # Custom program error definitions
    ├── state/
    │   ├── mod.rs
    │   └── escrow.rs       # Escrow state layout (#[p_state])
    └── instructions/
        ├── mod.rs          # EscrowInstruction enum mapping
        ├── make.rs         # Make instruction handler
        ├── take.rs         # Take instruction handler
        └── refund.rs       # Refund instruction handler
```

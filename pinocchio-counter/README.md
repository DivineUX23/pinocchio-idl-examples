# pinocchio-counter

A minimal state-management program built with [Pinocchio](https://github.com/febo/pinocchio) and annotated with [pinocchio-idl](https://github.com/DivineUX23/pinocchio-idl) for IDL generation.

This program demonstrates fundamental PDA state initialization and mutation using `#[p_instruction]`, `#[p_state]`, and custom `#[p_error]` definitions.

---

## Program Design

The counter program allows users to create their own counter and mutate its state:
- **`initialize`**: An authority creates a `Counter` PDA bound to their address, initializing the count to zero.
- **`increment`**: The authority increments their counter by 1.
- **`decrement`**: The authority decrements their counter by 1, with underflow protection.

### Instructions

| ID | Name         | Description |
|----|--------------|-------------|
| 0  | `initialize` | Creates a new counter PDA for the signing authority. |
| 1  | `increment`  | Adds 1 to the counter value. |
| 2  | `decrement`  | Subtracts 1 from the counter value. |

### Accounts

| Account   | Type      | Description |
|-----------|-----------|-------------|
| `Counter` | PDA State | Stores the authority, the current count, and the derivation bump. |

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
pinocchio-counter/
├── Cargo.toml
└── src/
    ├── lib.rs              # Program entrypoint and instruction router
    ├── error.rs            # Custom program error definitions (#[p_error])
    ├── state.rs            # Counter state layout (#[p_state])
    └── instructions/
        ├── mod.rs          # CounterInstruction enum mapping
        ├── initialize.rs   # Initialize instruction handler
        ├── increment.rs    # Increment instruction handler
        └── decrement.rs    # Decrement instruction handler
```

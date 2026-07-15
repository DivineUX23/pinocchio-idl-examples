# pinocchio-fundraiser

A token fundraising program built with [Pinocchio](https://github.com/febo/pinocchio) and annotated with [pinocchio-idl](https://github.com/DivineUX23/pinocchio-idl) for IDL generation.

This program demonstrates how to use `#[p_instruction]`, `#[p_state]`, and `#[p_constant]` across a multi-instruction Pinocchio program with PDA accounts, ATA constraints, token transfers, and time-based business logic.

---

## Program Overview

A maker creates a fundraiser campaign targeting a specific token and amount. Contributors deposit tokens within the campaign window. Once the target is met, the maker can claim the funds. If the window expires without the target being met, contributors can reclaim their deposits.

### Instructions

| ID | Name          | Description |
|----|---------------|-------------|
| 0  | `initialize`  | Creates the fundraiser PDA and its vault ATA. |
| 1  | `contribute`  | Transfers tokens from a contributor into the vault. |
| 2  | `checker`     | Verifies the target is met and pays out to the maker. |
| 3  | `refund`      | Returns a contributor's tokens after campaign expiry. |

### Accounts

| Account              | Type        | Description |
|----------------------|-------------|-------------|
| `Fundraiser`         | PDA state   | Campaign metadata: maker, mint, target, current total, duration. |
| `Contributor`        | PDA state   | Per-contributor total contribution tracking. |

### Constants

| Name                        | Value  | Description |
|-----------------------------|--------|-------------|
| `MIN_AMOUNT_TO_RAISE`       | 3      | Minimum raise target (in token base units, before decimal scaling). |
| `SECONDS_TO_DAYS`           | 86400  | Conversion factor from seconds to days. |
| `MAX_CONTRIBUTION_PERCENTAGE` | 10  | Maximum contribution size as a percentage of the target. |
| `PERCENTAGE_SCALER`         | 100    | Divisor for percentage calculations. |

---

## Usage with pinocchio-idl

### 1. Add the dependency

```toml
[dependencies]
pinocchio-idl-macros = { git = "https://github.com/DivineUX23/pinocchio-idl.git", branch = "main" }
```

### 2. Build and generate the IDL

```bash
cargo build-sbf
pinocchio-idl generate
```

This produces `idl.json` in the project root.

---

## Key Patterns Demonstrated

### Explicit PDA bump as a seed

`pinocchio-idl` does not perform bump-search validation. The canonical bump must be stored in instruction data and passed explicitly in `pda = [...]`:

```rust
#[p_instruction(
    id = 0,
    accounts = [
        fundraiser(mut, pda = [b"fundraiser", maker, bump], state = Fundraiser),
        ...
    ],
    data = [
        ...,
        bump: u8 = data[17]
    ]
)]
```

### ATA constraints

The `ata = [owner, mint]` constraint records the ATA relationship in the IDL and validates owner/mint on-chain. It requires exactly two expressions:

```rust
vault(mut, ata = [fundraiser, mint_to_raise])
contributor_ata(mut, ata = [contributor, mint_to_raise])
```

### Account extraction ordering

All account bindings must be extracted contiguously before any other logic. Interleaving other statements between bindings will cause the macro to inject validation guards at the wrong point:

```rust
pub fn process_initialize_instruction(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    // All bindings first
    let [maker, mint_to_raise, fundraiser, vault, system_program, token_program, _] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    // Business logic follows
    ...
}
```

### Range-based data field extraction

Range syntax (`data[start..end]`) is preferred over single-index syntax (`data[N]`) because it returns a graceful `ProgramError::InvalidArgument` on short input rather than panicking:

```rust
data = [
    amount_to_raise: u64 = data[0..8],   // safe
    time_started:    i64 = data[8..16],  // safe
    duration:        u8  = data[16],     // single-index — panics on short input
    bump:            u8  = data[17]      // single-index — panics on short input
]
```

---

## Project Structure

```text
pinocchio-fundraiser/
├── Cargo.toml
└── src/
    ├── lib.rs              # Program entrypoint and instruction dispatch
    ├── constants.rs        # #[p_constant] definitions
    ├── state/
    │   ├── mod.rs
    │   ├── fundraiser.rs   # Fundraiser #[p_state] struct
    │   └── contributor.rs  # Contributor #[p_state] struct
    └── instructions/
        ├── mod.rs          # FundraiserInstructions enum
        ├── initialize.rs   # Instruction 0
        ├── contribute.rs   # Instruction 1
        ├── checker.rs      # Instruction 2
        └── refund.rs       # Instruction 3
```

# Personal Financial Modeling

A self-hosted **financial planning application** that mirrors what professional
financial-planner software does. Download a release, enter your own information, and run it
locally. Nothing about any user ships with the code.

The focus is **planning, not tracking**. Budgeting and account-tracking tools already answer
"what do I have today?". This application answers forward-looking questions:

- Where should the next dollar of savings go — 401(k), Roth vs Traditional, IRA, HSA, taxable, or debt?
- How can lifetime taxes be minimized — bracket management, Roth conversions, asset location?
- Pay down debt or invest?
- How should investments be reallocated, and what does that cost in taxes?
- What does "enough saved" look like, and how confident can you be? (Monte Carlo / probabilistic modeling)

## Status

Early planning. No application code yet.

## How it is meant to work

1. Download a release (or build from source).
2. On first run, a setup flow collects your household, accounts, income, goals and assumptions.
3. Everything you enter is stored in an **encrypted file on your own machine**.
4. Explore projections, scenarios and recommendations in a browser UI served locally.

## Design principles

**The repository is the application only.** No user's information, profile, selections or
results are ever committed. Everything personal is a runtime input.

**Privacy model (non-negotiable).**

- No financial information is ever stored in this repository — no balances, holdings,
  transactions or plan parameters, and no keys, tokens or passwords for financial institutions.
- User data lives only in a local encrypted store, encrypted **at rest and in transit**
  (including browser ↔ local backend).
- Institution credentials (for the later account-connection phase) live in the OS keychain or
  the encrypted local store — never in the repo, never in env files inside the repo tree.
- The repo contains only code, docs, and clearly-labeled **synthetic** fixtures.
- `.gitignore` blocks data directories, database/encrypted files, finance export formats and
  secret material. A secret scanner (gitleaks) is run before commits.

**Transparent assumptions.** Every rate, table and default is visible, sourced and overridable.

## Planned capabilities

| Area | Direction |
|---|---|
| Households | Single or two-person households; ages, filing status, retirement dates and survivor scenarios are user inputs |
| Taxes | US federal + state (user selects the state) |
| Accounts | 401(k)/403(b), Traditional and Roth IRA, HSA, 529, taxable, cash, debts |
| Modeling | Year-by-year projection, side-by-side scenarios, Monte Carlo |
| Data input | Manual entry and CSV/OFX import first; direct institution connections later through a connector interface |
| Front end | Web-based, served locally |
| Backend | To be decided (Rust, Go, C++ or Python) |

> Nothing in this repository is financial advice.

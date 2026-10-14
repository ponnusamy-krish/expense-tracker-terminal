# expense-tracker-terminal

A small interactive expense tracker for the terminal, written in Rust. Expenses are saved to a CSV file, so they persist between runs.

## Run

```sh
cargo run                    # stores data in ./expenses.csv
cargo run -- ~/money.csv     # use a different data file
```

## Features

- Add an expense with amount, date (defaults to today), category and an optional description
- List all expenses with a running total
- Summary of totals per category
- Delete an expense by ID

## Data file

Plain CSV with a header row:

```csv
id,date,category,amount,description
1,2026-10-14,food,12.50,"lunch, with friends"
```

## Test

```sh
cargo test
```

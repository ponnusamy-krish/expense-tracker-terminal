mod expense;

use std::env;
use std::io::{self, Write};
use std::process;

use expense::{format_amount, parse_amount, parse_date, today, Store};

const DEFAULT_FILE: &str = "expenses.csv";

fn main() -> io::Result<()> {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_FILE.to_string());
    let mut store = Store::open(&path).unwrap_or_else(|err| {
        eprintln!("Error: {err}");
        process::exit(1);
    });

    println!("Expense Tracker (data file: {path})");
    loop {
        print_menu();
        let choice = match prompt("Choose an option: ") {
            Ok(choice) => choice,
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err),
        };

        let outcome = match choice.as_str() {
            "1" => add_expense(&mut store),
            "2" => {
                list_expenses(&store);
                Ok(())
            }
            "3" => {
                show_summary(&store);
                Ok(())
            }
            "4" => delete_expense(&mut store),
            "q" | "Q" => break,
            _ => {
                println!("Unknown option.");
                Ok(())
            }
        };
        if let Err(err) = outcome {
            println!("Error: {err}");
        }
    }

    println!("Bye!");
    Ok(())
}

fn print_menu() {
    println!("\n=== Expense Tracker ===");
    println!("1) Add expense");
    println!("2) List expenses");
    println!("3) Summary by category");
    println!("4) Delete expense");
    println!("q) Quit");
}

/// Prints `label` and reads one trimmed line. Returns UnexpectedEof when stdin is closed.
fn prompt(label: &str) -> io::Result<String> {
    print!("{label}");
    io::stdout().flush()?;
    let mut line = String::new();
    if io::stdin().read_line(&mut line)? == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "input closed"));
    }
    Ok(line.trim().to_string())
}

fn add_expense(store: &mut Store) -> io::Result<()> {
    let amount_cents = loop {
        match parse_amount(&prompt("Amount (e.g. 12.50): ")?) {
            Ok(cents) if cents > 0 => break cents,
            Ok(_) => println!("Amount must be greater than zero."),
            Err(err) => println!("{err}"),
        }
    };

    let date = loop {
        let input = prompt("Date (YYYY-MM-DD, Enter for today): ")?;
        if input.is_empty() {
            break today();
        }
        match parse_date(&input) {
            Ok(date) => break date,
            Err(err) => println!("{err}"),
        }
    };

    let mut category = prompt("Category (Enter for 'other'): ")?.to_lowercase();
    if category.is_empty() {
        category = "other".to_string();
    }
    let description = prompt("Description (optional): ")?;

    let id = store.add(date, category, amount_cents, description)?;
    println!("Added expense #{id}.");
    Ok(())
}

fn list_expenses(store: &Store) {
    let expenses = store.expenses();
    if expenses.is_empty() {
        println!("No expenses yet.");
        return;
    }

    println!(
        "{:>4}  {:<10}  {:<15}  {:>10}  DESCRIPTION",
        "ID", "DATE", "CATEGORY", "AMOUNT"
    );
    for e in expenses {
        println!(
            "{:>4}  {:<10}  {:<15}  {:>10}  {}",
            e.id,
            e.date,
            e.category,
            format_amount(e.amount_cents),
            e.description
        );
    }
    println!(
        "{:>4}  {:<10}  {:<15}  {:>10}",
        "",
        "",
        "TOTAL",
        format_amount(store.total_cents())
    );
}

fn show_summary(store: &Store) {
    if store.expenses().is_empty() {
        println!("No expenses yet.");
        return;
    }

    println!("{:<15}  {:>10}", "CATEGORY", "TOTAL");
    for (category, cents) in store.totals_by_category() {
        println!("{:<15}  {:>10}", category, format_amount(cents));
    }
    println!(
        "{:<15}  {:>10}",
        "TOTAL",
        format_amount(store.total_cents())
    );
}

fn delete_expense(store: &mut Store) -> io::Result<()> {
    if store.expenses().is_empty() {
        println!("Nothing to delete.");
        return Ok(());
    }

    list_expenses(store);
    let input = prompt("ID to delete (Enter to cancel): ")?;
    if input.is_empty() {
        return Ok(());
    }
    let Ok(id) = input.parse::<u32>() else {
        println!("'{input}' is not a valid ID.");
        return Ok(());
    };

    if store.delete(id)? {
        println!("Deleted expense #{id}.");
    } else {
        println!("No expense with ID {id}.");
    }
    Ok(())
}

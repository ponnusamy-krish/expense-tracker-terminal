//! Expense storage: a CSV file on disk plus helpers for parsing and formatting values.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use chrono::{Local, NaiveDate};

const HEADER: &str = "id,date,category,amount,description";

#[derive(Debug, Clone, PartialEq)]
pub struct Expense {
    pub id: u32,
    pub date: String,
    pub category: String,
    pub amount_cents: i64,
    pub description: String,
}

/// All expenses, kept in memory and written back to disk after every change.
pub struct Store {
    path: PathBuf,
    expenses: Vec<Expense>,
}

impl Store {
    /// Loads expenses from `path`. A missing file is treated as an empty store.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path.into();
        let expenses = match fs::read_to_string(&path) {
            Ok(text) => parse_csv(&text)?,
            Err(err) if err.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(err) => return Err(format!("cannot read {}: {err}", path.display())),
        };
        Ok(Store { path, expenses })
    }

    pub fn expenses(&self) -> &[Expense] {
        &self.expenses
    }

    pub fn add(
        &mut self,
        date: String,
        category: String,
        amount_cents: i64,
        description: String,
    ) -> io::Result<u32> {
        let id = self.expenses.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        self.expenses.push(Expense {
            id,
            date,
            category,
            amount_cents,
            description,
        });
        self.save()?;
        Ok(id)
    }

    /// Returns `true` if an expense with `id` existed and was removed.
    pub fn delete(&mut self, id: u32) -> io::Result<bool> {
        let before = self.expenses.len();
        self.expenses.retain(|e| e.id != id);
        let removed = self.expenses.len() < before;
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    pub fn total_cents(&self) -> i64 {
        self.expenses.iter().map(|e| e.amount_cents).sum()
    }

    pub fn totals_by_category(&self) -> BTreeMap<&str, i64> {
        let mut totals = BTreeMap::new();
        for e in &self.expenses {
            *totals.entry(e.category.as_str()).or_insert(0) += e.amount_cents;
        }
        totals
    }

    /// Writes to a temp file and renames it over the real one, so a crash mid-write
    /// cannot leave a half-written data file behind.
    fn save(&self) -> io::Result<()> {
        let mut out = format!("{HEADER}\n");
        for e in &self.expenses {
            out.push_str(&format!(
                "{},{},{},{},{}\n",
                e.id,
                e.date,
                csv_escape(&e.category),
                format_amount(e.amount_cents),
                csv_escape(&e.description),
            ));
        }
        let tmp = PathBuf::from(format!("{}.tmp", self.path.display()));
        fs::write(&tmp, out)?;
        fs::rename(&tmp, &self.path)
    }
}

/// Parses "12", "12." , "12.5" or "12.50" into cents.
pub fn parse_amount(input: &str) -> Result<i64, String> {
    let text = input.trim();
    let invalid = || format!("invalid amount '{text}', use a number like 12 or 12.50");
    let (whole, frac) = text.split_once('.').unwrap_or((text, ""));
    let is_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());

    if (whole.is_empty() && frac.is_empty())
        || !is_digits(whole)
        || !is_digits(frac)
        || frac.len() > 2
    {
        return Err(invalid());
    }

    let dollars: i64 = if whole.is_empty() {
        0
    } else {
        whole
            .parse()
            .map_err(|_| "amount is too large".to_string())?
    };
    // "5" -> "50" cents, "" -> "00". Digits are validated above, so parsing cannot fail.
    let cents: i64 = format!("{frac:0<2}").parse().unwrap_or(0);

    dollars
        .checked_mul(100)
        .and_then(|v| v.checked_add(cents))
        .ok_or_else(|| "amount is too large".to_string())
}

pub fn format_amount(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}

/// Accepts YYYY-MM-DD and returns it normalised, rejecting impossible dates like 2023-02-29.
pub fn parse_date(input: &str) -> Result<String, String> {
    let text = input.trim();
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .map(|date| date.to_string())
        .map_err(|_| format!("invalid date '{text}', use YYYY-MM-DD"))
}

/// Today's date in the local timezone as YYYY-MM-DD.
pub fn today() -> String {
    Local::now().date_naive().to_string()
}

fn parse_csv(text: &str) -> Result<Vec<Expense>, String> {
    let mut expenses = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_no = index + 1;
        if index == 0 {
            if line.trim() != HEADER {
                return Err(format!("line 1: expected header '{HEADER}'"));
            }
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }

        let fields = split_csv_line(line).map_err(|err| format!("line {line_no}: {err}"))?;
        let [id, date, category, amount, description] = fields.as_slice() else {
            return Err(format!(
                "line {line_no}: expected 5 fields, found {}",
                fields.len()
            ));
        };
        expenses.push(Expense {
            id: id
                .parse::<u32>()
                .map_err(|_| format!("line {line_no}: invalid id '{id}'"))?,
            date: date.clone(),
            category: category.clone(),
            amount_cents: parse_amount(amount).map_err(|err| format!("line {line_no}: {err}"))?,
            description: description.clone(),
        });
    }
    Ok(expenses)
}

/// Splits one CSV line, honouring double-quoted fields and doubled quotes ("") inside them.
fn split_csv_line(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;

    while let Some(c) = chars.next() {
        match (in_quotes, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            (true, '"') => in_quotes = false,
            (true, _) => field.push(c),
            (false, '"') if field.is_empty() => in_quotes = true,
            (false, ',') => fields.push(std::mem::take(&mut field)),
            (false, _) => field.push(c),
        }
    }

    if in_quotes {
        return Err("unterminated quoted field".to_string());
    }
    fields.push(field);
    Ok(fields)
}

fn csv_escape(field: &str) -> String {
    if field.contains([',', '"']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_amounts_into_cents() {
        assert_eq!(parse_amount("12"), Ok(1200));
        assert_eq!(parse_amount("12."), Ok(1200));
        assert_eq!(parse_amount("12.5"), Ok(1250));
        assert_eq!(parse_amount("12.50"), Ok(1250));
        assert_eq!(parse_amount(".75"), Ok(75));
        assert_eq!(parse_amount(" 0.05 "), Ok(5));
    }

    #[test]
    fn rejects_bad_amounts() {
        for bad in ["", ".", "abc", "-5", "1.234", "1,50", "1.2.3"] {
            assert!(parse_amount(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn formats_cents() {
        assert_eq!(format_amount(1250), "12.50");
        assert_eq!(format_amount(5), "0.05");
        assert_eq!(format_amount(0), "0.00");
    }

    #[test]
    fn validates_dates() {
        assert_eq!(parse_date("2024-02-29"), Ok("2024-02-29".to_string()));
        assert_eq!(parse_date("2024-1-5"), Ok("2024-01-05".to_string()));
        assert!(parse_date("2023-02-29").is_err());
        assert!(parse_date("2024-13-01").is_err());
        assert!(parse_date("yesterday").is_err());
        assert!(parse_date(&today()).is_ok());
    }

    #[test]
    fn splits_quoted_csv_fields() {
        let fields =
            split_csv_line(r#"1,2024-01-05,food,12.50,"lunch, with ""friends""""#).unwrap();
        assert_eq!(fields.len(), 5);
        assert_eq!(fields[4], r#"lunch, with "friends""#);
        assert!(split_csv_line(r#"1,"unterminated"#).is_err());
    }

    #[test]
    fn store_round_trips_through_disk() {
        let path = std::env::temp_dir().join(format!("expense-test-{}.csv", std::process::id()));
        let _ = fs::remove_file(&path);

        let mut store = Store::open(path.clone()).unwrap();
        store
            .add(
                "2024-01-05".into(),
                "food".into(),
                1250,
                r#"lunch, with "friends""#.into(),
            )
            .unwrap();
        store
            .add("2024-01-06".into(), "travel".into(), 500, String::new())
            .unwrap();

        let reopened = Store::open(path.clone()).unwrap();
        assert_eq!(reopened.expenses(), store.expenses());
        assert_eq!(reopened.total_cents(), 1750);
        assert_eq!(reopened.totals_by_category().get("travel"), Some(&500));

        fs::remove_file(&path).unwrap();
    }
}

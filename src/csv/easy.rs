use crate::errors::{CsvError, Result};
use crate::Position;
use std::io::BufRead;

/// Read valid CSV one line at a time.
pub fn fast_stream_valid_csv<R: BufRead>(
    reader: R,
    delimiter: char,
    quote: char,
) -> impl Iterator<Item = Result<Vec<String>>> {
    reader.lines().into_iter().map(move |line_result| {
        let line = line_result?;
        let mut chars = line.chars().peekable();
        let mut row: Vec<String> = Vec::new();
        let mut current_field = String::new();
        let mut within_quotes = false;

        while let Some(ch) = chars.next() {
            if ch == quote {
                if within_quotes && chars.peek() == Some(&quote) {
                    // Two quotes in a row inside a quoted field means a literal quote
                    current_field.push(quote);
                    chars.next();
                } else {
                    within_quotes = !within_quotes;
                }
            } else if ch == delimiter && !within_quotes {
                row.push(current_field);
                current_field = String::new();
            } else {
                current_field.push(ch as char);
            }
        }
        row.push(current_field);

        Ok(row)
    })
}

/// Read CSV and handle unescaped delimiters in one field.
///
/// This requires that there are no unexpected newlines.
/// If you have both unescaped delimiters and unexpected newlines, then parsing will be ambiguous
/// so there are no deterministic solutions.
pub fn fast_stream_csv_with_unescaped_delimiters<R: BufRead>(
    reader: R,
    delimiter: char,
    quote: char,
    invalid_column_index: usize,
    expected_column_count: usize,
) -> impl Iterator<Item = Result<Vec<String>>> {
    fast_stream_valid_csv(reader, delimiter, quote)
        .enumerate()
        .map(move |(line, row_result)| {
            let row = row_result?;
            let apparent_column_count = row.len();
            if apparent_column_count < expected_column_count {
                return Err(CsvError::Invalid(
                    Position {
                        line,
                        column: expected_column_count,
                    },
                    "Not enough columns. There may be an unescaped newline in a field.",
                ));
            } else if apparent_column_count > expected_column_count {
                // Combine the extra columns into the invalid column
                let mut row = row.into_iter();
                // First take the valid columns on the left
                let new_row = row.by_ref().take(invalid_column_index).collect::<Vec<_>>();
                // Then take the invalid columns and join them with the delimiter
                let invalid_column = row
                    .by_ref()
                    .take(apparent_column_count - expected_column_count + 1)
                    .collect::<Vec<_>>()
                    .join(&delimiter.to_string());
                // Then take the valid columns on the right
                let new_row = new_row
                    .into_iter()
                    .chain(std::iter::once(invalid_column))
                    .chain(row)
                    .collect::<Vec<_>>();
                return Ok(new_row);
            }
            Ok(row)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_fast_stream_valid_csv() {
        let input = Cursor::new("a,b,c\n1,2,3\n4,5,6");
        let mut iter = fast_stream_valid_csv(input, ',', '"');
        assert_eq!(
            iter.next().unwrap().unwrap(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert_eq!(
            iter.next().unwrap().unwrap(),
            vec!["1".to_string(), "2".to_string(), "3".to_string()]
        );
        assert_eq!(
            iter.next().unwrap().unwrap(),
            vec!["4".to_string(), "5".to_string(), "6".to_string()]
        );
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_fast_stream_csv_with_unescaped_delimiters() {
        let input = Cursor::new("a,b,c\n1,2,3\n4,5,6,7,8,9");
        let mut iter = fast_stream_csv_with_unescaped_delimiters(input, ',', '"', 2, 3);
        assert_eq!(
            iter.next().unwrap().unwrap(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert_eq!(
            iter.next().unwrap().unwrap(),
            vec!["1".to_string(), "2".to_string(), "3".to_string()]
        );
        assert_eq!(
            iter.next().unwrap().unwrap(),
            vec!["4".to_string(), "5".to_string(), "6,7,8,9".to_string()]
        );
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_fast_stream_csv_with_unescaped_delimiters_error() {
        let input = Cursor::new("a,b,c\n1,2,3\n4,5\n10,11,12");
        let mut iter = fast_stream_csv_with_unescaped_delimiters(input, ',', '"', 2, 3);
        assert_eq!(
            iter.next().unwrap().unwrap(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert_eq!(
            iter.next().unwrap().unwrap(),
            vec!["1".to_string(), "2".to_string(), "3".to_string()]
        );
        assert_eq!(
            iter.next().unwrap().unwrap_err(),
            CsvError::Invalid(
                Position { line: 2, column: 3 },
                "Not enough columns. There may be an unescaped newline in a field."
            )
        );
        assert_eq!(
            iter.next().unwrap().unwrap(),
            vec!["10".to_string(), "11".to_string(), "12".to_string()]
        );
        assert!(iter.next().is_none());
    }
}

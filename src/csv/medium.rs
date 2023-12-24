//! A medium-complexity CSV parser, able to handle extra delimiters, quotes and newlines
//!
//! Limitations:
//! - Cannot handle missing delimiters or newlines
//! - It doesn't handle carriage returns (e.g. \r\n) as newlines correctly
//!   - This is a problem particularly when the last column is quoted
//! - Requires whole columns must be quoted or unquoted, not mixed
//!   (e.g. a,b"c,d"e,f) can be no more than 4 columns: [ "a", "b\"c", "d\"e", "f"]
//! - Requires either:
//!   - a fixed number of columns you specify before parsing
//!   - or a valid header row, with no unquoted delimiters or newlines
//! - Memory complexity: O(n) space on account of tracking all delimiters, quotes, and newlines
//! - Time complexity: O(2^n) where n is the number of cells.
//!   - But the search can be terminated, so the worst case is a timeout
//!   - The more likely outcome is O(nm) where m is the number of invalid special characters

use bitvec::prelude::*;

/// The properties of data contained in a column
///
/// In general, the solver will look for solutions that minimize the number of
/// complexities in the data, treating this as an indication that the data is
/// more likely to be valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum CharacterClass {
    Digit,
    Letter,
    Punctuation,
    Whitespace,
    Quote,
    Comma,
    Tab,
    Newline,
    Other,
}
impl CharacterClass {
    pub fn from_byte(byte: u8) -> CharacterClass {
        match byte {
            b'"' => CharacterClass::Quote,
            b',' => CharacterClass::Comma,
            b'\t' => CharacterClass::Tab,
            b'\n' => CharacterClass::Newline,
            b'\r' => CharacterClass::Newline,
            _ if byte.is_ascii_digit() => CharacterClass::Digit,
            _ if byte.is_ascii_alphabetic() => CharacterClass::Letter,
            _ if byte.is_ascii_punctuation() => CharacterClass::Punctuation,
            _ if byte.is_ascii_whitespace() => CharacterClass::Whitespace,
            _ if byte > 127 => CharacterClass::Other,
            _ => CharacterClass::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Eq, PartialOrd, Ord)]
pub struct ColumnComplexity {
    class_counts: [usize; 9],
}
impl ColumnComplexity {
    /// Create a new column complexity from an iterator of byte slices
    pub fn from_byte_slice_iter<'t, I: Iterator<Item = &'t [u8]>>(iter: I) -> Self {
        let mut this = Self::default();
        for bytes in iter {
            this.add_bytes(bytes);
        }
        this
    }

    /// Recalculate the column complexity after removing some bytes
    pub fn remove_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.class_counts[CharacterClass::from_byte(*byte) as usize] -= 1;
        }
    }

    /// Recalculate the column complexity after adding some bytes
    pub fn add_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.class_counts[CharacterClass::from_byte(*byte) as usize] += 1;
        }
    }

    /// Calculate the gini impurity of this column
    ///
    /// The gini impurity is a measure of how evenly distributed the classes are;
    /// a column with only one class has a gini impurity of 0, while a column with
    /// an even distribution of classes has a gini impurity of 1.
    /// We want to minimize the gini impurity of each column, since that indicates
    /// that the data is more likely to be valid.
    pub fn gini_impurity(&self) -> f64 {
        let total = self.class_counts.iter().sum::<usize>() as f64;
        let mut sum = 0.0;
        for count in self.class_counts.iter() {
            let p = *count as f64 / total;
            sum += p * p;
        }
        1.0 - sum
    }
}

type Mask = BitVec<u64, Lsb0>;

pub struct Switches {
    pub delimiter_valid: Mask,
    pub quote_valid: Mask,
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Solution {
    delimiter: u8,
    column_count: Option<usize>,
    column_complexities: Vec<ColumnComplexity>,
    file_length: usize,
    delimiter_locations: Vec<usize>,
    quote_locations: Vec<usize>,
    quote_can_start: Mask,
    quote_can_end: Mask,
}
impl Solution {
    /// Create a new default solution
    ///
    /// The validity of all delimiters, quotes, and newlines are subject to change;
    /// the purpose of this initial parse is to provide a starting point for the
    /// solver.
    pub fn new(raw: &[u8], delimiter: u8) -> Self {
        let mut this = Self::default();
        this.delimiter = delimiter;
        for (i, byte) in raw.iter().enumerate() {
            match byte {
                b'\n' => {
                    this.delimiter_locations.push(i);
                }
                b'"' => {
                    if i == 0
                        || raw[i - 1] == delimiter
                        || raw[i - 1] == b'\n'
                        || raw[i - 1..].starts_with(b"\r\n")
                        || i == raw.len() - 1
                    {
                        // Quotes are only allowed next to a delimiter, a carriage return, or the start/end of the file
                        this.quote_locations.push(i);
                    }
                }
                _ if *byte == delimiter => {
                    this.delimiter_locations.push(i);
                }
                _ => {}
            }
        }
        this.quote_can_start = this.quote_valid.clone();
        this.quote_can_end = this.quote_valid.clone();
        this.file_length = raw.len();
        this.default_heuristics();
        this
    }

    /// Apply the default heuristics to this solution
    /// in order to give the solver a better starting point
    fn default_heuristics(&mut self) {
        // Quotes can only be valid if they are preceded by or followed by a valid delimiter
        for (quote_num, &quote_byte) in self.quote_locations.iter().enumerate() {
            let prev = quote_byte == 0
                || self
                    .delimiter_locations
                    .binary_search(&(quote_byte - 1))
                    .is_ok();
            let next = quote_byte == self.file_length - 1
                || self
                    .delimiter_locations
                    .binary_search(&(quote_byte + 1))
                    .is_ok();
            if prev {
                self.quote_can_start.set(quote_num, false);
            }
            if next {
                self.quote_can_end.set(quote_num, false);
            }
        }
        // All end quotes before the first start quote are invalid
        for mut endable in &mut self.quote_can_end[..self.quote_can_start.first_one().unwrap_or(0)]
        {
            endable.set(false);
        }
        // Likewise, all start quotes after the last end quote are invalid
        for mut startable in &mut self.quote_can_start[self.quote_can_end.last_one().unwrap_or(0)..]
        {
            startable.set(false)
        }
        //
    }

    /// Iterate over quote pairs in the solution in order
    fn iter_quote_pairs<'t>(&'t self) -> impl Iterator<Item = (usize, usize)> + 't {
        let mut quotes = self.quote_locations.iter().enumerate().peekable();

        std::iter::from_fn(move || {
            let quotes = quotes.by_ref();
            let start_byte = quotes
                .skip_while(|(q_ix, _q_byte)| {
                    !self.quote_can_start[*q_ix] || !self.quote_valid[*q_ix]
                })
                .next()?
                .1;
            let end_byte = quotes
                .skip_while(|(q_ix, _q_byte)| {
                    !self.quote_can_end[*q_ix] || !self.quote_valid[*q_ix]
                })
                .next()?
                .1;
            Some((*start_byte, *end_byte))
        })
    }

    /// Iterate over all cells in the solution
    pub fn iter_cells<'t>(&'t self, raw: &'t [u8]) -> impl Iterator<Item = &'t [u8]> + 't {
        let mut prev_index = 0;
        self.iter_specials()
            .flat_map(move |(index, class)| match class {
                CharacterClass::Comma | CharacterClass::Newline => {
                    let cell = &raw[prev_index..index];
                    prev_index = index + 1;
                    cell
                }
                CharacterClass::Quote => {
                    let mut cell = &raw[prev_index..index];
                    prev_index = index + 1;
                    if cell.starts_with(&[b'"']) {
                        cell = &cell[1..];
                    }
                    if cell.ends_with(&[b'"']) {
                        cell = &cell[..cell.len() - 1];
                    }
                    cell
                }
            })
            .chain(std::iter::once(&raw[prev_index..]))
    }
}

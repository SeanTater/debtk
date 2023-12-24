mod csv;
mod errors;

#[derive(Debug, PartialEq)]
pub struct Position {
    line: usize,
    column: usize,
}

//! Parse possibly-malformed CSV files
///
/// There are many algorithms for robust parsing with different tradeoffs
/// between the errors they can handle and the performance they can achieve.
///
/// In general, you should prefer to use the simplest algorithm that meets your needs.
/// This is because the simplest algorithms are deterministic and easy to reason about.
/// The more complex parsers can handle ambiguous cases, but can actually parse valid
/// CSV files incorrectly.
mod easy;
mod medium;

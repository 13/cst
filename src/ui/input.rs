//! Interactive state of the sheet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State { pub query: String, pub focus: Option<usize>, pub scroll: usize, pub quit: bool }

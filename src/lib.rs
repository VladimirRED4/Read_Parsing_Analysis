mod binary_format;
mod csv_format;
mod error;
mod txt_format;

pub use binary_format::{BinaryParser, BinaryRecord};
pub use csv_format::CsvParser;
pub use error::ParserError;
pub use txt_format::TextParser;

use std::io::{Read, Write};

pub trait ParseFromRead<R: Read> {
    fn parse(reader: &mut R) -> Result<Self, ParserError>
    where
        Self: Sized;
}

pub trait WriteTo<W: Write> {
    fn write(&self, writer: &mut W) -> Result<(), ParserError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    pub tx_id: u64,
    pub tx_type: TransactionType,
    pub from_user_id: u64,
    pub to_user_id: u64,
    pub amount: i64,
    pub timestamp: u64,
    pub status: TransactionStatus,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionType {
    Deposit,
    Transfer,
    Withdrawal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionStatus {
    Success,
    Failure,
    Pending,
}

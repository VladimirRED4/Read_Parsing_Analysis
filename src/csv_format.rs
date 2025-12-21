use crate::{Transaction, TransactionType, TransactionStatus, ParserError};
use std::io::{Read, Write};
use csv::{ReaderBuilder, WriterBuilder};
use serde::{Deserialize, Serialize};

pub struct CsvParser;

impl CsvParser {
    pub fn parse_records<R: Read>(reader: R) -> Result<Vec<Transaction>, ParserError> {
        let mut csv_reader = ReaderBuilder::new()
            .has_headers(true)
            .trim(csv::Trim::All)
            .from_reader(reader);

        // Проверяем заголовок
        let headers = csv_reader.headers()
            .map_err(|e| ParserError::Parse(format!("Failed to read headers: {}", e)))?;

        let expected = ["TX_ID", "TX_TYPE", "FROM_USER_ID", "TO_USER_ID",
                       "AMOUNT", "TIMESTAMP", "STATUS", "DESCRIPTION"];

        if headers.len() != expected.len() {
            return Err(ParserError::Parse(
                format!("Expected {} columns, got {}", expected.len(), headers.len())
            ));
        }

        let mut records = Vec::new();

        for result in csv_reader.deserialize() {
            let csv_record: CsvRecord = result
                .map_err(|e| ParserError::Parse(e.to_string()))?;
            records.push(csv_record.into());
        }

        Ok(records)
    }

    pub fn write_records<W: Write>(records: &[Transaction], writer: W) -> Result<(), ParserError> {
        let mut csv_writer = WriterBuilder::new()
            .has_headers(true)
            .from_writer(writer);

        for record in records {
            let csv_record: CsvRecord = record.into();
            csv_writer.serialize(&csv_record)
                .map_err(|e| ParserError::Parse(e.to_string()))?;
        }

        csv_writer.flush()?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct CsvRecord {
    #[serde(rename = "TX_ID")]
    tx_id: u64,

    #[serde(rename = "TX_TYPE")]
    tx_type: String,

    #[serde(rename = "FROM_USER_ID")]
    from_user_id: u64,

    #[serde(rename = "TO_USER_ID")]
    to_user_id: u64,

    #[serde(rename = "AMOUNT")]
    amount: i64,

    #[serde(rename = "TIMESTAMP")]
    timestamp: u64,

    #[serde(rename = "STATUS")]
    status: String,

    #[serde(rename = "DESCRIPTION")]
    description: String,
}

impl From<CsvRecord> for Transaction {
    fn from(record: CsvRecord) -> Self {
        let tx_type = match record.tx_type.as_str() {
            "DEPOSIT" => TransactionType::Deposit,
            "TRANSFER" => TransactionType::Transfer,
            "WITHDRAWAL" => TransactionType::Withdrawal,
            _ => TransactionType::Transfer, // fallback
        };

        let status = match record.status.as_str() {
            "SUCCESS" => TransactionStatus::Success,
            "FAILURE" => TransactionStatus::Failure,
            "PENDING" => TransactionStatus::Pending,
            _ => TransactionStatus::Pending, // fallback
        };

        Transaction {
            tx_id: record.tx_id,
            tx_type,
            from_user_id: record.from_user_id,
            to_user_id: record.to_user_id,
            amount: record.amount,
            timestamp: record.timestamp,
            status,
            description: record.description,
        }
    }
}

impl From<&Transaction> for CsvRecord {
    fn from(transaction: &Transaction) -> Self {
        let tx_type = match transaction.tx_type {
            TransactionType::Deposit => "DEPOSIT",
            TransactionType::Transfer => "TRANSFER",
            TransactionType::Withdrawal => "WITHDRAWAL",
        };

        let status = match transaction.status {
            TransactionStatus::Success => "SUCCESS",
            TransactionStatus::Failure => "FAILURE",
            TransactionStatus::Pending => "PENDING",
        };

        CsvRecord {
            tx_id: transaction.tx_id,
            tx_type: tx_type.to_string(),
            from_user_id: transaction.from_user_id,
            to_user_id: transaction.to_user_id,
            amount: transaction.amount,
            timestamp: transaction.timestamp,
            status: status.to_string(),
            description: transaction.description.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const VALID_CSV: &str = r#"TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION
1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,"Initial account funding"
1002,TRANSFER,501,502,15000,1672534800000,FAILURE,"Payment for services"
1003,WITHDRAWAL,502,0,-1000,1672538400000,PENDING,"ATM withdrawal""#;

    #[test]
    fn test_parse_valid_csv() {
        let cursor = Cursor::new(VALID_CSV);
        let result = CsvParser::parse_records(cursor);

        assert!(result.is_ok());
        let transactions = result.unwrap();

        assert_eq!(transactions.len(), 3);

        // Проверяем первую запись
        assert_eq!(transactions[0].tx_id, 1001);
        assert!(matches!(transactions[0].tx_type, TransactionType::Deposit));
        assert_eq!(transactions[0].from_user_id, 0);
        assert_eq!(transactions[0].to_user_id, 501);
        assert_eq!(transactions[0].amount, 50000);
        assert_eq!(transactions[0].timestamp, 1672531200000);
        assert!(matches!(transactions[0].status, TransactionStatus::Success));
        assert_eq!(transactions[0].description, "Initial account funding");

        // Проверяем вторую запись
        assert_eq!(transactions[1].amount, 15000);
        assert!(matches!(transactions[1].status, TransactionStatus::Failure));

        // Проверяем третью запись
        assert_eq!(transactions[2].amount, -1000);
        assert!(matches!(transactions[2].tx_type, TransactionType::Withdrawal));
    }

    #[test]
    fn test_parse_csv_with_commas_in_description() {
        let csv = r#"TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION
1001,TRANSFER,501,502,15000,1672534800000,SUCCESS,"Payment for services, invoice #123""#;

        let cursor = Cursor::new(csv);
        let result = CsvParser::parse_records(cursor);

        assert!(result.is_ok());
        let transactions = result.unwrap();

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].description, "Payment for services, invoice #123");
    }

    #[test]
    fn test_parse_csv_wrong_headers() {
        let csv = r#"ID,TYPE,FROM,TO,AMOUNT,TIME,STATUS,DESC
1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,Test"#;

        let cursor = Cursor::new(csv);
        let result = CsvParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Parse(_))));
    }

    #[test]
    fn test_write_records() {
        let transactions = vec![
            Transaction {
                tx_id: 1001,
                tx_type: TransactionType::Deposit,
                from_user_id: 0,
                to_user_id: 501,
                amount: 50000,
                timestamp: 1672531200000,
                status: TransactionStatus::Success,
                description: "Initial deposit".to_string(),
            },
            Transaction {
                tx_id: 1002,
                tx_type: TransactionType::Transfer,
                from_user_id: 501,
                to_user_id: 502,
                amount: -15000,
                timestamp: 1672534800000,
                status: TransactionStatus::Failure,
                description: "Transfer with, comma".to_string(),
            },
        ];

        let mut buffer = Vec::new();
        let result = CsvParser::write_records(&transactions, &mut buffer);

        assert!(result.is_ok());

        let csv_output = String::from_utf8(buffer).unwrap();
        println!("Generated CSV:\n{}", csv_output);

        // Проверяем заголовок
        assert!(csv_output.starts_with("TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n"));

        // Проверяем наличие данных
        assert!(csv_output.contains("1001,DEPOSIT"));
        assert!(csv_output.contains("1002,TRANSFER"));

        // Парсим обратно и проверяем round-trip
        let cursor = Cursor::new(csv_output);
        let parsed = CsvParser::parse_records(cursor).unwrap();

        assert_eq!(transactions.len(), parsed.len());
        assert_eq!(transactions[0].tx_id, parsed[0].tx_id);
        assert_eq!(transactions[1].description, parsed[1].description);
    }

    #[test]
    fn test_roundtrip() {
        // Создаем тестовые транзакции
        let original_transactions = vec![
            Transaction {
                tx_id: 1001,
                tx_type: TransactionType::Deposit,
                from_user_id: 0,
                to_user_id: 501,
                amount: 50000,
                timestamp: 1672531200000,
                status: TransactionStatus::Success,
                description: "Test deposit".to_string(),
            },
            Transaction {
                tx_id: 1002,
                tx_type: TransactionType::Withdrawal,
                from_user_id: 502,
                to_user_id: 0,
                amount: -2000,
                timestamp: 1672538400000,
                status: TransactionStatus::Pending,
                description: "ATM withdrawal".to_string(),
            },
        ];

        // Записываем
        let mut buffer = Vec::new();
        CsvParser::write_records(&original_transactions, &mut buffer).unwrap();

        // Читаем обратно
        let cursor = Cursor::new(&buffer);
        let parsed_transactions = CsvParser::parse_records(cursor).unwrap();

        // Сравниваем
        assert_eq!(original_transactions, parsed_transactions);
    }
}
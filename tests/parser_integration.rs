use parser_lib::{CsvParser, TextParser, BinaryParser, Transaction, TransactionType, TransactionStatus};
use std::io::Cursor;

#[test]
fn test_csv_parsing() {
    let csv_data = r#"TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION
1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,"Initial account funding""#;

    let cursor = Cursor::new(csv_data);
    let result = CsvParser::parse_records(cursor);

    assert!(result.is_ok());
    let transactions = result.unwrap();
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].tx_id, 1001);
}

#[test]
fn test_text_parsing() {
    let text_data = r#"TX_ID: 1001
TX_TYPE: DEPOSIT
FROM_USER_ID: 0
TO_USER_ID: 501
AMOUNT: 50000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Initial deposit""#;

    let cursor = Cursor::new(text_data);
    let result = TextParser::parse_records(cursor);

    assert!(result.is_ok());
    let transactions = result.unwrap();
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].tx_id, 1001);
}

#[test]
fn test_binary_parsing() {
    let record = Transaction {
        tx_id: 1001,
        tx_type: TransactionType::Deposit,
        from_user_id: 0,
        to_user_id: 501,
        amount: 50000,
        timestamp: 1672531200000,
        status: TransactionStatus::Success,
        description: "Test".to_string(),
    };

    let mut buffer = Vec::new();
    assert!(BinaryParser::write_records(&[record.clone()], &mut buffer).is_ok());

    let mut cursor = Cursor::new(&buffer);
    let parsed = BinaryParser::parse_records(&mut cursor);
    assert!(parsed.is_ok());
    let parsed_records = parsed.unwrap();
    assert_eq!(parsed_records.len(), 1);
    assert_eq!(parsed_records[0].tx_id, record.tx_id);
}

#[test]
fn test_cross_format_roundtrip() {
    // Создаем тестовую транзакцию
    let original = Transaction {
        tx_id: 12345,
        tx_type: TransactionType::Transfer,
        from_user_id: 100,
        to_user_id: 200,
        amount: 5000,
        timestamp: 1672531200000,
        status: TransactionStatus::Success,
        description: "Test transaction".to_string(),
    };

    // Тест CSV roundtrip
    let mut csv_buffer = Vec::new();
    CsvParser::write_records(&[original.clone()], &mut csv_buffer).unwrap();
    let csv_cursor = Cursor::new(csv_buffer);
    let csv_result = CsvParser::parse_records(csv_cursor).unwrap();
    assert_eq!(csv_result.len(), 1);
    assert_eq!(csv_result[0].tx_id, original.tx_id);

    // Тест Text roundtrip
    let mut text_buffer = Vec::new();
    TextParser::write_records(&[original.clone()], &mut text_buffer).unwrap();
    let text_cursor = Cursor::new(text_buffer);
    let text_result = TextParser::parse_records(text_cursor).unwrap();
    assert_eq!(text_result.len(), 1);
    assert_eq!(text_result[0].tx_id, original.tx_id);

    // Тест Binary roundtrip
    let mut bin_buffer = Vec::new();
    BinaryParser::write_records(&[original.clone()], &mut bin_buffer).unwrap();
    let mut bin_cursor = Cursor::new(bin_buffer);
    let bin_result = BinaryParser::parse_records(&mut bin_cursor).unwrap();
    assert_eq!(bin_result.len(), 1);
    assert_eq!(bin_result[0].tx_id, original.tx_id);
}

#[test]
fn test_comparer_functionality() {
    // Этот тест проверяет, что парсеры работают согласованно,
    // что важно для comparer

    let transaction = Transaction {
        tx_id: 1001,
        tx_type: TransactionType::Deposit,
        from_user_id: 0,
        to_user_id: 501,
        amount: 50000,
        timestamp: 1672531200000,
        status: TransactionStatus::Success,
        description: "Test".to_string(),
    };

    // CSV roundtrip
    let mut csv_buffer = Vec::new();
    CsvParser::write_records(&[transaction.clone()], &mut csv_buffer).unwrap();
    let csv_cursor = std::io::Cursor::new(csv_buffer);
    let csv_result = CsvParser::parse_records(csv_cursor).unwrap();

    // Text roundtrip
    let mut text_buffer = Vec::new();
    TextParser::write_records(&[transaction.clone()], &mut text_buffer).unwrap();
    let text_cursor = std::io::Cursor::new(text_buffer);
    let text_result = TextParser::parse_records(text_cursor).unwrap();

    // Binary roundtrip
    let mut bin_buffer = Vec::new();
    BinaryParser::write_records(&[transaction.clone()], &mut bin_buffer).unwrap();
    let mut bin_cursor = std::io::Cursor::new(bin_buffer);
    let bin_result = BinaryParser::parse_records(&mut bin_cursor).unwrap();

    // Все должны быть равны оригиналу
    assert_eq!(csv_result[0], transaction);
    assert_eq!(text_result[0], transaction);
    assert_eq!(bin_result[0], transaction);

    // Все должны быть равны между собой
    assert_eq!(csv_result[0], text_result[0]);
    assert_eq!(csv_result[0], bin_result[0]);
    assert_eq!(text_result[0], bin_result[0]);
}

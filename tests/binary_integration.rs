use parser_lib::{BinaryParser, Transaction, TransactionStatus, TransactionType};
use std::io::Cursor;

#[test]
fn test_binary_parser_multiple_records() {
    let records = vec![
        Transaction {
            tx_id: 1001,
            tx_type: TransactionType::Deposit,
            from_user_id: 0,
            to_user_id: 501,
            amount: 50000,
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description: "First".to_string(),
        },
        Transaction {
            tx_id: 1002,
            tx_type: TransactionType::Transfer,
            from_user_id: 501,
            to_user_id: 502,
            amount: -15000,
            timestamp: 1672534800000,
            status: TransactionStatus::Failure,
            description: "Second".to_string(),
        },
    ];

    let mut buffer = Vec::new();
    BinaryParser::write_records(&records, &mut buffer).unwrap();

    let mut cursor = Cursor::new(&buffer);
    let parsed = BinaryParser::parse_records(&mut cursor).unwrap();

    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].tx_id, 1001);
    assert_eq!(parsed[1].tx_id, 1002);
}

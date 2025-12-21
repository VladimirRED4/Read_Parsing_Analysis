use std::io::Cursor;
use parser_lib::{BinaryRecord, BinaryParser, Transaction, TransactionType, TransactionStatus};

fn main() -> Result<(), parser_lib::ParserError> {
    println!("=== Тестирование бинарного формата ===\n");

    // 1. Тест одной записи
    println!("1. Тест round-trip одной записи:");
    let record = BinaryRecord {
        tx_id: 1001,
        tx_type: TransactionType::Deposit,
        from_user_id: 0,
        to_user_id: 501,
        amount: 50000,
        timestamp: 1672531200000,
        status: TransactionStatus::Success,
        description: "Initial deposit".to_string(),
    };

    let mut buffer = Vec::new();
    record.write_to(&mut buffer)?;
    println!("   Записано {} байт", buffer.len());

    let mut cursor = Cursor::new(&buffer);
    let parsed = BinaryRecord::from_read(&mut cursor)?;

    if record == parsed {
        println!("   ✓ Round-trip успешен");
        println!("   TX_ID: {}, Описание: '{}'", parsed.tx_id, parsed.description);
    } else {
        println!("   ✗ Round-trip не удался");
    }

    // 2. Тест нескольких записей
    println!("\n2. Тест нескольких записей:");

    let records = vec![
        BinaryRecord {
            tx_id: 1001,
            tx_type: TransactionType::Deposit,
            from_user_id: 0,
            to_user_id: 501,
            amount: 10000,
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description: "First deposit".to_string(),
        },
        BinaryRecord {
            tx_id: 1002,
            tx_type: TransactionType::Transfer,
            from_user_id: 501,
            to_user_id: 502,
            amount: -5000,
            timestamp: 1672534800000,
            status: TransactionStatus::Failure,
            description: "Failed transfer".to_string(),
        },
        BinaryRecord {
            tx_id: 1003,
            tx_type: TransactionType::Withdrawal,
            from_user_id: 502,
            to_user_id: 0,
            amount: -2000,
            timestamp: 1672538400000,
            status: TransactionStatus::Pending,
            description: "ATM withdrawal".to_string(),
        },
    ];

    let mut multi_buffer = Vec::new();
    for record in &records {
        record.write_to(&mut multi_buffer)?;
    }
    println!("   Записано {} записей, всего {} байт", records.len(), multi_buffer.len());

    // 3. Тест через BinaryParser
    println!("\n3. Чтение через BinaryParser:");
    let mut cursor = Cursor::new(&multi_buffer);
    let transactions = BinaryParser::parse_records(&mut cursor)?;

    println!("   Прочитано {} транзакций", transactions.len());
    for (i, tx) in transactions.iter().enumerate() {
        println!("   Транзакция {}: ID={}, Тип={:?}, Сумма={}, Статус={:?}",
                 i + 1, tx.tx_id, tx.tx_type, tx.amount, tx.status);
    }

    // 4. Тест конвертации
    println!("\n4. Тест конвертации в Transaction:");
    let transaction: Transaction = records[0].clone().into();
    println!("   Конвертировано: ID={}, Описание='{}'",
             transaction.tx_id, transaction.description);

    // 5. Тест с пустым описанием
    println!("\n5. Тест с пустым описанием:");
    let empty_desc_record = BinaryRecord {
        tx_id: 9999,
        tx_type: TransactionType::Deposit,
        from_user_id: 0,
        to_user_id: 100,
        amount: 1000,
        timestamp: 1672531200000,
        status: TransactionStatus::Success,
        description: String::new(),
    };

    let mut buffer3 = Vec::new();
    empty_desc_record.write_to(&mut buffer3)?;

    let mut cursor = Cursor::new(&buffer3);
    let parsed_empty = BinaryRecord::from_read(&mut cursor)?;

    if parsed_empty.description.is_empty() {
        println!("   ✓ Пустое описание корректно обработано");
        println!("   Размер записи: {} байт", buffer3.len());
    }

    println!("\n=== Все тесты завершены ===");
    Ok(())
}
use std::io::Cursor;
use parser_lib::{CsvParser, Transaction, TransactionType, TransactionStatus};

fn main() -> Result<(), parser_lib::ParserError> {
    println!("=== Тестирование CSV формата ===\n");

    // 1. Парсинг примера из спецификации
    println!("1. Парсинг примера из спецификации:");
    let csv_data = r#"TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION
1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,"Initial account funding"
1002,TRANSFER,501,502,15000,1672534800000,FAILURE,"Payment for services, invoice #123"
1003,WITHDRAWAL,502,0,1000,1672538400000,PENDING,"ATM withdrawal""#;

    let cursor = Cursor::new(csv_data);
    let transactions = CsvParser::parse_records(cursor)?;

    println!("   Прочитано {} транзакций:", transactions.len());
    for (i, tx) in transactions.iter().enumerate() {
        println!("   {}: ID={}, Тип={:?}, Сумма={}, Описание='{}'",
                 i + 1, tx.tx_id, tx.tx_type, tx.amount, tx.description);
    }

    // 2. Запись обратно в CSV
    println!("\n2. Запись обратно в CSV:");
    let mut buffer = Vec::new();
    CsvParser::write_records(&transactions, &mut buffer)?;

    let csv_output = String::from_utf8(buffer).unwrap();
    println!("   Сгенерировано {} байт", csv_output.len());
    println!("   Первые строки:\n{}",
             csv_output.lines().take(4).collect::<Vec<_>>().join("\n"));

    // 3. Round-trip тест
    println!("\n3. Round-trip тест:");
    let cursor = Cursor::new(csv_output);
    let parsed_again = CsvParser::parse_records(cursor)?;

    if transactions == parsed_again {
        println!("   ✓ Транзакции идентичны после round-trip");
    } else {
        println!("   ✗ Транзакции отличаются после round-trip");
    }

    // 4. Тест со специальными символами
    println!("\n4. Тест со специальными символами:");
    let special_transaction = Transaction {
        tx_id: 9999,
        tx_type: TransactionType::Transfer,
        from_user_id: 100,
        to_user_id: 200,
        amount: 5000,
        timestamp: 1672531200000,
        status: TransactionStatus::Success,
        description: r#"Payment with "quotes" and, comma inside"#.to_string(),
    };

    let mut buffer2 = Vec::new();
    CsvParser::write_records(&[special_transaction.clone()], &mut buffer2)?;

    let csv_special = String::from_utf8(buffer2).unwrap();
    println!("   Сгенерированный CSV:");
    println!("   {}", csv_special.trim());

    let cursor = Cursor::new(csv_special);
    let parsed_special = CsvParser::parse_records(cursor)?;

    if parsed_special[0].description == r#"Payment with "quotes" and, comma inside"# {
        println!("   ✓ Специальные символы корректно обработаны");
    } else {
        println!("   ✗ Проблема со специальными символами");
        println!("   Ожидалось: {}", r#"Payment with "quotes" and, comma inside"#);
        println!("   Получено:  {}", parsed_special[0].description);
    }

    // 5. Создание своих данных
    println!("\n5. Создание и парсинг собственных данных:");
    let my_transactions = vec![
        Transaction {
            tx_id: 3001,
            tx_type: TransactionType::Deposit,
            from_user_id: 0,
            to_user_id: 300,
            amount: 75000,
            timestamp: 1672642800000,
            status: TransactionStatus::Success,
            description: "Salary deposit".to_string(),
        },
        Transaction {
            tx_id: 3002,
            tx_type: TransactionType::Withdrawal,
            from_user_id: 300,
            to_user_id: 0,
            amount: 25000, // Положительная сумма для WITHDRAWAL
            timestamp: 1672646400000,
            status: TransactionStatus::Pending,
            description: "Rent payment".to_string(),
        },
    ];

    let mut buffer3 = Vec::new();
    CsvParser::write_records(&my_transactions, &mut buffer3)?;

    let my_csv = String::from_utf8(buffer3).unwrap();
    println!("   Созданный CSV:");
    for line in my_csv.lines() {
        println!("   {}", line);
    }

    // 6. Парсинг созданного CSV обратно
    println!("\n6. Парсинг созданного CSV обратно:");
    let cursor = Cursor::new(my_csv);
    let parsed_my = CsvParser::parse_records(cursor)?;

    println!("   Распарсено {} транзакций", parsed_my.len());
    for (i, tx) in parsed_my.iter().enumerate() {
        println!("   Транзакция {}: ID={}, Тип={:?}, Сумма={}",
                 i + 1, tx.tx_id, tx.tx_type, tx.amount);
    }

    println!("\n=== Все тесты завершены ===");
    Ok(())
}
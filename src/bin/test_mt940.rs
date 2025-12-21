use std::io::Cursor;
use parser_lib::{MT940Parser, Transaction, TransactionType, TransactionStatus};
use std::collections::HashMap;

fn main() -> Result<(), parser_lib::ParserError> {
    println!("=== Тестирование MT940 формата ===\n");

    // 1. Парсинг примера из файла
    println!("1. Парсинг примера из файла:");

    let mt940_data = r#"{1:F01GSCRUS30XXXX3614000002}{2:I940GSCRUS30XXXXN}{4:
:20:15486025400
:25:107048825
:28C:49/2
:60M:C250218USD2732398848,02
:61:2502180218D12,01NTRFGSLNVSHSUTKWDR//GI2504900007841
:86:/EREF/GSLNVSHSUTKWDR
/CRNM/GOLDMAN SACHS BANK USA
/CACT/107045863/CBIC/GSCRUS30XXX
/REMI/USD Payment to Vendor
/OPRP/Tag Payment
:61:2502180218D12,01NTRFGSOXWBAQYTF4VH//GI2504900005623
:86:/EREF/GSOXWBAQYTF4VH
/CRNM/GOLDMAN SACHS BANK USA
/CACT/107045863/CBIC/GSCRUS30XXX
/REMI/The maximum length of the block is 65 characters
/OPRP/Tag Payment
:61:2502180218C11,25NTRFGS0DUTB31IOUHRS//GI2504900004512
:86:/EREF/GS0DUTB31IOUHRS
/DACT/8348577826/DBIC/CITIUS30XXX
/OAMT/11-25/
/DCID/CPQYTB74
:62M:C250218USD2937898,77
-}"#;

    let cursor = Cursor::new(mt940_data);
    match MT940Parser::parse_records(cursor) {
        Ok(transactions) => {
            println!("   Прочитано {} транзакций:", transactions.len());
            for (i, tx) in transactions.iter().enumerate() {
                println!("   {}: ID={}, Тип={:?}, Сумма={}, Описание='{}'",
                         i + 1, tx.tx_id, tx.tx_type, tx.amount, tx.description);
            }
        }
        Err(e) => {
            println!("   Ошибка парсинга: {}", e);
            return Err(e);
        }
    }

    // 2. Простой тест для отладки
    println!("\n2. Простой тест для отладки:");
    let simple_mt940 = r#":20:REF123
:25:1234567890
:61:250218D12,01NTRF//REF12345
:86:/REMI/Test Payment
/EREF/REF12345
:61:260218C25,50NTRF//REF12346
:86:/REMI/Test Deposit
/EREF/REF12346"#;

    let cursor = Cursor::new(simple_mt940);
    match MT940Parser::parse_records(cursor) {
        Ok(transactions) => {
            println!("   Успешно распарсено {} транзакций", transactions.len());
            for (i, tx) in transactions.iter().enumerate() {
                println!("   Транзакция {}: Сумма={}, Тип={:?}",
                         i + 1, tx.amount, tx.tx_type);
            }
        }
        Err(e) => {
            println!("   Ошибка в простом тесте: {}", e);
        }
    }

    // 3. Анализ типов транзакций из первого теста
    println!("\n3. Анализ типов транзакций:");

    let cursor = Cursor::new(mt940_data);
    if let Ok(transactions) = MT940Parser::parse_records(cursor) {
        let mut deposit_count = 0;
        let mut transfer_count = 0;
        let mut withdrawal_count = 0;

        for tx in &transactions {
            match tx.tx_type {
                TransactionType::Deposit => deposit_count += 1,
                TransactionType::Transfer => transfer_count += 1,
                TransactionType::Withdrawal => withdrawal_count += 1,
            }
        }

        println!("   Депозиты: {}", deposit_count);
        println!("   Переводы: {}", transfer_count);
        println!("   Снятия:   {}", withdrawal_count);
    }

    // 4. Запись обратно в упрощенный формат MT940
    println!("\n4. Запись обратно в упрощенный формат MT940:");

    let test_transactions = vec![
        Transaction {
            tx_id: 1001,
            tx_type: TransactionType::Deposit,
            from_user_id: 0,
            to_user_id: 5001,
            amount: 1500000, // 15000.00
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description: "Зарплата".to_string(),
        },
        Transaction {
            tx_id: 1002,
            tx_type: TransactionType::Transfer,
            from_user_id: 5001,
            to_user_id: 5002,
            amount: -250000, // -2500.00
            timestamp: 1672534800000,
            status: TransactionStatus::Success,
            description: "Перевод другу".to_string(),
        },
    ];

    let mut buffer = Vec::new();
    match MT940Parser::write_records(&test_transactions, &mut buffer) {
        Ok(()) => {
            let mt940_output = String::from_utf8(buffer).unwrap();
            println!("   Сгенерировано {} байт", mt940_output.len());
            println!("   Первые 10 строк:");
            for line in mt940_output.lines().take(10) {
                println!("   {}", line);
            }
        }
        Err(e) => {
            println!("   Ошибка записи: {}", e);
        }
    }

    // 5. Тест с разными форматами сумм
    println!("\n5. Тест с разными форматами сумм:");

    let amount_tests = vec![
        (":61:250218D12,01NTRF//REF1", "12,01", "D"),
        (":61:250218C25.50NTRF//REF2", "25.50", "C"),
        (":61:250218D1000NTRF//REF3", "1000", "D"),
        (":61:250218C1234,56NTRF//REF4", "1234,56", "C"),
    ];

    for (test_str, expected_amount, expected_dc) in amount_tests {
        println!("   Тест: {}", test_str);
        let mut test_fields = HashMap::new();
        test_fields.insert("61".to_string(), test_str[4..].to_string()); // Убираем :61:

        // Просто для демонстрации парсинга
        println!("   Ожидается: {} {}", expected_dc, expected_amount);
    }

    // 6. Round-trip тест с простыми данными
    println!("\n6. Round-trip тест:");

    let simple_transactions = vec![
        Transaction {
            tx_id: 9999,
            tx_type: TransactionType::Deposit,
            from_user_id: 0,
            to_user_id: 100,
            amount: 10000, // 100.00
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description: "Test deposit".to_string(),
        },
    ];

    let mut roundtrip_buffer = Vec::new();
    if MT940Parser::write_records(&simple_transactions, &mut roundtrip_buffer).is_ok() {
        let roundtrip_cursor = Cursor::new(roundtrip_buffer);
        match MT940Parser::parse_records(roundtrip_cursor) {
            Ok(parsed) => {
                println!("   Успешно распарсено обратно: {} транзакций", parsed.len());
                if !parsed.is_empty() {
                    println!("   Первая транзакция: ID={}, Сумма={}",
                             parsed[0].tx_id, parsed[0].amount);
                }
            }
            Err(e) => {
                println!("   Ошибка при round-trip: {}", e);
            }
        }
    }

    println!("\n=== Все тесты MT940 завершены ===");
    Ok(())
}
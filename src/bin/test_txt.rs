use std::io::Cursor;
use parser_lib::{TextParser, Transaction, TransactionType, TransactionStatus};

fn main() -> Result<(), parser_lib::ParserError> {
    println!("=== Тестирование текстового формата ===\n");

    // 1. Парсинг примера из спецификации
    println!("1. Парсинг примера из спецификации:");
    let text_data = r#"# Record 1 (Deposit)
TX_ID: 1234567890123456
TX_TYPE: DEPOSIT
FROM_USER_ID: 0
TO_USER_ID: 9876543210987654
AMOUNT: 10000
TIMESTAMP: 1633036800000
STATUS: SUCCESS
DESCRIPTION: "Terminal deposit"

# Record 2 (Transfer)
TX_ID: 2312321321321321
TIMESTAMP: 1633056800000
STATUS: FAILURE
TX_TYPE: TRANSFER
FROM_USER_ID: 1231231231231231
TO_USER_ID: 9876543210987654
AMOUNT: 1000
DESCRIPTION: "User transfer"

# Record 3 (Withdrawal)
TX_ID: 3213213213213213
AMOUNT: 100
TX_TYPE: WITHDRAWAL
FROM_USER_ID: 9876543210987654
TO_USER_ID: 0
TIMESTAMP: 1633066800000
STATUS: SUCCESS
DESCRIPTION: "User withdrawal""#;

    let cursor = Cursor::new(text_data);
    let transactions = TextParser::parse_records(cursor)?;

    println!("   Прочитано {} транзакций:", transactions.len());
    for (i, tx) in transactions.iter().enumerate() {
        println!("   {}: ID={}, Тип={:?}, Сумма={}, Статус={:?}",
                 i + 1, tx.tx_id, tx.tx_type, tx.amount, tx.status);
    }

    // 2. Запись обратно в текстовый формат
    println!("\n2. Запись обратно в текстовый формат:");
    let mut buffer = Vec::new();
    TextParser::write_records(&transactions, &mut buffer)?;

    let text_output = String::from_utf8(buffer).unwrap();
    println!("   Сгенерировано {} байт", text_output.len());
    println!("   Первые 5 строк:");
    for line in text_output.lines().take(5) {
        println!("   {}", line);
    }

    // 3. Round-trip тест
    println!("\n3. Round-trip тест:");
    let cursor = Cursor::new(text_output);
    let parsed_again = TextParser::parse_records(cursor)?;

    if transactions == parsed_again {
        println!("   ✓ Транзакции идентичны после round-trip");
    } else {
        println!("   ✗ Транзакции отличаются после round-trip");
    }

    // 4. Тест с экранированными кавычками
    println!("\n4. Тест с экранированными кавычками:");
    let special_transaction = Transaction {
        tx_id: 9999,
        tx_type: TransactionType::Transfer,
        from_user_id: 100,
        to_user_id: 200,
        amount: 5000,
        timestamp: 1672531200000,
        status: TransactionStatus::Success,
        description: r#"Payment with "quotes" inside"#.to_string(),
    };

    let mut buffer2 = Vec::new();
    TextParser::write_records(&[special_transaction.clone()], &mut buffer2)?;

    let text_special = String::from_utf8(buffer2).unwrap();
    println!("   Сгенерированный текст:");
    for line in text_special.lines() {
        println!("   {}", line);
    }

    let cursor = Cursor::new(text_special);
    let parsed_special = TextParser::parse_records(cursor)?;

    if parsed_special[0].description == r#"Payment with "quotes" inside"# {
        println!("   ✓ Кавычки корректно экранированы и разэкранированы");
    } else {
        println!("   ✗ Проблема с кавычками");
        println!("   Ожидалось: {}", r#"Payment with "quotes" inside"#);
        println!("   Получено:  {}", parsed_special[0].description);
    }

    // 5. Тест с разным порядком полей
    println!("\n5. Тест с разным порядком полей:");
    let random_order_text = r#"DESCRIPTION: "Random order test"
AMOUNT: 75000
TX_TYPE: DEPOSIT
FROM_USER_ID: 0
STATUS: PENDING
TX_ID: 7777
TO_USER_ID: 8888
TIMESTAMP: 1672642800000"#;

    let cursor = Cursor::new(random_order_text);
    let random_transactions = TextParser::parse_records(cursor)?;

    println!("   Успешно распарсено: ID={}, Сумма={}",
             random_transactions[0].tx_id, random_transactions[0].amount);

    // 6. Тест с комментариями и пробелами
    println!("\n6. Тест с комментариями и пробелами:");
    let messy_text = r#"
# Много комментариев
# Пустых строк

   TX_ID:   1001
TX_TYPE: DEPOSIT
  FROM_USER_ID: 0
TO_USER_ID:   501
    AMOUNT: 50000
  TIMESTAMP:   1672531200000
STATUS: SUCCESS
DESCRIPTION:   "Messy but valid"

# Еще комментарий
# И еще один

TX_ID: 1002
TX_TYPE: WITHDRAWAL
FROM_USER_ID: 501
TO_USER_ID: 0
AMOUNT: -2000
TIMESTAMP: 1672538400000
STATUS: PENDING
DESCRIPTION: "Another one"
"#;

    let cursor = Cursor::new(messy_text);
    let messy_transactions = TextParser::parse_records(cursor)?;

    println!("   Распарсено {} транзакций из грязного текста",
             messy_transactions.len());

    println!("\n=== Все тесты завершены ===");
    Ok(())
}
use parser_lib::{TextParser, Transaction, TransactionStatus, TransactionType};
use std::io::Cursor;
use std::slice::from_ref;

fn main() -> Result<(), parser_lib::ParserError> {
    println!("=== Тестирование текстового формата ===\n");

    println!("1. Парсинг примера из спецификации:");
    let text_data = r#"# Record 1 (DEPOSIT)
TX_TYPE: DEPOSIT
TO_USER_ID: 9223372036854775807
FROM_USER_ID: 0
TIMESTAMP: 1633036860000
DESCRIPTION: "Record number 1"
TX_ID: 1000000000000000
AMOUNT: 100
STATUS: FAILURE

# Record 2 (TRANSFER)
DESCRIPTION: "Record number 2"
TIMESTAMP: 1633036920000
STATUS: PENDING
AMOUNT: 200
TX_ID: 1000000000000001
TX_TYPE: TRANSFER
FROM_USER_ID: 9223372036854775807
TO_USER_ID: 9223372036854775807

# Record 3 (WITHDRAWAL)
DESCRIPTION: "Record number 3"
FROM_USER_ID: 599094029349995112
TX_ID: 1000000000000002
TO_USER_ID: 0
AMOUNT: 300
TX_TYPE: WITHDRAWAL
STATUS: SUCCESS
TIMESTAMP: 1633036980000

# Record 4 (DEPOSIT)
TIMESTAMP: 1633037040000
TO_USER_ID: 6386297538413372968
AMOUNT: 400
TX_ID: 1000000000000003
STATUS: FAILURE
DESCRIPTION: "Record number 4"
TX_TYPE: DEPOSIT
FROM_USER_ID: 0

# Record 5 (TRANSFER)
AMOUNT: 500
FROM_USER_ID: 9223372036854775807
TO_USER_ID: 9223372036854775807
TX_ID: 1000000000000004
DESCRIPTION: "Record number 5"
TIMESTAMP: 1633037100000
STATUS: PENDING
TX_TYPE: TRANSFER

# Record 6 (WITHDRAWAL)
TO_USER_ID: 0
TX_ID: 1000000000000005
DESCRIPTION: "Record number 6"
AMOUNT: 600
TX_TYPE: WITHDRAWAL
TIMESTAMP: 1633037160000
FROM_USER_ID: 6238472699204189335
STATUS: SUCCESS

# Record 7 (DEPOSIT)
TO_USER_ID: 728970204360217851
TX_TYPE: DEPOSIT
AMOUNT: 700
DESCRIPTION: "Record number 7"
STATUS: FAILURE
FROM_USER_ID: 0
TIMESTAMP: 1633037220000
TX_ID: 1000000000000006

# Record 8 (TRANSFER)
TX_TYPE: TRANSFER
STATUS: PENDING
DESCRIPTION: "Record number 8"
TX_ID: 1000000000000007
TIMESTAMP: 1633037280000
FROM_USER_ID: 9223372036854775807
AMOUNT: 800
TO_USER_ID: 7524637015105340931

# Record 9 (WITHDRAWAL)
AMOUNT: 900
TX_ID: 1000000000000008
STATUS: SUCCESS
FROM_USER_ID: 5108918777190567747
TX_TYPE: WITHDRAWAL
TIMESTAMP: 1633037340000
DESCRIPTION: "Record number 9"
TO_USER_ID: 0

# Record 10 (DEPOSIT)
TX_TYPE: DEPOSIT
TX_ID: 1000000000000009
STATUS: FAILURE
AMOUNT: 1000
TIMESTAMP: 1633037400000
TO_USER_ID: 9223372036854775807
FROM_USER_ID: 0
DESCRIPTION: "Record number 10"

# Record 11 (TRANSFER)
AMOUNT: 1100
TX_ID: 1000000000000010
FROM_USER_ID: 2742528693116261933
TX_TYPE: TRANSFER
DESCRIPTION: "Record number 11"
TO_USER_ID: 6195600858058280266
STATUS: PENDING
TIMESTAMP: 1633037460000

# Record 12 (WITHDRAWAL)
FROM_USER_ID: 9223372036854775807
AMOUNT: 1200
TIMESTAMP: 1633037520000
TX_TYPE: WITHDRAWAL
TX_ID: 1000000000000011
TO_USER_ID: 0
STATUS: SUCCESS
DESCRIPTION: "Record number 12""#;

    let cursor = Cursor::new(text_data);
    let transactions = TextParser::parse_records(cursor)?;

    println!("   Прочитано {} транзакций:", transactions.len());
    for (i, tx) in transactions.iter().enumerate() {
        println!(
            "   {}: ID={}, Тип={:?}, Сумма={}, Статус={:?}",
            i + 1,
            tx.tx_id,
            tx.tx_type,
            tx.amount,
            tx.status
        );
    }

    println!("\n2. Запись обратно в текстовый формат:");
    let mut buffer = Vec::new();
    TextParser::write_records(&transactions, &mut buffer)?;

    let text_output = String::from_utf8(buffer).unwrap();
    println!("   Сгенерировано {} байт", text_output.len());
    println!("   Первые 5 строк:");
    for line in text_output.lines().take(5) {
        println!("   {}", line);
    }

    println!("\n3. Round-trip тест:");
    let cursor = Cursor::new(text_output);
    let parsed_again = TextParser::parse_records(cursor)?;

    if transactions == parsed_again {
        println!("   ✓ Транзакции идентичны после round-trip");
    } else {
        println!("   ✗ Транзакции отличаются после round-trip");
    }

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
    TextParser::write_records(from_ref(&special_transaction), &mut buffer2)?;

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
        println!("   Ожидалось: Payment with \"quotes\" inside");
        println!("   Получено:  {}", parsed_special[0].description);
    }

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

    println!(
        "   Успешно распарсено: ID={}, Сумма={}",
        random_transactions[0].tx_id, random_transactions[0].amount
    );

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
AMOUNT: 2000  # Положительная сумма для WITHDRAWAL
TIMESTAMP: 1672538400000
STATUS: PENDING
DESCRIPTION: "Another one"
"#;

    let cursor = Cursor::new(messy_text);
    let messy_transactions = TextParser::parse_records(cursor)?;

    println!(
        "   Распарсено {} транзакций из грязного текста",
        messy_transactions.len()
    );

    println!("\n=== Все тесты завершены ===");
    Ok(())
}

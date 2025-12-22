use crate::{Transaction, TransactionType, TransactionStatus, ParserError};
use std::io::{Read, Write};
use std::collections::HashMap;

/// Парсер для текстового формата YPBankText
pub struct TextParser;

impl TextParser {
    /// Читает все записи из текстового источника
    pub fn parse_records<R: Read>(reader: R) -> Result<Vec<Transaction>, ParserError> {
        let content = std::io::read_to_string(reader)
            .map_err(ParserError::Io)?;

        let mut records = Vec::new();
        let mut current_record: HashMap<String, String> = HashMap::new();
        let mut line_number = 0;

        for line in content.lines() {
            line_number += 1;

            // Пропускаем пустые строки
            let trimmed = line.trim();
            if trimmed.is_empty() {
                // Если встречаем пустую строку и есть текущая запись - сохраняем её
                if !current_record.is_empty() {
                    let record = Self::parse_record(&current_record, line_number)?;
                    records.push(record);
                    current_record.clear();
                }
                continue;
            }

            // Пропускаем комментарии
            if trimmed.starts_with('#') {
                continue;
            }

            // Парсим пару ключ-значение
            match Self::parse_key_value(trimmed, line_number) {
                Ok((key, value)) => {
                    if current_record.contains_key(&key) {
                        return Err(ParserError::Parse(
                            format!("Line {}: duplicate field '{}'", line_number, key)
                        ));
                    }
                    current_record.insert(key, value);
                }
                Err(e) => return Err(e),
            }
        }

        // Обрабатываем последнюю запись (если есть)
        if !current_record.is_empty() {
            let record = Self::parse_record(&current_record, line_number)?;
            records.push(record);
        }

        Ok(records)
    }

    /// Записывает все записи в текстовый приёмник
    pub fn write_records<W: Write>(records: &[Transaction], writer: &mut W) -> Result<(), ParserError> {
        for (i, record) in records.iter().enumerate() {
            if i > 0 {
                writeln!(writer).map_err(ParserError::Io)?; // Пустая строка между записями
            }

            writeln!(writer, "# Record {} ({:?})", i + 1, record.tx_type).map_err(ParserError::Io)?;

            // Записываем поля в фиксированном порядке для читаемости
            writeln!(writer, "TX_ID: {}", record.tx_id).map_err(ParserError::Io)?;
            writeln!(writer, "TX_TYPE: {}", Self::tx_type_to_str(record.tx_type)).map_err(ParserError::Io)?;
            writeln!(writer, "FROM_USER_ID: {}", record.from_user_id).map_err(ParserError::Io)?;
            writeln!(writer, "TO_USER_ID: {}", record.to_user_id).map_err(ParserError::Io)?;
            writeln!(writer, "AMOUNT: {}", record.amount).map_err(ParserError::Io)?;
            writeln!(writer, "TIMESTAMP: {}", record.timestamp).map_err(ParserError::Io)?;
            writeln!(writer, "STATUS: {}", Self::status_to_str(record.status)).map_err(ParserError::Io)?;
            writeln!(writer, "DESCRIPTION: \"{}\"", Self::escape_description(&record.description)).map_err(ParserError::Io)?;
        }

        Ok(())
    }

    /// Парсит строку вида "KEY: VALUE"
    fn parse_key_value(line: &str, line_number: usize) -> Result<(String, String), ParserError> {
        let parts: Vec<&str> = line.splitn(2, ':').collect();

        if parts.len() != 2 {
            return Err(ParserError::Parse(
                format!("Line {}: expected 'KEY: VALUE' format, got '{}'", line_number, line)
            ));
        }

        let key = parts[0].trim().to_string();
        let value = parts[1].trim().to_string();

        if key.is_empty() {
            return Err(ParserError::Parse(
                format!("Line {}: empty key", line_number)
            ));
        }

        Ok((key, value))
    }

    /// Парсит запись из HashMap полей
    fn parse_record(fields: &HashMap<String, String>, line_number: usize) -> Result<Transaction, ParserError> {
        // Проверяем наличие всех обязательных полей
        let required_fields = ["TX_ID", "TX_TYPE", "FROM_USER_ID", "TO_USER_ID",
                               "AMOUNT", "TIMESTAMP", "STATUS", "DESCRIPTION"];

        for &field in &required_fields {
            if !fields.contains_key(field) {
                return Err(ParserError::Parse(
                    format!("Missing required field: {}", field)
                ));
            }
        }

        // Парсим отдельные поля
        let tx_id = Self::parse_u64_field(fields, "TX_ID", line_number)?;
        let tx_type = Self::parse_tx_type(fields, line_number)?;
        let from_user_id = Self::parse_u64_field(fields, "FROM_USER_ID", line_number)?;
        let to_user_id = Self::parse_u64_field(fields, "TO_USER_ID", line_number)?;
        let amount = Self::parse_i64_field(fields, "AMOUNT", line_number)?;
        let timestamp = Self::parse_u64_field(fields, "TIMESTAMP", line_number)?;
        let status = Self::parse_status(fields, line_number)?;
        let description = Self::parse_description(fields, line_number)?;

        // Валидация бизнес-правил
        Self::validate_record(tx_type, from_user_id, to_user_id, amount, line_number)?;

        Ok(Transaction {
            tx_id,
            tx_type,
            from_user_id,
            to_user_id,
            amount, // Всегда положительная сумма в текстовом формате
            timestamp,
            status,
            description,
        })
    }

    /// Парсит поле типа u64
    fn parse_u64_field(fields: &HashMap<String, String>, field_name: &str, line_number: usize) -> Result<u64, ParserError> {
        let value = fields.get(field_name)
            .ok_or_else(|| ParserError::Parse(format!("Field {} not found", field_name)))?;

        value.parse::<u64>()
            .map_err(|e| ParserError::Parse(
                format!("Line {}: invalid {} '{}': {}", line_number, field_name, value, e)
            ))
    }

    /// Парсит поле типа i64 (всегда положительное в текстовом формате)
    fn parse_i64_field(fields: &HashMap<String, String>, field_name: &str, line_number: usize) -> Result<i64, ParserError> {
        let value = fields.get(field_name)
            .ok_or_else(|| ParserError::Parse(format!("Field {} not found", field_name)))?;

        // Убираем возможные комментарии и пробелы
        let clean_value = value.split('#').next().unwrap_or(value).trim();

        let amount = clean_value.parse::<i64>()
            .map_err(|e| ParserError::Parse(
                format!("Line {}: invalid {} '{}': {}", line_number, field_name, clean_value, e)
            ))?;

        // Проверяем что сумма положительная
        if amount <= 0 {
            return Err(ParserError::Parse(
                format!("Line {}: {} must be positive, got {}", line_number, field_name, amount)
            ));
        }

        Ok(amount)
    }

    /// Парсит тип транзакции
    fn parse_tx_type(fields: &HashMap<String, String>, line_number: usize) -> Result<TransactionType, ParserError> {
        let value = fields.get("TX_TYPE")
            .ok_or_else(|| ParserError::Parse("Field TX_TYPE not found".to_string()))?;

        match value.to_uppercase().as_str() {
            "DEPOSIT" => Ok(TransactionType::Deposit),
            "TRANSFER" => Ok(TransactionType::Transfer),
            "WITHDRAWAL" => Ok(TransactionType::Withdrawal),
            other => Err(ParserError::Parse(
                format!("Line {}: invalid TX_TYPE '{}', must be DEPOSIT, TRANSFER, or WITHDRAWAL",
                        line_number, other)
            )),
        }
    }

    /// Парсит статус транзакции
    fn parse_status(fields: &HashMap<String, String>, line_number: usize) -> Result<TransactionStatus, ParserError> {
        let value = fields.get("STATUS")
            .ok_or_else(|| ParserError::Parse("Field STATUS not found".to_string()))?;

        match value.to_uppercase().as_str() {
            "SUCCESS" => Ok(TransactionStatus::Success),
            "FAILURE" => Ok(TransactionStatus::Failure),
            "PENDING" => Ok(TransactionStatus::Pending),
            other => Err(ParserError::Parse(
                format!("Line {}: invalid STATUS '{}', must be SUCCESS, FAILURE, or PENDING",
                        line_number, other)
            )),
        }
    }

    /// Парсит описание (убирает окружающие кавычки)
    fn parse_description(fields: &HashMap<String, String>, line_number: usize) -> Result<String, ParserError> {
        let value = fields.get("DESCRIPTION")
            .ok_or_else(|| ParserError::Parse("Field DESCRIPTION not found".to_string()))?;

        let trimmed = value.trim();

        // Проверяем что описание в кавычках
        if !(trimmed.starts_with('"') && trimmed.ends_with('"')) {
            return Err(ParserError::Parse(
                format!("Line {}: DESCRIPTION must be in double quotes, got '{}'",
                        line_number, value)
            ));
        }

        // Убираем кавычки и разэкранируем
        let content = &trimmed[1..trimmed.len()-1];
        let unescaped = Self::unescape_description(content);

        Ok(unescaped)
    }

        /// Валидирует запись согласно бизнес-правилам
    fn validate_record(
        tx_type: TransactionType,
        from_user_id: u64,
        to_user_id: u64,
        _amount: i64, // Всегда положительное в текстовом формате (уже проверено в parse_i64_field)
        line_number: usize,
    ) -> Result<(), ParserError> {
        // Сумма уже проверена на положительность в parse_i64_field,
        // поэтому здесь просто игнорируем параметр

        match tx_type {
            TransactionType::Deposit => {
                if from_user_id != 0 {
                    return Err(ParserError::Parse(
                        format!("Line {}: DEPOSIT must have FROM_USER_ID = 0, got {}",
                                line_number, from_user_id)
                    ));
                }
            }
            TransactionType::Withdrawal => {
                if to_user_id != 0 {
                    return Err(ParserError::Parse(
                        format!("Line {}: WITHDRAWAL must have TO_USER_ID = 0, got {}",
                                line_number, to_user_id)
                    ));
                }
            }
            TransactionType::Transfer => {
                if from_user_id == 0 {
                    return Err(ParserError::Parse(
                        format!("Line {}: TRANSFER cannot have FROM_USER_ID = 0", line_number)
                    ));
                }
                if to_user_id == 0 {
                    return Err(ParserError::Parse(
                        format!("Line {}: TRANSFER cannot have TO_USER_ID = 0", line_number)
                    ));
                }
            }
        }

        Ok(())
    }

    /// Конвертирует TransactionType в строку
    fn tx_type_to_str(tx_type: TransactionType) -> &'static str {
        match tx_type {
            TransactionType::Deposit => "DEPOSIT",
            TransactionType::Transfer => "TRANSFER",
            TransactionType::Withdrawal => "WITHDRAWAL",
        }
    }

    /// Конвертирует TransactionStatus в строку
    fn status_to_str(status: TransactionStatus) -> &'static str {
        match status {
            TransactionStatus::Success => "SUCCESS",
            TransactionStatus::Failure => "FAILURE",
            TransactionStatus::Pending => "PENDING",
        }
    }

    /// Экранирует кавычки в описании
    fn escape_description(description: &str) -> String {
        description.replace('"', "\\\"")
    }

    /// Разэкранирует описание
    fn unescape_description(description: &str) -> String {
        description.replace("\\\"", "\"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_valid_text() {
        let text_data = r#"TX_ID: 1234567890123456
TX_TYPE: DEPOSIT
FROM_USER_ID: 0
TO_USER_ID: 9876543210987654
AMOUNT: 10000
TIMESTAMP: 1633036800000
STATUS: SUCCESS
DESCRIPTION: "Terminal deposit"

TX_ID: 2312321321321321
TX_TYPE: TRANSFER
FROM_USER_ID: 1231231231231231
TO_USER_ID: 9876543210987654
AMOUNT: 1000
TIMESTAMP: 1633056800000
STATUS: FAILURE
DESCRIPTION: "User transfer"

TX_ID: 3213213213213213
TX_TYPE: WITHDRAWAL
FROM_USER_ID: 9876543210987654
TO_USER_ID: 0
AMOUNT: 100
TIMESTAMP: 1633066800000
STATUS: SUCCESS
DESCRIPTION: "User withdrawal""#;

        let cursor = Cursor::new(text_data);
        let result = TextParser::parse_records(cursor);

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let transactions = result.unwrap();

        assert_eq!(transactions.len(), 3);

        // Проверяем первую запись
        assert_eq!(transactions[0].tx_id, 1234567890123456);
        assert!(matches!(transactions[0].tx_type, TransactionType::Deposit));
        assert_eq!(transactions[0].from_user_id, 0);
        assert_eq!(transactions[0].to_user_id, 9876543210987654);
        assert_eq!(transactions[0].amount, 10000); // Положительная
        assert_eq!(transactions[0].timestamp, 1633036800000);
        assert!(matches!(transactions[0].status, TransactionStatus::Success));
        assert_eq!(transactions[0].description, "Terminal deposit");

        // Проверяем вторую запись
        assert_eq!(transactions[1].tx_id, 2312321321321321);
        assert!(matches!(transactions[1].status, TransactionStatus::Failure));
        assert_eq!(transactions[1].description, "User transfer");

        // Проверяем третью запись
        assert_eq!(transactions[2].tx_id, 3213213213213213);
        assert!(matches!(transactions[2].tx_type, TransactionType::Withdrawal));
        assert_eq!(transactions[2].amount, 100); // Положительная для WITHDRAWAL
    }

    #[test]
    fn test_parse_with_comments_and_whitespace() {
        let text = r#"
# This is a comment
# Another comment

TX_ID: 1001
  TX_TYPE:   DEPOSIT
FROM_USER_ID:0
TO_USER_ID: 501
AMOUNT: 50000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Test deposit"

# Empty lines before next record


TX_ID: 1002
TX_TYPE: TRANSFER
FROM_USER_ID: 501
TO_USER_ID: 502
AMOUNT: 15000
TIMESTAMP: 1672534800000
STATUS: FAILURE
DESCRIPTION: "Test transfer"
"#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(result.is_ok());
        let transactions = result.unwrap();

        assert_eq!(transactions.len(), 2);
        assert_eq!(transactions[0].tx_id, 1001);
        assert_eq!(transactions[1].tx_id, 1002);
    }

    #[test]
    fn test_parse_missing_field() {
        let text = r#"TX_ID: 1001
TX_TYPE: DEPOSIT
FROM_USER_ID: 0
TO_USER_ID: 501
AMOUNT: 50000
TIMESTAMP: 1672531200000
# STATUS пропущено
DESCRIPTION: "Test""#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Parse(_))));
        if let Err(ParserError::Parse(msg)) = result {
            assert!(msg.contains("missing") || msg.contains("STATUS"));
        }
    }

    #[test]
    fn test_parse_duplicate_field() {
        let text = r#"TX_ID: 1001
TX_TYPE: DEPOSIT
TX_TYPE: DEPOSIT  # Дубликат
FROM_USER_ID: 0
TO_USER_ID: 501
AMOUNT: 50000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Test""#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Parse(_))));
        if let Err(ParserError::Parse(msg)) = result {
            assert!(msg.contains("duplicate"));
        }
    }

    #[test]
    fn test_parse_invalid_tx_type() {
        let text = r#"TX_ID: 1001
TX_TYPE: INVALID
FROM_USER_ID: 0
TO_USER_ID: 501
AMOUNT: 50000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Test""#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Parse(_))));
    }

    #[test]
    fn test_parse_description_without_quotes() {
        let text = r#"TX_ID: 1001
TX_TYPE: DEPOSIT
FROM_USER_ID: 0
TO_USER_ID: 501
AMOUNT: 50000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: Test without quotes"#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Parse(_))));
        if let Err(ParserError::Parse(msg)) = result {
            assert!(msg.contains("quotes"));
        }
    }

    #[test]
    fn test_parse_description_with_escaped_quotes() {
        let text = r#"TX_ID: 1001
TX_TYPE: DEPOSIT
FROM_USER_ID: 0
TO_USER_ID: 501
AMOUNT: 50000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Test with \"quotes\" inside""#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(result.is_ok());
        let transactions = result.unwrap();

        assert_eq!(transactions[0].description, r#"Test with "quotes" inside"#);
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
                amount: 15000,
                timestamp: 1672534800000,
                status: TransactionStatus::Failure,
                description: r#"Transfer with "quotes" and special chars"#.to_string(),
            },
        ];

        let mut buffer = Vec::new();
        let result = TextParser::write_records(&transactions, &mut buffer);

        assert!(result.is_ok());

        let text_output = String::from_utf8(buffer).unwrap();

        // Проверяем наличие комментариев
        assert!(text_output.contains("# Record"));

        // Проверяем наличие всех полей
        assert!(text_output.contains("TX_ID: 1001"));
        assert!(text_output.contains("TX_TYPE: DEPOSIT"));
        assert!(text_output.contains("TX_TYPE: TRANSFER"));

        // Проверяем что описание в кавычках
        assert!(text_output.contains("DESCRIPTION: \""));

        // Проверяем что кавычки экранированы
        assert!(text_output.contains(r#"\"quotes\""#));
    }

    #[test]
    fn test_roundtrip() {
        let original_transactions = vec![
            Transaction {
                tx_id: 1234567890,
                tx_type: TransactionType::Deposit,
                from_user_id: 0,
                to_user_id: 9876543210,
                amount: 100000,
                timestamp: 1672531200000,
                status: TransactionStatus::Success,
                description: "Test deposit with \"special\" chars".to_string(),
            },
            Transaction {
                tx_id: 9876543210,
                tx_type: TransactionType::Withdrawal,
                from_user_id: 1234567890,
                to_user_id: 0,
                amount: 50000, // Положительная для WITHDRAWAL
                timestamp: 1672534800000,
                status: TransactionStatus::Pending,
                description: "Test withdrawal".to_string(),
            },
        ];

        // Записываем
        let mut buffer = Vec::new();
        TextParser::write_records(&original_transactions, &mut buffer).unwrap();

        // Читаем обратно
        let cursor = Cursor::new(&buffer);
        let parsed_transactions = TextParser::parse_records(cursor).unwrap();

        // Сравниваем
        assert_eq!(original_transactions.len(), parsed_transactions.len());

        for i in 0..original_transactions.len() {
            assert_eq!(original_transactions[i].tx_id, parsed_transactions[i].tx_id);
            assert_eq!(original_transactions[i].tx_type, parsed_transactions[i].tx_type);
            assert_eq!(original_transactions[i].description, parsed_transactions[i].description);
        }
    }

    #[test]
    fn test_invalid_key_value_format() {
        let text = r#"TX_ID 1001  # Нет двоеточия
TX_TYPE: DEPOSIT
FROM_USER_ID: 0
TO_USER_ID: 501
AMOUNT: 50000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Test""#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Parse(_))));
    }

    #[test]
    fn test_business_validation_deposit() {
        // DEPOSIT с ненулевым from_user_id
        let text = r#"TX_ID: 1001
TX_TYPE: DEPOSIT
FROM_USER_ID: 123  # Должно быть 0
TO_USER_ID: 501
AMOUNT: 50000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Invalid deposit""#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Parse(_))));
    }

    #[test]
    fn test_business_validation_withdrawal() {
        // WITHDRAWAL с ненулевым to_user_id
        let text = r#"TX_ID: 1001
TX_TYPE: WITHDRAWAL
FROM_USER_ID: 501
TO_USER_ID: 123  # Должно быть 0
AMOUNT: 1000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Invalid withdrawal""#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Parse(_))));
    }

    #[test]
    fn test_negative_amount() {
        // Отрицательная сумма недопустима в текстовом формате
        let text = r#"TX_ID: 1001
TX_TYPE: DEPOSIT
FROM_USER_ID: 0
TO_USER_ID: 501
AMOUNT: -50000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Test""#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Parse(_))));
        if let Err(ParserError::Parse(msg)) = result {
            assert!(msg.contains("positive"));
        }
    }

    #[test]
    fn test_zero_amount() {
        // Нулевая сумма недопустима
        let text = r#"TX_ID: 1001
TX_TYPE: DEPOSIT
FROM_USER_ID: 0
TO_USER_ID: 501
AMOUNT: 0
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Test""#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Parse(_))));
        if let Err(ParserError::Parse(msg)) = result {
            assert!(msg.contains("positive"));
        }
    }
}
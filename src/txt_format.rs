use crate::{
    ParseFromRead, ParserError, TextTransactions, Transaction, TransactionStatus, TransactionType,
    WriteTo,
};
use std::collections::HashMap;
use std::io::{Read, Write};

/// Парсер текстового (key-value) формата транзакций
///
/// Текстовый формат имеет следующую структуру:
/// - Каждая запись состоит из пар "KEY: VALUE"
/// - Поддерживает комментарии (строки, начинающиеся с #)
/// - Поддерживает пустые строки как разделители записей
/// - Описания должны быть в двойных кавычках
pub struct TextParser;

impl TextParser {
    /// Парсит текстовые записи транзакций из читаемого потока
    ///
    /// # Аргументы
    /// * `reader` - Читаемый поток (например, файл или буфер)
    ///
    /// # Возвращает
    /// * `Ok(Vec<Transaction>)` - Вектор распарсенных транзакций
    /// * `Err(ParserError)` - Ошибка парсинга или ввода-вывода
    ///
    /// # Ошибки
    /// * `ParserError::Parse` - нарушен формат файла (пары `KEY: VALUE`,
    ///   обязательные поля, значения)
    /// * `ParserError::Validation` - нарушены бизнес-правила и требование
    ///   положительной суммы ([`Transaction::validate`],
    ///   [`Transaction::validate_positive_amount`])
    ///
    pub fn parse_records<R: Read>(reader: R) -> Result<Vec<Transaction>, ParserError> {
        Self::parse_records_impl(reader, true)
    }

    /// Парсит текстовые записи транзакций без проверки бизнес-правил
    ///
    /// Метод нужен, когда требуется прочитать заведомо некорректные данные
    /// (например, чтобы затем их исправить): проверки бизнес-правил
    /// ([`Transaction::validate`]) и положительной суммы
    /// ([`Transaction::validate_positive_amount`]) пропускаются. Проверки формата
    /// — пары `KEY: VALUE`, обязательные поля, типы значений — выполняются всегда.
    ///
    /// # Аргументы
    /// * `reader` - Читаемый поток (например, файл или буфер)
    ///
    /// # Возвращает
    /// * `Ok(Vec<Transaction>)` - Вектор распарсенных транзакций
    /// * `Err(ParserError)` - Ошибка парсинга или ввода-вывода
    ///
    pub fn parse_records_unvalidated<R: Read>(reader: R) -> Result<Vec<Transaction>, ParserError> {
        Self::parse_records_impl(reader, false)
    }

    fn parse_records_impl<R: Read>(
        reader: R,
        validate: bool,
    ) -> Result<Vec<Transaction>, ParserError> {
        let content = std::io::read_to_string(reader).map_err(ParserError::Io)?;

        let mut records = Vec::new();
        let mut current_record: HashMap<String, String> = HashMap::new();
        let mut line_number = 0;

        for line in content.lines() {
            line_number += 1;

            let trimmed = line.trim();
            if trimmed.is_empty() {
                if !current_record.is_empty() {
                    Self::append_record(&mut records, &current_record, line_number, validate)?;
                    current_record.clear();
                }
                continue;
            }

            if trimmed.starts_with('#') {
                continue;
            }

            match Self::parse_key_value(trimmed, line_number) {
                Ok((key, value)) => {
                    if current_record.contains_key(&key) {
                        return Err(ParserError::Parse(format!(
                            "Line {}: duplicate field '{}'",
                            line_number, key
                        )));
                    }
                    current_record.insert(key, value);
                }
                Err(e) => return Err(e),
            }
        }

        if !current_record.is_empty() {
            Self::append_record(&mut records, &current_record, line_number, validate)?;
        }

        Ok(records)
    }

    /// Разбирает накопленные поля записи, при необходимости проверяет
    /// бизнес-правила и добавляет транзакцию в результат.
    fn append_record(
        records: &mut Vec<Transaction>,
        fields: &HashMap<String, String>,
        line_number: usize,
        validate: bool,
    ) -> Result<(), ParserError> {
        let transaction = Self::parse_record(fields, line_number)?;

        if validate {
            let context = format!("Line {}", line_number);
            transaction
                .validate()
                .map_err(|e| e.with_context(&context))?;
            transaction
                .validate_positive_amount()
                .map_err(|e| e.with_context(&context))?;
        }

        records.push(transaction);

        Ok(())
    }

    /// Записывает транзакции в текстовый формат в записываемый поток
    ///
    /// # Аргументы
    /// * `records` - Список транзакций для записи
    /// * `writer` - Записываемый поток (например, файл или буфер)
    ///
    /// # Возвращает
    /// * `Ok(())` - Успешная запись
    /// * `Err(ParserError)` - Ошибка записи
    ///
    /// # Пример
    /// ```
    /// use parser_lib::{TextParser, Transaction, TransactionType, TransactionStatus};
    /// use std::fs::File;
    /// use std::io::BufWriter;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let transactions = vec![Transaction {
    ///     tx_id: 1001,
    ///     tx_type: TransactionType::Deposit,
    ///     from_user_id: 0,
    ///     to_user_id: 501,
    ///     amount: 50000,
    ///     timestamp: 1672531200000,
    ///     status: TransactionStatus::Success,
    ///     description: "Test".to_string(),
    /// }];
    ///
    /// let file = File::create("output.txt")?;
    /// let mut writer = BufWriter::new(file);
    /// TextParser::write_records(&transactions, &mut writer).unwrap();
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_records<W: Write>(
        records: &[Transaction],
        writer: &mut W,
    ) -> Result<(), ParserError> {
        for (i, record) in records.iter().enumerate() {
            if i > 0 {
                writeln!(writer).map_err(ParserError::Io)?;
            }

            writeln!(writer, "# Record {} ({:?})", i + 1, record.tx_type)
                .map_err(ParserError::Io)?;

            writeln!(writer, "TX_ID: {}", record.tx_id).map_err(ParserError::Io)?;
            writeln!(writer, "TX_TYPE: {}", Self::tx_type_to_str(record.tx_type))
                .map_err(ParserError::Io)?;
            writeln!(writer, "FROM_USER_ID: {}", record.from_user_id).map_err(ParserError::Io)?;
            writeln!(writer, "TO_USER_ID: {}", record.to_user_id).map_err(ParserError::Io)?;
            writeln!(writer, "AMOUNT: {}", record.amount).map_err(ParserError::Io)?;
            writeln!(writer, "TIMESTAMP: {}", record.timestamp).map_err(ParserError::Io)?;
            writeln!(writer, "STATUS: {}", Self::status_to_str(record.status))
                .map_err(ParserError::Io)?;
            writeln!(
                writer,
                "DESCRIPTION: \"{}\"",
                Self::escape_description(&record.description)
            )
            .map_err(ParserError::Io)?;
        }

        Ok(())
    }

    fn parse_key_value(line: &str, line_number: usize) -> Result<(String, String), ParserError> {
        let parts: Vec<&str> = line.splitn(2, ':').collect();

        if parts.len() != 2 {
            return Err(ParserError::Parse(format!(
                "Line {}: expected 'KEY: VALUE' format, got '{}'",
                line_number, line
            )));
        }

        let key = parts[0].trim().to_string();
        let value = parts[1].trim().to_string();

        if key.is_empty() {
            return Err(ParserError::Parse(format!(
                "Line {}: empty key",
                line_number
            )));
        }

        Ok((key, value))
    }

    fn parse_record(
        fields: &HashMap<String, String>,
        line_number: usize,
    ) -> Result<Transaction, ParserError> {
        let required_fields = [
            "TX_ID",
            "TX_TYPE",
            "FROM_USER_ID",
            "TO_USER_ID",
            "AMOUNT",
            "TIMESTAMP",
            "STATUS",
            "DESCRIPTION",
        ];

        for &field in &required_fields {
            if !fields.contains_key(field) {
                return Err(ParserError::Parse(format!(
                    "Missing required field: {}",
                    field
                )));
            }
        }

        let tx_id = Self::parse_u64_field(fields, "TX_ID", line_number)?;
        let tx_type = Self::parse_tx_type(fields, line_number)?;
        let from_user_id = Self::parse_u64_field(fields, "FROM_USER_ID", line_number)?;
        let to_user_id = Self::parse_u64_field(fields, "TO_USER_ID", line_number)?;
        let amount = Self::parse_i64_field(fields, "AMOUNT", line_number)?;
        let timestamp = Self::parse_u64_field(fields, "TIMESTAMP", line_number)?;
        let status = Self::parse_status(fields, line_number)?;
        let description = Self::parse_description(fields, line_number)?;

        Ok(Transaction {
            tx_id,
            tx_type,
            from_user_id,
            to_user_id,
            amount,
            timestamp,
            status,
            description,
        })
    }

    fn parse_u64_field(
        fields: &HashMap<String, String>,
        field_name: &str,
        line_number: usize,
    ) -> Result<u64, ParserError> {
        let value = fields
            .get(field_name)
            .ok_or_else(|| ParserError::Parse(format!("Field {} not found", field_name)))?;

        value.parse::<u64>().map_err(|e| {
            ParserError::Parse(format!(
                "Line {}: invalid {} '{}': {}",
                line_number, field_name, value, e
            ))
        })
    }

    fn parse_i64_field(
        fields: &HashMap<String, String>,
        field_name: &str,
        line_number: usize,
    ) -> Result<i64, ParserError> {
        let value = fields
            .get(field_name)
            .ok_or_else(|| ParserError::Parse(format!("Field {} not found", field_name)))?;

        let clean_value = value.split('#').next().unwrap_or(value).trim();

        clean_value.parse::<i64>().map_err(|e| {
            ParserError::Parse(format!(
                "Line {}: invalid {} '{}': {}",
                line_number, field_name, clean_value, e
            ))
        })
    }

    fn parse_tx_type(
        fields: &HashMap<String, String>,
        line_number: usize,
    ) -> Result<TransactionType, ParserError> {
        let value = fields
            .get("TX_TYPE")
            .ok_or_else(|| ParserError::Parse("Field TX_TYPE not found".to_string()))?;

        match value.to_uppercase().as_str() {
            "DEPOSIT" => Ok(TransactionType::Deposit),
            "TRANSFER" => Ok(TransactionType::Transfer),
            "WITHDRAWAL" => Ok(TransactionType::Withdrawal),
            other => Err(ParserError::Parse(format!(
                "Line {}: invalid TX_TYPE '{}', must be DEPOSIT, TRANSFER, or WITHDRAWAL",
                line_number, other
            ))),
        }
    }

    fn parse_status(
        fields: &HashMap<String, String>,
        line_number: usize,
    ) -> Result<TransactionStatus, ParserError> {
        let value = fields
            .get("STATUS")
            .ok_or_else(|| ParserError::Parse("Field STATUS not found".to_string()))?;

        match value.to_uppercase().as_str() {
            "SUCCESS" => Ok(TransactionStatus::Success),
            "FAILURE" => Ok(TransactionStatus::Failure),
            "PENDING" => Ok(TransactionStatus::Pending),
            other => Err(ParserError::Parse(format!(
                "Line {}: invalid STATUS '{}', must be SUCCESS, FAILURE, or PENDING",
                line_number, other
            ))),
        }
    }

    fn parse_description(
        fields: &HashMap<String, String>,
        line_number: usize,
    ) -> Result<String, ParserError> {
        let value = fields
            .get("DESCRIPTION")
            .ok_or_else(|| ParserError::Parse("Field DESCRIPTION not found".to_string()))?;

        let trimmed = value.trim();

        if !(trimmed.starts_with('"') && trimmed.ends_with('"')) {
            return Err(ParserError::Parse(format!(
                "Line {}: DESCRIPTION must be in double quotes, got '{}'",
                line_number, value
            )));
        }

        if trimmed.len() < 2 {
            return Err(ParserError::Parse(format!(
                "Line {}: DESCRIPTION too short, must be at least 2 characters for quotes",
                line_number
            )));
        }

        let content = &trimmed[1..trimmed.len() - 1];
        let unescaped = Self::unescape_description(content);

        Ok(unescaped)
    }

    fn tx_type_to_str(tx_type: TransactionType) -> &'static str {
        match tx_type {
            TransactionType::Deposit => "DEPOSIT",
            TransactionType::Transfer => "TRANSFER",
            TransactionType::Withdrawal => "WITHDRAWAL",
        }
    }

    fn status_to_str(status: TransactionStatus) -> &'static str {
        match status {
            TransactionStatus::Success => "SUCCESS",
            TransactionStatus::Failure => "FAILURE",
            TransactionStatus::Pending => "PENDING",
        }
    }

    fn escape_description(description: &str) -> String {
        description.replace('"', "\\\"")
    }

    fn unescape_description(description: &str) -> String {
        description.replace("\\\"", "\"")
    }
}

// Реализуем трейт ParseFromRead для TextTransactions
impl<R: Read> ParseFromRead<R> for TextTransactions {
    fn parse(reader: &mut R) -> Result<Self, ParserError> {
        let transactions = TextParser::parse_records(reader)?;
        Ok(TextTransactions(transactions))
    }
}

// Реализуем трейт WriteTo для TextTransactions
impl<W: Write> WriteTo<W> for TextTransactions {
    fn write(&self, writer: &mut W) -> Result<(), ParserError> {
        TextParser::write_records(&self.0, writer)
    }
}

// Реализуем трейт WriteTo для среза TextTransactions
impl<W: Write> WriteTo<W> for [TextTransactions] {
    fn write(&self, writer: &mut W) -> Result<(), ParserError> {
        for transactions in self {
            transactions.write(writer)?;
        }
        Ok(())
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

        assert_eq!(transactions[0].tx_id, 1234567890123456);
        assert!(matches!(transactions[0].tx_type, TransactionType::Deposit));
        assert_eq!(transactions[0].from_user_id, 0);
        assert_eq!(transactions[0].to_user_id, 9876543210987654);
        assert_eq!(transactions[0].amount, 10000);
        assert_eq!(transactions[0].timestamp, 1633036800000);
        assert!(matches!(transactions[0].status, TransactionStatus::Success));
        assert_eq!(transactions[0].description, "Terminal deposit");

        assert_eq!(transactions[1].tx_id, 2312321321321321);
        assert!(matches!(transactions[1].status, TransactionStatus::Failure));
        assert_eq!(transactions[1].description, "User transfer");

        assert_eq!(transactions[2].tx_id, 3213213213213213);
        assert!(matches!(
            transactions[2].tx_type,
            TransactionType::Withdrawal
        ));
        assert_eq!(transactions[2].amount, 100);
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

        assert!(text_output.contains("# Record"));

        assert!(text_output.contains("TX_ID: 1001"));
        assert!(text_output.contains("TX_TYPE: DEPOSIT"));
        assert!(text_output.contains("TX_TYPE: TRANSFER"));

        assert!(text_output.contains("DESCRIPTION: \""));

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
                amount: 50000,
                timestamp: 1672534800000,
                status: TransactionStatus::Pending,
                description: "Test withdrawal".to_string(),
            },
        ];

        let mut buffer = Vec::new();
        TextParser::write_records(&original_transactions, &mut buffer).unwrap();

        let cursor = Cursor::new(&buffer);
        let parsed_transactions = TextParser::parse_records(cursor).unwrap();

        assert_eq!(original_transactions.len(), parsed_transactions.len());

        for i in 0..original_transactions.len() {
            assert_eq!(original_transactions[i].tx_id, parsed_transactions[i].tx_id);
            assert_eq!(
                original_transactions[i].tx_type,
                parsed_transactions[i].tx_type
            );
            assert_eq!(
                original_transactions[i].description,
                parsed_transactions[i].description
            );
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
        // Для DEPOSIT FROM_USER_ID должен быть равен 0
        let text = r#"TX_ID: 1001
TX_TYPE: DEPOSIT
FROM_USER_ID: 123
TO_USER_ID: 501
AMOUNT: 50000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Invalid deposit""#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Validation(_))));
        if let Err(ParserError::Validation(msg)) = result {
            assert!(msg.contains("FROM_USER_ID"), "unexpected message: {}", msg);
            assert!(msg.contains("Line "), "нет контекста записи: {}", msg);
        }
    }

    #[test]
    fn test_business_validation_withdrawal() {
        // Для WITHDRAWAL TO_USER_ID должен быть равен 0
        let text = r#"TX_ID: 1001
TX_TYPE: WITHDRAWAL
FROM_USER_ID: 501
TO_USER_ID: 123
AMOUNT: 1000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Invalid withdrawal""#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Validation(_))));
        if let Err(ParserError::Validation(msg)) = result {
            assert!(msg.contains("TO_USER_ID"), "unexpected message: {}", msg);
            assert!(msg.contains("Line "), "нет контекста записи: {}", msg);
        }
    }

    #[test]
    fn test_negative_amount() {
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

        assert!(matches!(result, Err(ParserError::Validation(_))));
        if let Err(ParserError::Validation(msg)) = result {
            assert!(msg.contains("positive"));
        }
    }

    #[test]
    fn test_zero_amount() {
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

        assert!(matches!(result, Err(ParserError::Validation(_))));
        if let Err(ParserError::Validation(msg)) = result {
            assert!(msg.contains("positive"));
        }
    }

    #[test]
    fn test_validate_transfer_to_self() {
        let text = r#"TX_ID: 1001
TX_TYPE: TRANSFER
FROM_USER_ID: 501
TO_USER_ID: 501
AMOUNT: 5000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Transfer to self""#;

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Validation(_))));
        if let Err(ParserError::Validation(msg)) = result {
            assert!(
                msg.contains("FROM_USER_ID != TO_USER_ID"),
                "unexpected message: {}",
                msg
            );
        }
    }

    #[test]
    fn test_parse_records_unvalidated_allows_invalid_rules() {
        let text = r#"TX_ID: 1001
TX_TYPE: DEPOSIT
FROM_USER_ID: 999
TO_USER_ID: 501
AMOUNT: -50000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Invalid deposit""#;

        let transactions = TextParser::parse_records_unvalidated(Cursor::new(text)).unwrap();

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].from_user_id, 999);
        assert_eq!(transactions[0].amount, -50000);

        // с включённой проверкой тот же ввод отвергается
        assert!(TextParser::parse_records(Cursor::new(text)).is_err());
    }

    #[test]
    fn test_parse_description_empty_quotes() {
        let text = "TX_ID: 1001\nTX_TYPE: DEPOSIT\nFROM_USER_ID: 0\nTO_USER_ID: 501\nAMOUNT: 50000\nTIMESTAMP: 1672531200000\nSTATUS: SUCCESS\nDESCRIPTION: \"\"";

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let transactions = result.unwrap();
        assert_eq!(transactions[0].description, "");
    }

    #[test]
    fn test_parse_description_unicode() {
        let text = "TX_ID: 1001\nTX_TYPE: DEPOSIT\nFROM_USER_ID: 0\nTO_USER_ID: 501\nAMOUNT: 50000\nTIMESTAMP: 1672531200000\nSTATUS: SUCCESS\nDESCRIPTION: \"Тест с Unicode 🚀 и эмодзи 😊\"";

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let transactions = result.unwrap();
        assert_eq!(transactions[0].description, "Тест с Unicode 🚀 и эмодзи 😊");
    }

    #[test]
    fn test_parse_empty_lines_at_end() {
        let text = "TX_ID: 1001\nTX_TYPE: DEPOSIT\nFROM_USER_ID: 0\nTO_USER_ID: 501\nAMOUNT: 50000\nTIMESTAMP: 1672531200000\nSTATUS: SUCCESS\nDESCRIPTION: \"Test\"\n\n\n";

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(result.is_ok());
        let transactions = result.unwrap();
        assert_eq!(transactions.len(), 1);
    }

    #[test]
    fn test_parse_description_with_escaped_backslash() {
        let text = "TX_ID: 1001\nTX_TYPE: DEPOSIT\nFROM_USER_ID: 0\nTO_USER_ID: 501\nAMOUNT: 50000\nTIMESTAMP: 1672531200000\nSTATUS: SUCCESS\nDESCRIPTION: \"Test with \\\\ backslash\"";

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let transactions = result.unwrap();
        assert_eq!(transactions[0].description, "Test with \\\\ backslash");
    }

    #[test]
    fn test_parse_description_with_trailing_spaces() {
        let text = "TX_ID: 1001\nTX_TYPE: DEPOSIT\nFROM_USER_ID: 0\nTO_USER_ID: 501\nAMOUNT: 50000\nTIMESTAMP: 1672531200000\nSTATUS: SUCCESS\nDESCRIPTION:   \"Test with spaces\"   ";

        let cursor = Cursor::new(text);
        let result = TextParser::parse_records(cursor);

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let transactions = result.unwrap();
        assert_eq!(transactions[0].description, "Test with spaces");
    }

    #[test]
    fn test_parsefromread_trait_implementation() {
        let text = "TX_ID: 1001\nTX_TYPE: DEPOSIT\nFROM_USER_ID: 0\nTO_USER_ID: 501\nAMOUNT: 50000\nTIMESTAMP: 1672531200000\nSTATUS: SUCCESS\nDESCRIPTION: \"Test trait implementation\"";
        let mut cursor = Cursor::new(text);

        // Используем трейт ParseFromRead для TextTransactions
        let text_transactions: TextTransactions = TextTransactions::parse(&mut cursor).unwrap();
        let transactions = text_transactions.0; // Достаем Vec<Transaction> из обертки

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].tx_id, 1001);
        assert_eq!(transactions[0].description, "Test trait implementation");
    }

    #[test]
    fn test_writeto_trait_implementation() {
        let transactions = vec![Transaction {
            tx_id: 1001,
            tx_type: TransactionType::Deposit,
            from_user_id: 0,
            to_user_id: 501,
            amount: 50000,
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description: "Test trait write".to_string(),
        }];

        let text_transactions = TextTransactions(transactions);
        let mut buffer = Vec::new();

        // Используем трейт WriteTo для TextTransactions
        text_transactions.write(&mut buffer).unwrap();

        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("TX_ID: 1001"));
        assert!(output.contains("DESCRIPTION: \"Test trait write\""));
    }
}

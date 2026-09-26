use crate::{
    CsvTransactions, ParseFromRead, ParserError, Transaction, TransactionStatus, TransactionType,
    WriteTo,
};
use std::io::{Read, Write};

/// Парсер CSV формата транзакций
///
/// CSV формат имеет следующую структуру:
/// - Заголовок с именами полей (первая строка)
/// - Данные транзакций (последующие строки)
/// - Поддерживает экранирование кавычек и запятых в описаниях
pub struct CsvParser;

impl CsvParser {
    /// Парсит CSV записи транзакций из читаемого потока
    ///
    /// # Аргументы
    /// * `reader` - Читаемый поток (например, файл или буфер)
    ///
    /// # Возвращает
    /// * `Ok(Vec<Transaction>)` - Вектор распарсенных транзакций
    /// * `Err(ParserError)` - Ошибка парсинга или ввода-вывода
    ///
    /// # Ошибки
    /// * `ParserError::Parse` - нарушен формат файла (заголовки, поля, значения)
    /// * `ParserError::Validation` - нарушены бизнес-правила и требование
    ///   положительной суммы ([`Transaction::validate`],
    ///   [`Transaction::validate_positive_amount`])
    ///
    pub fn parse_records<R: Read>(reader: R) -> Result<Vec<Transaction>, ParserError> {
        Self::parse_records_impl(reader, true)
    }

    /// Парсит CSV записи транзакций без проверки бизнес-правил
    ///
    /// Метод нужен, когда требуется прочитать заведомо некорректные данные
    /// (например, чтобы затем их исправить): проверки бизнес-правил
    /// ([`Transaction::validate`]) и положительной суммы
    /// ([`Transaction::validate_positive_amount`]) пропускаются. Проверки формата
    /// — заголовки, количество полей, типы значений — выполняются всегда.
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

        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            return Ok(Vec::new());
        }

        let headers = Self::parse_line(lines[0], 0)?;
        Self::validate_headers(&headers)?;

        let mut records = Vec::new();

        for (line_num, line) in lines.iter().enumerate().skip(1) {
            let line_num = line_num + 1;
            if line.trim().is_empty() {
                continue;
            }

            let fields = Self::parse_line(line, line_num)?;
            let transaction = Self::parse_record(&fields, line_num)?;

            if validate {
                let context = format!("Line {}", line_num);
                transaction
                    .validate()
                    .map_err(|e| e.with_context(&context))?;
                transaction
                    .validate_positive_amount()
                    .map_err(|e| e.with_context(&context))?;
            }

            records.push(transaction);
        }

        Ok(records)
    }

    /// Записывает транзакции в CSV формат в записываемый поток
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
    /// use parser_lib::{CsvParser, Transaction, TransactionType, TransactionStatus};
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
    /// let mut buffer = Vec::new();
    /// CsvParser::write_records(&transactions, &mut buffer)?;
    ///
    /// let output = String::from_utf8(buffer)?;
    /// assert!(output.contains("1001,DEPOSIT"));
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_records<W: Write>(
        records: &[Transaction],
        writer: &mut W,
    ) -> Result<(), ParserError> {
        writeln!(
            writer,
            "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION"
        )
        .map_err(ParserError::Io)?;

        for record in records {
            let tx_type = match record.tx_type {
                TransactionType::Deposit => "DEPOSIT",
                TransactionType::Transfer => "TRANSFER",
                TransactionType::Withdrawal => "WITHDRAWAL",
            };

            let status = match record.status {
                TransactionStatus::Success => "SUCCESS",
                TransactionStatus::Failure => "FAILURE",
                TransactionStatus::Pending => "PENDING",
            };

            let description = Self::escape_description(&record.description);

            writeln!(
                writer,
                "{},{},{},{},{},{},{},{}",
                record.tx_id,
                tx_type,
                record.from_user_id,
                record.to_user_id,
                record.amount,
                record.timestamp,
                status,
                description
            )
            .map_err(ParserError::Io)?;
        }

        Ok(())
    }

    fn parse_line(line: &str, line_num: usize) -> Result<Vec<String>, ParserError> {
        let mut fields = Vec::new();
        let mut current_field = String::new();
        let mut current_field_quoted = false;
        let mut in_quotes = false;
        let mut chars = line.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '"' => {
                    current_field_quoted = true;
                    if in_quotes {
                        if let Some(&next_ch) = chars.peek() {
                            if next_ch == '"' {
                                chars.next();
                                current_field.push('"');
                            } else {
                                in_quotes = false;
                            }
                        } else {
                            in_quotes = false;
                        }
                    } else {
                        in_quotes = true;
                    }
                }
                ',' => {
                    if in_quotes {
                        current_field.push(',');
                    } else {
                        fields.push(Self::finish_field(current_field, current_field_quoted));
                        current_field = String::new();
                        current_field_quoted = false;
                    }
                }
                _ => {
                    current_field.push(ch);
                }
            }
        }

        fields.push(Self::finish_field(current_field, current_field_quoted));

        if in_quotes {
            return Err(ParserError::Parse(format!(
                "Line {}: Unclosed double quote",
                line_num
            )));
        }

        Ok(fields)
    }

    /// Завершает разбор очередного поля строки.
    ///
    /// Кавычки и удвоенные кавычки разбираются в `parse_line` —
    /// это единственное место, где обрабатывается экранирование. Повторная
    /// обработка значения приводила к потере данных: например, описание
    /// `"in quotes"` теряло внешние кавычки, а `""` превращалось в пустую строку.
    ///
    /// Содержимое закавыченных полей возвращается как есть (пробелы значимы),
    /// незакавыченные поля обрезаются от лишних пробелов (` 1001` → `1001`).
    fn finish_field(value: String, quoted: bool) -> String {
        if quoted {
            value
        } else {
            value.trim().to_string()
        }
    }

    fn validate_headers(headers: &[String]) -> Result<(), ParserError> {
        let expected = [
            "TX_ID",
            "TX_TYPE",
            "FROM_USER_ID",
            "TO_USER_ID",
            "AMOUNT",
            "TIMESTAMP",
            "STATUS",
            "DESCRIPTION",
        ];

        if headers.len() != expected.len() {
            return Err(ParserError::Parse(format!(
                "Expected {} columns, got {}",
                expected.len(),
                headers.len()
            )));
        }

        for (i, (actual, expected)) in headers.iter().zip(expected.iter()).enumerate() {
            if actual != expected {
                return Err(ParserError::Parse(format!(
                    "Column {}: expected '{}', got '{}'",
                    i + 1,
                    expected,
                    actual
                )));
            }
        }

        Ok(())
    }

    fn parse_record(fields: &[String], line_num: usize) -> Result<Transaction, ParserError> {
        if fields.len() != 8 {
            return Err(ParserError::Parse(format!(
                "Line {}: Expected 8 fields, got {}",
                line_num,
                fields.len()
            )));
        }

        let tx_id = fields[0].parse::<u64>().map_err(|e| {
            ParserError::Parse(format!(
                "Line {}: Invalid TX_ID '{}': {}",
                line_num, fields[0], e
            ))
        })?;

        let tx_type = match fields[1].as_str() {
            "DEPOSIT" => TransactionType::Deposit,
            "TRANSFER" => TransactionType::Transfer,
            "WITHDRAWAL" => TransactionType::Withdrawal,
            other => {
                return Err(ParserError::Parse(format!(
                    "Line {}: Invalid TX_TYPE '{}', must be DEPOSIT, TRANSFER, or WITHDRAWAL",
                    line_num, other
                )));
            }
        };

        let from_user_id = fields[2].parse::<u64>().map_err(|e| {
            ParserError::Parse(format!(
                "Line {}: Invalid FROM_USER_ID '{}': {}",
                line_num, fields[2], e
            ))
        })?;

        let to_user_id = fields[3].parse::<u64>().map_err(|e| {
            ParserError::Parse(format!(
                "Line {}: Invalid TO_USER_ID '{}': {}",
                line_num, fields[3], e
            ))
        })?;

        let amount = fields[4].parse::<i64>().map_err(|e| {
            ParserError::Parse(format!(
                "Line {}: Invalid AMOUNT '{}': {}",
                line_num, fields[4], e
            ))
        })?;

        let timestamp = fields[5].parse::<u64>().map_err(|e| {
            ParserError::Parse(format!(
                "Line {}: Invalid TIMESTAMP '{}': {}",
                line_num, fields[5], e
            ))
        })?;

        let status = match fields[6].as_str() {
            "SUCCESS" => TransactionStatus::Success,
            "FAILURE" => TransactionStatus::Failure,
            "PENDING" => TransactionStatus::Pending,
            other => {
                return Err(ParserError::Parse(format!(
                    "Line {}: Invalid STATUS '{}', must be SUCCESS, FAILURE, or PENDING",
                    line_num, other
                )));
            }
        };

        let description = fields[7].clone();

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

    fn escape_description(description: &str) -> String {
        let escaped = description.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    }
}

// Реализуем трейт ParseFromRead для CsvTransactions
impl<R: Read> ParseFromRead<R> for CsvTransactions {
    fn parse(reader: &mut R) -> Result<Self, ParserError> {
        let transactions = CsvParser::parse_records(reader)?;
        Ok(CsvTransactions(transactions))
    }
}

// Реализуем трейт WriteTo для CsvTransactions
impl<W: Write> WriteTo<W> for CsvTransactions {
    fn write(&self, writer: &mut W) -> Result<(), ParserError> {
        CsvParser::write_records(&self.0, writer)
    }
}

// Реализуем трейт WriteTo для среза CsvTransactions
impl<W: Write> WriteTo<W> for [CsvTransactions] {
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
    use std::slice::from_ref;

    const VALID_CSV: &str = r#"TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION
1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,"Initial account funding"
1002,TRANSFER,501,502,15000,1672534800000,FAILURE,"Payment for services"
1003,WITHDRAWAL,502,0,1000,1672538400000,PENDING,"ATM withdrawal""#;

    #[test]
    fn test_parse_valid_csv() {
        let cursor = Cursor::new(VALID_CSV);
        let result = CsvParser::parse_records(cursor);

        assert!(result.is_ok());
        let transactions = result.unwrap();

        assert_eq!(transactions.len(), 3);

        assert_eq!(transactions[0].tx_id, 1001);
        assert!(matches!(transactions[0].tx_type, TransactionType::Deposit));
        assert_eq!(transactions[0].from_user_id, 0);
        assert_eq!(transactions[0].to_user_id, 501);
        assert_eq!(transactions[0].amount, 50000);
        assert_eq!(transactions[0].timestamp, 1672531200000);
        assert!(matches!(transactions[0].status, TransactionStatus::Success));
        assert_eq!(transactions[0].description, "Initial account funding");

        assert_eq!(transactions[1].amount, 15000);
        assert!(matches!(transactions[1].status, TransactionStatus::Failure));

        assert_eq!(transactions[2].amount, 1000);
        assert!(matches!(
            transactions[2].tx_type,
            TransactionType::Withdrawal
        ));
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
        assert_eq!(
            transactions[0].description,
            "Payment for services, invoice #123"
        );
    }

    #[test]
    fn test_parse_csv_with_escaped_quotes() {
        let csv = r#"TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION
1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,"Test with ""quotes"" inside""#;

        let cursor = Cursor::new(csv);
        let result = CsvParser::parse_records(cursor);

        assert!(result.is_ok());
        let transactions = result.unwrap();

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].description, r#"Test with "quotes" inside"#);
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
                tx_type: TransactionType::Withdrawal,
                from_user_id: 501,
                to_user_id: 0,
                amount: 15000,
                timestamp: 1672534800000,
                status: TransactionStatus::Failure,
                description: "Withdrawal with, comma and \"quotes\"".to_string(),
            },
        ];

        let mut buffer = Vec::new();
        let result = CsvParser::write_records(&transactions, &mut buffer);

        assert!(result.is_ok());

        let csv_output = String::from_utf8(buffer).unwrap();

        assert!(csv_output.starts_with(
            "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n"
        ));

        assert!(csv_output.contains("1001,DEPOSIT"));
        assert!(csv_output.contains("1002,WITHDRAWAL"));
        assert!(csv_output.contains("15000"));
        assert!(csv_output.contains("\"Withdrawal with, comma and \"\"quotes\"\"\""));

        let cursor = Cursor::new(csv_output);
        let parsed = CsvParser::parse_records(cursor).unwrap();

        assert_eq!(transactions.len(), parsed.len());
        assert_eq!(transactions[0].tx_id, parsed[0].tx_id);
        assert_eq!(transactions[1].tx_type, parsed[1].tx_type);
        assert_eq!(transactions[1].amount, parsed[1].amount);
        assert_eq!(transactions[1].description, parsed[1].description);
    }

    #[test]
    fn test_roundtrip() {
        let original_transactions = vec![
            Transaction {
                tx_id: 1001,
                tx_type: TransactionType::Deposit,
                from_user_id: 0,
                to_user_id: 501,
                amount: 50000,
                timestamp: 1672531200000,
                status: TransactionStatus::Success,
                description: "Test deposit with \"quotes\" and, commas".to_string(),
            },
            Transaction {
                tx_id: 1002,
                tx_type: TransactionType::Withdrawal,
                from_user_id: 502,
                to_user_id: 0,
                amount: 2000,
                timestamp: 1672538400000,
                status: TransactionStatus::Pending,
                description: "ATM withdrawal".to_string(),
            },
        ];

        let mut buffer = Vec::new();
        CsvParser::write_records(&original_transactions, &mut buffer).unwrap();

        let cursor = Cursor::new(&buffer);
        let parsed_transactions = CsvParser::parse_records(cursor).unwrap();

        assert_eq!(original_transactions, parsed_transactions);
    }

    #[test]
    fn test_parse_unclosed_quote() {
        let csv = r#"TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION
1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,"Unclosed quote"#;

        let cursor = Cursor::new(csv);
        let result = CsvParser::parse_records(cursor);

        assert!(
            matches!(result, Err(ParserError::Parse(msg)) if msg.contains("Unclosed double quote"))
        );
    }

    #[test]
    fn test_parse_empty_lines() {
        let csv = r#"TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION
1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,"First"


1002,TRANSFER,501,502,15000,1672534800000,FAILURE,"Second"

"#;

        let cursor = Cursor::new(csv);
        let result = CsvParser::parse_records(cursor);

        assert!(result.is_ok());
        let transactions = result.unwrap();
        assert_eq!(transactions.len(), 2);
        assert_eq!(transactions[0].tx_id, 1001);
        assert_eq!(transactions[1].tx_id, 1002);
    }

    #[test]
    fn test_parse_large_numbers() {
        let csv = r#"TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION
1000000000000000,DEPOSIT,0,9223372036854775807,100,1633036860000,FAILURE,"Record number 1"
1000000000000002,WITHDRAWAL,599094029349995112,0,300,1633036980000,SUCCESS,"Record number 3""#;

        let cursor = Cursor::new(csv);
        let result = CsvParser::parse_records(cursor);

        assert!(result.is_ok());
        let transactions = result.unwrap();
        assert_eq!(transactions.len(), 2);

        assert_eq!(transactions[0].tx_id, 1000000000000000);
        assert_eq!(transactions[0].from_user_id, 0);
        assert_eq!(transactions[0].to_user_id, 9223372036854775807);
        assert_eq!(transactions[0].amount, 100);

        assert_eq!(transactions[1].tx_id, 1000000000000002);
        assert_eq!(transactions[1].tx_type, TransactionType::Withdrawal);
        assert_eq!(transactions[1].amount, 300);
    }

    #[test]
    fn test_escape_description() {
        assert_eq!(CsvParser::escape_description("Simple"), "\"Simple\"");
        assert_eq!(
            CsvParser::escape_description("With,comma"),
            "\"With,comma\""
        );
        assert_eq!(
            CsvParser::escape_description("With\"quote"),
            "\"With\"\"quote\""
        );
        assert_eq!(
            CsvParser::escape_description("With\nnewline"),
            "\"With\nnewline\""
        );
        assert_eq!(
            CsvParser::escape_description("With\"multiple\"quotes\"and,comma"),
            "\"With\"\"multiple\"\"quotes\"\"and,comma\""
        );
    }

    #[test]
    fn test_parse_quoted_description_is_not_processed_twice() {
        // parse_line уже снимает внешние кавычки и раскрывает удвоенные,
        // повторная обработка значения приводила к потере данных
        let csv = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,\"Simple\"\n1002,DEPOSIT,0,501,50000,1672531200000,SUCCESS,\"With\"\"quote\"\n";

        let parsed = CsvParser::parse_records(Cursor::new(csv)).unwrap();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].description, "Simple");
        assert_eq!(parsed[1].description, "With\"quote");
    }

    #[test]
    fn test_parse_quoted_description_keeps_spaces() {
        let csv = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,\"  spaced  \"\n";

        let parsed = CsvParser::parse_records(Cursor::new(csv)).unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].description, "  spaced  ");
    }

    #[test]
    fn test_parse_unquoted_fields_are_trimmed() {
        // Незакавыченные поля могут содержать лишние пробелы — они обрезаются
        let csv = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n 1001 , DEPOSIT , 0 , 501 , 50000 , 1672531200000 , SUCCESS , Unquoted note  \n";

        let parsed = CsvParser::parse_records(Cursor::new(csv)).unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].tx_id, 1001);
        assert_eq!(parsed[0].tx_type, TransactionType::Deposit);
        assert_eq!(parsed[0].status, TransactionStatus::Success);
        assert_eq!(parsed[0].amount, 50000);
        assert_eq!(parsed[0].description, "Unquoted note");
    }

    #[test]
    fn test_parse_negative_amount_in_csv() {
        let csv = r#"TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION
1001,WITHDRAWAL,501,0,-1000,1672538400000,PENDING,"Test""#;

        let cursor = Cursor::new(csv);
        let result = CsvParser::parse_records(cursor);

        assert!(matches!(result, Err(ParserError::Validation(_))));
        if let Err(ParserError::Validation(msg)) = result {
            assert!(msg.contains("positive"), "unexpected message: {}", msg);
            assert!(msg.contains("Line 2"), "нет контекста строки: {}", msg);
        }
    }

    #[test]
    fn test_write_records_always_quotes() {
        let transaction = Transaction {
            tx_id: 1001,
            tx_type: TransactionType::Deposit,
            from_user_id: 0,
            to_user_id: 501,
            amount: 50000,
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description: "Simple description".to_string(),
        };

        let mut buffer = Vec::new();
        CsvParser::write_records(&[transaction], &mut buffer).unwrap();

        let csv_output = String::from_utf8(buffer).unwrap();

        let lines: Vec<&str> = csv_output.lines().collect();
        assert!(lines.len() >= 2);
        let data_line = lines[1];
        let fields: Vec<&str> = data_line.split(',').collect();
        assert_eq!(fields.len(), 8);

        let description_field = fields[7];
        assert!(description_field.starts_with('"'));
        assert!(description_field.ends_with('"'));
        assert_eq!(description_field, "\"Simple description\"");
    }

    #[test]
    fn test_roundtrip_simple_description() {
        let original = Transaction {
            tx_id: 1001,
            tx_type: TransactionType::Deposit,
            from_user_id: 0,
            to_user_id: 501,
            amount: 50000,
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description: "Record number 1".to_string(),
        };

        let mut buffer = Vec::new();
        CsvParser::write_records(from_ref(&original), &mut buffer).unwrap();

        let csv_output = String::from_utf8(buffer).unwrap();
        println!("CSV output: {}", csv_output);

        assert!(csv_output.contains("\"Record number 1\""));

        let cursor = std::io::Cursor::new(csv_output);
        let parsed = CsvParser::parse_records(cursor).unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].description, "Record number 1");
        assert_eq!(parsed[0].tx_id, original.tx_id);
        assert_eq!(parsed[0].tx_type, original.tx_type);
        assert_eq!(parsed[0].amount, original.amount);
    }

    #[test]
    fn test_parse_csv_description_single_quote() {
        // Поле DESCRIPTION, содержащее ровно одну кавычку, в файле записано как """"
        let csv = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,\"\"\"\"";

        let cursor = Cursor::new(csv);
        let result = CsvParser::parse_records(cursor);

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let transactions = result.unwrap();
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].description, "\"");
    }

    #[test]
    fn test_roundtrip_description_single_quote() {
        let original = Transaction {
            tx_id: 1001,
            tx_type: TransactionType::Deposit,
            from_user_id: 0,
            to_user_id: 501,
            amount: 50000,
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description: "\"".to_string(),
        };

        let mut buffer = Vec::new();
        CsvParser::write_records(from_ref(&original), &mut buffer).unwrap();

        let cursor = Cursor::new(&buffer);
        let parsed = CsvParser::parse_records(cursor).unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0], original);
    }

    #[test]
    fn test_roundtrip_descriptions_with_quotes_and_spaces() {
        // Раньше внешние кавычки и краевые пробелы терялись при чтении
        let descriptions = ["\"in quotes\"", "\"\"", "a\"b\"c", "  spaces  ", "plain"];

        let transactions: Vec<Transaction> = descriptions
            .iter()
            .enumerate()
            .map(|(i, description)| Transaction {
                tx_id: i as u64 + 1,
                tx_type: TransactionType::Deposit,
                from_user_id: 0,
                to_user_id: 501,
                amount: 50000,
                timestamp: 1672531200000,
                status: TransactionStatus::Success,
                description: description.to_string(),
            })
            .collect();

        let mut buffer = Vec::new();
        CsvParser::write_records(&transactions, &mut buffer).unwrap();

        let parsed = CsvParser::parse_records(Cursor::new(&buffer)).unwrap();

        assert_eq!(parsed, transactions);
    }

    #[test]
    fn test_validate_business_rules_with_line_context() {
        // DEPOSIT с ненулевым FROM_USER_ID нарушает бизнес-правила
        let csv = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n1001,DEPOSIT,999,501,50000,1672531200000,SUCCESS,\"Test\"\n";

        let result = CsvParser::parse_records(Cursor::new(csv));

        assert!(matches!(result, Err(ParserError::Validation(_))));
        if let Err(ParserError::Validation(msg)) = result {
            assert!(msg.contains("Line 2"), "нет контекста строки: {}", msg);
            assert!(msg.contains("FROM_USER_ID"), "unexpected message: {}", msg);
        }
    }

    #[test]
    fn test_validate_transfer_to_self() {
        let csv = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n1001,TRANSFER,501,501,50000,1672531200000,SUCCESS,\"Test\"\n";

        let result = CsvParser::parse_records(Cursor::new(csv));

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
        let csv = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n1001,DEPOSIT,999,501,-50000,1672531200000,SUCCESS,\"Invalid\"\n";

        let transactions = CsvParser::parse_records_unvalidated(Cursor::new(csv)).unwrap();

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].from_user_id, 999);
        assert_eq!(transactions[0].amount, -50000);

        // с включённой проверкой тот же ввод отвергается
        assert!(CsvParser::parse_records(Cursor::new(csv)).is_err());
    }
}

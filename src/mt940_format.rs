use crate::{Transaction, TransactionType, TransactionStatus, ParserError};
use std::io::{Read, Write};
use regex::Regex;
use chrono::{DateTime, Utc, NaiveDate, TimeZone};  // Убрал Datelike из импорта
use std::collections::HashMap;

/// Парсер для банковского формата MT940
pub struct MT940Parser;

impl MT940Parser {
    /// Читает все записи из MT940 формата
    pub fn parse_records<R: Read>(reader: R) -> Result<Vec<Transaction>, ParserError> {
        let content = std::io::read_to_string(reader)
            .map_err(ParserError::Io)?;

        let records = Self::parse_mt940_content(&content)?;
        Ok(records)
    }

    /// Парсинг содержимого MT940
    fn parse_mt940_content(content: &str) -> Result<Vec<Transaction>, ParserError> {
        let mut transactions = Vec::new();
        let mut current_record = HashMap::new();
        let mut line_number = 0;
        let mut has_61_field = false; // Флаг, что у нас есть поле :61:

        // Регулярные выражения для парсинга
        let tag_re = Regex::new(r"^:(\d{2}[A-Z]?):").unwrap();
        let field_re = Regex::new(r":(\d{2}[A-Z]?):(.+)").unwrap();

        for line in content.lines() {
            line_number += 1;
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            // Обработка тегов MT940
            if tag_re.is_match(line) {
                if let Some(caps) = field_re.captures(line) {
                    let tag = caps.get(1).unwrap().as_str();
                    let value = caps.get(2).unwrap().as_str();

                    match tag {
                        "20" => {
                            // Сохраняем референс, но не начинаем новую транзакцию
                            current_record.insert("Reference".to_string(), value.to_string());
                            has_61_field = false; // Сбрасываем флаг при новом :20:
                        }
                        "61" => {
                            // Если у нас уже есть транзакция с полем :61:, сохраняем её
                            if has_61_field && !current_record.is_empty() {
                                if let Ok(transaction) = Self::parse_transaction(&current_record, line_number) {
                                    transactions.push(transaction);
                                }
                                current_record.clear();
                                current_record.insert("Reference".to_string(), "".to_string());
                            }

                            has_61_field = true;
                            if let Ok(tx_details) = Self::parse_61_field(value) {
                                current_record.extend(tx_details);
                            }
                        }
                        "86" => {
                            // Информация о транзакции
                            if let Ok(tx_info) = Self::parse_86_field(value) {
                                current_record.extend(tx_info);
                            }
                        }
                        "25" | "28C" | "60F" | "60M" | "62F" | "62M" => {
                            // Эти поля игнорируем - они не являются частью транзакций
                        }
                        _ => {
                            // Игнорируем другие теги для простоты
                        }
                    }
                }
            }
        }

        // Обработка последней транзакции, если у неё есть поле :61:
        if has_61_field && !current_record.is_empty() {
            if let Ok(transaction) = Self::parse_transaction(&current_record, line_number) {
                transactions.push(transaction);
            }
        }

        Ok(transactions)
    }

    /// Парсинг поля :61: - детали транзакции
    fn parse_61_field(value: &str) -> Result<HashMap<String, String>, ParserError> {
        let mut fields = HashMap::new();

        // Формат: ДДММГГ СММГГ D/C СУММА КОД ТРАНЗАКЦИИ // РЕФЕРЕНС
        // Пример: 2502180218D12,01NTRFGSLNVSHSUTKWDR//GI2504900007841
        // Или: 2304200420D12,01NTRF//REF12345

        // Убираем лишние пробелы
        let value = value.trim();

        if value.len() < 10 {
            return Err(ParserError::Parse(
                format!("Invalid :61: field format, too short: '{}'", value)
            ));
        }

        // Дата транзакции (ДДММГГ) - первые 6 символов
        if value.len() >= 6 {
            let date_str = &value[0..6];
            if date_str.chars().all(char::is_numeric) {
                fields.insert("Date".to_string(), date_str.to_string());
            }
        }

        // Ищем позицию D или C (дебет/кредит)
        // Может быть на позиции 10 (6 дата + 4 валютирование) или позже
        let mut dc_pos = None;
        for (i, c) in value.chars().enumerate() {
            if i >= 6 && (c == 'D' || c == 'C') {
                dc_pos = Some(i);
                break;
            }
        }

        if let Some(pos) = dc_pos {
            // Сохраняем D/C маркер
            let dc_marker = value.chars().nth(pos).unwrap();
            fields.insert("DC".to_string(), dc_marker.to_string());

            // Ищем конец суммы (цифры, запятые, точки)
            let mut amount_end = pos + 1;
            while amount_end < value.len() {
                let c = value.chars().nth(amount_end).unwrap();
                if !(c.is_digit(10) || c == ',' || c == '.') {
                    break;
                }
                amount_end += 1;
            }

            if amount_end > pos + 1 {
                let amount_str = &value[pos + 1..amount_end];
                if !amount_str.is_empty() {
                    fields.insert("AmountRaw".to_string(), amount_str.to_string());
                }
            }

            // Ищем код транзакции (после суммы)
            if amount_end < value.len() {
                // Ищем референс после //
                if let Some(double_slash_pos) = value[amount_end..].find("//") {
                    // Текст между суммой и // - это код транзакции
                    let code_str = &value[amount_end..amount_end + double_slash_pos];
                    if !code_str.trim().is_empty() {
                        fields.insert("TransactionCode".to_string(), code_str.trim().to_string());
                    }

                    // Извлекаем референс после //
                    let ref_start = amount_end + double_slash_pos + 2;
                    if ref_start < value.len() {
                        let ref_str = &value[ref_start..];
                        if !ref_str.trim().is_empty() {
                            fields.insert("CustomerReference".to_string(), ref_str.trim().to_string());
                        }
                    }
                } else {
                    // Если нет //, весь оставшийся текст - код транзакции
                    let code_str = &value[amount_end..];
                    if !code_str.trim().is_empty() {
                        fields.insert("TransactionCode".to_string(), code_str.trim().to_string());
                    }
                }
            }
        } else {
            return Err(ParserError::Parse(
                format!("No D/C marker found in :61: field: '{}'", value)
            ));
        }

        Ok(fields)
    }

    /// Парсинг поля :86: - информация о транзакции
    fn parse_86_field(value: &str) -> Result<HashMap<String, String>, ParserError> {
        let mut fields = HashMap::new();

        // Формат: /ПОЛЕ/ЗНАЧЕНИЕ
        let lines: Vec<&str> = value.split('/').collect();

        let mut current_field = String::new();

        for line in lines {
            if line.is_empty() {
                continue;
            }

            if current_field.is_empty() {
                current_field = line.to_string();
            } else {
                let current_value = line.to_string();

                // Сохраняем поле
                match current_field.as_str() {
                    "EREF" => fields.insert("EREF".to_string(), current_value.clone()),
                    "CRNM" | "CNRM" => fields.insert("CounterpartyName".to_string(), current_value.clone()),
                    "CACT" | "DACT" => fields.insert("AccountNumber".to_string(), current_value.clone()),
                    "CBIC" | "DBIC" => fields.insert("BIC".to_string(), current_value.clone()),
                    "REMI" => fields.insert("Description".to_string(), current_value.clone()),
                    "OPRP" => fields.insert("Purpose".to_string(), current_value.clone()),
                    "OAMT" => fields.insert("OriginalAmount".to_string(), current_value.clone()),
                    "DCID" => fields.insert("DebtorId".to_string(), current_value.clone()),
                    _ => {
                        // Если поле неизвестно, добавляем с префиксом
                        fields.insert(format!("Other_{}", current_field), current_value.clone())
                    }
                };

                current_field.clear();
            }
        }

        // Если остался непарный field
        if !current_field.is_empty() {
            fields.insert("Unparsed".to_string(), current_field);
        }

        Ok(fields)
    }

    /// Преобразование HashMap полей в Transaction
    fn parse_transaction(fields: &HashMap<String, String>, line_number: usize) -> Result<Transaction, ParserError> {
        // Проверяем, есть ли обязательные поля для транзакции
        if !fields.contains_key("AmountRaw") && !fields.contains_key("OriginalAmount") {
            return Err(ParserError::Parse(
                format!("Line {}: Transaction must have amount field", line_number)
            ));
        }

        // Извлекаем основные поля
        let tx_id = Self::generate_tx_id(fields);
        let (tx_type, from_user_id, to_user_id) = Self::determine_transfer_type(fields);
        let amount = Self::parse_amount(fields, line_number)?;
        let timestamp = Self::parse_timestamp(fields, line_number)?;
        let status = TransactionStatus::Success; // В MT940 обычно успешные транзакции
        let description = Self::build_description(fields);

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

    /// Генерация ID транзакции на основе полей
    fn generate_tx_id(fields: &HashMap<String, String>) -> u64 {
        // Используем EREF или CustomerReference для генерации ID
        if let Some(eref) = fields.get("EREF") {
            // Простая хэш-функция для строки
            let hash: u64 = eref.bytes().fold(0, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
            hash % 1000000000 // Ограничиваем размер
        } else if let Some(ref_num) = fields.get("CustomerReference") {
            let hash: u64 = ref_num.bytes().fold(0, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
            hash % 1000000000
        } else {
            // Генерация на основе других полей
            let combined = format!("{:?}", fields);
            let hash: u64 = combined.bytes().fold(0, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
            hash % 1000000000
        }
    }

    /// Определение типа транзакции и пользователей
    fn determine_transfer_type(fields: &HashMap<String, String>) -> (TransactionType, u64, u64) {
        // Определяем по полю D/C (Debit/Credit)
        if let Some(dc) = fields.get("DC") {
            match dc.as_str() {
                "D" => {
                    // Дебет - исходящий платеж (Withdrawal или Transfer)
                    if let Some(bic) = fields.get("BIC") {
                        if bic.contains("CITI") || bic.starts_with("DB") {
                            // Перевод между счетами
                            (TransactionType::Transfer, 1000, 2000)
                        } else {
                            // Снятие наличных
                            (TransactionType::Withdrawal, 1000, 0)
                        }
                    } else {
                        (TransactionType::Transfer, 1000, 2000)
                    }
                }
                "C" => {
                    // Кредит - входящий платеж (Deposit)
                    (TransactionType::Deposit, 0, 1000)
                }
                _ => (TransactionType::Transfer, 1000, 2000),
            }
        } else {
            // По умолчанию считаем переводом
            (TransactionType::Transfer, 1000, 2000)
        }
    }

    /// Парсинг суммы
    fn parse_amount(fields: &HashMap<String, String>, line_number: usize) -> Result<i64, ParserError> {
        // Пробуем несколько полей
        let amount_str = fields.get("AmountRaw")
            .or_else(|| fields.get("OriginalAmount"))
            .ok_or_else(|| ParserError::Parse(
                format!("Line {}: No amount field found", line_number)
            ))?;

        // Очищаем строку от запятых и точек
        let cleaned = amount_str.replace(',', ".");

        // Парсим как число с плавающей точкой и конвертируем в центы/копейки
        let amount_f64: f64 = cleaned.parse()
            .map_err(|e| ParserError::Parse(
                format!("Line {}: Invalid amount format '{}': {}", line_number, amount_str, e)
            ))?;

        // Конвертируем в целое число (например, в копейках)
        let amount_i64 = (amount_f64 * 100.0).round() as i64;

        // Корректируем знак в зависимости от D/C
        if let Some(dc) = fields.get("DC") {
            if dc == "D" {
                // Дебет - отрицательная сумма
                Ok(-amount_i64)
            } else {
                // Кредит - положительная сумма
                Ok(amount_i64)
            }
        } else {
            Ok(amount_i64)
        }
    }

    /// Парсинг timestamp
    fn parse_timestamp(fields: &HashMap<String, String>, line_number: usize) -> Result<u64, ParserError> {
        if let Some(date_str) = fields.get("Date") {
            // Формат ДДММГГ (например, 250218 = 25 февраля 2018)
            if date_str.len() == 6 {
                let day: u32 = date_str[0..2].parse()
                    .map_err(|e| ParserError::Parse(
                        format!("Line {}: Invalid day in date '{}': {}", line_number, date_str, e)
                    ))?;
                let month: u32 = date_str[2..4].parse()
                    .map_err(|e| ParserError::Parse(
                        format!("Line {}: Invalid month in date '{}': {}", line_number, date_str, e)
                    ))?;
                let year_short: u32 = date_str[4..6].parse()
                    .map_err(|e| ParserError::Parse(
                        format!("Line {}: Invalid year in date '{}': {}", line_number, date_str, e)
                    ))?;

                // Преобразуем короткий год в полный
                let year = if year_short >= 50 {
                    1900 + year_short
                } else {
                    2000 + year_short
                };

                // Создаем дату - from_ymd_opt возвращает Option, а не Result
                if let Some(date) = NaiveDate::from_ymd_opt(year as i32, month, day) {
                    // and_hms_opt тоже возвращает Option
                    if let Some(datetime) = date.and_hms_opt(12, 0, 0) {
                        // Преобразуем в DateTime<Utc>
                        if let chrono::LocalResult::Single(dt) = Utc.from_local_datetime(&datetime) {
                            let timestamp = dt.timestamp_millis() as u64;
                            Ok(timestamp)
                        } else {
                            Err(ParserError::Parse(
                                format!("Line {}: Invalid timezone conversion for date '{}'", line_number, date_str)
                            ))
                        }
                    } else {
                        // Не должно случиться для валидного времени
                        Err(ParserError::Parse(
                            format!("Line {}: Invalid time for date '{}'", line_number, date_str)
                        ))
                    }
                } else {
                    Err(ParserError::Parse(
                        format!("Line {}: Invalid date '{}' (day={}, month={}, year={})",
                                line_number, date_str, day, month, year)
                    ))
                }
            } else {
                // Если дата не в правильном формате, используем текущее время
                Ok(Utc::now().timestamp_millis() as u64)
            }
        } else {
            // Если даты нет, используем текущее время
            Ok(Utc::now().timestamp_millis() as u64)
        }
    }

    /// Построение описания из полей
    fn build_description(fields: &HashMap<String, String>) -> String {
        let mut description_parts = Vec::new();

        // Добавляем основное описание
        if let Some(remi) = fields.get("Description") {
            description_parts.push(remi.clone());
        }

        // Добавляем назначение платежа
        if let Some(purpose) = fields.get("Purpose") {
            description_parts.push(format!("Purpose: {}", purpose));
        }

        // Добавляем имя контрагента
        if let Some(counterparty) = fields.get("CounterpartyName") {
            description_parts.push(format!("Counterparty: {}", counterparty));
        }

        // Добавляем референс
        if let Some(eref) = fields.get("EREF") {
            description_parts.push(format!("Ref: {}", eref));
        }

        // Добавляем код транзакции
        if let Some(tx_code) = fields.get("TransactionCode") {
            if !tx_code.is_empty() {
                description_parts.push(format!("Code: {}", tx_code));
            }
        }

        if description_parts.is_empty() {
            "MT940 Transaction".to_string()
        } else {
            description_parts.join(" | ")
        }
    }

    /// Записывает транзакции в упрощенный текстовый формат
    /// (MT940 обычно только для чтения, но мы создадим простой вывод для отладки)
    pub fn write_records<W: Write>(records: &[Transaction], writer: &mut W) -> Result<(), ParserError> {
        writeln!(writer, "MT940 Format Export (Simplified)")?;
        writeln!(writer, "=================================")?;

        for (i, record) in records.iter().enumerate() {
            writeln!(writer, "\nTransaction {}:", i + 1)?;
            writeln!(writer, ":20:REF{:010}", record.tx_id)?;

            // Определяем D/C маркер
            let dc = if record.amount < 0 { "D" } else { "C" };
            let amount_abs = (record.amount.abs() as f64) / 100.0;

            // Форматируем дату из timestamp
            let timestamp_millis = record.timestamp as i64;
            // from_timestamp_millis возвращает Option<DateTime>
            let datetime = if let Some(dt) = DateTime::from_timestamp_millis(timestamp_millis) {
                dt
            } else {
                // Если timestamp некорректный, используем текущее время
                Utc::now()
            };
            let date_str = datetime.format("%d%m%y").to_string();

            writeln!(writer, ":61:{}{}{}{:.2}NTRF",
                     date_str,
                     date_str,  // Используем ту же дату для валютирования
                     dc,
                     amount_abs)?;

            writeln!(writer, ":86:/REMI/{}", record.description)?;

            // Добавляем дополнительные поля в зависимости от типа транзакции
            match record.tx_type {
                TransactionType::Deposit => {
                    writeln!(writer, "/CRNM/Deposit from User {}", record.from_user_id)?;
                    writeln!(writer, "/CACT/{:010}", record.to_user_id)?;
                }
                TransactionType::Withdrawal => {
                    writeln!(writer, "/DACT/{:010}", record.from_user_id)?;
                    writeln!(writer, "/DBIC/WITHDRAWAL")?;
                }
                TransactionType::Transfer => {
                    writeln!(writer, "/CRNM/Transfer from User {}", record.from_user_id)?;
                    writeln!(writer, "/CACT/{:010}", record.to_user_id)?;
                }
            }

            writeln!(writer, "/EREF/TX{:010}", record.tx_id)?;
        }

        writeln!(writer, "\n-}}")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;  // Добавляем импорт Datelike

    #[test]
    fn test_parse_mt940_sample() {
        // Тестовые данные с полем :20:
        let simple_mt940 = r#":20:REF123
:61:2304200420D12,01NTRF//REF12345
:86:/REMI/Test Payment
/EREF/REF12345"#;

        let cursor = std::io::Cursor::new(simple_mt940);
        let result = MT940Parser::parse_records(cursor);

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let transactions = result.unwrap();

        // Должна быть одна транзакция
        assert_eq!(transactions.len(), 1);

        // Проверяем что ID сгенерирован
        assert!(transactions[0].tx_id > 0);

        // Проверяем что сумма отрицательная (D - дебет)
        assert!(transactions[0].amount < 0);

        // Проверяем описание
        assert!(transactions[0].description.contains("Test Payment"));
    }

    #[test]
    fn test_parse_multiple_transactions() {
        // Тест с несколькими транзакциями и полем :20:
        let multi_mt940 = r#":20:BATCH001
:61:2304200420D50,00NTRF//REF001
:86:/REMI/Payment 1
/EREF/REF001
:61:2304200420C25,50NTRF//REF002
:86:/REMI/Payment 2
/EREF/REF002"#;

        let cursor = std::io::Cursor::new(multi_mt940);
        let result = MT940Parser::parse_records(cursor);

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let transactions = result.unwrap();

        // Должно быть 2 транзакции
        assert_eq!(transactions.len(), 2);

        // Первая транзакция - дебет (отрицательная)
        assert!(transactions[0].amount < 0);

        // Вторая транзакция - кредит (положительная)
        assert!(transactions[1].amount > 0);
    }

    #[test]
    fn test_parse_61_field_simple() {
        // Тестируем упрощенный формат
        let value = "2304200420D12,01NTRF//REF12345";
        let result = MT940Parser::parse_61_field(value);

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let fields = result.unwrap();

        assert_eq!(fields.get("Date"), Some(&"230420".to_string()));
        assert_eq!(fields.get("DC"), Some(&"D".to_string()));
        assert_eq!(fields.get("AmountRaw"), Some(&"12,01".to_string()));
        assert_eq!(fields.get("CustomerReference"), Some(&"REF12345".to_string()));
    }

    #[test]
    fn test_parse_61_field() {
        let value = "2502180218D12,01NTRFGSLNVSHSUTKWDR//GI2504900007841";
        let result = MT940Parser::parse_61_field(value);

        assert!(result.is_ok());
        let fields = result.unwrap();

        assert_eq!(fields.get("Date"), Some(&"250218".to_string()));
        assert_eq!(fields.get("DC"), Some(&"D".to_string()));
        assert_eq!(fields.get("AmountRaw"), Some(&"12,01".to_string()));
        assert_eq!(fields.get("CustomerReference"), Some(&"GI2504900007841".to_string()));
    }

    #[test]
    fn test_parse_86_field() {
        let value = "/EREF/GSLNVSHSUTKWDR/CRNM/GOLDMAN SACHS BANK USA/CACT/107045863/CBIC/GSCRUS30XXX/REMI/USD Payment to Vendor/OPRP/Tag Payment";
        let result = MT940Parser::parse_86_field(value);

        assert!(result.is_ok());
        let fields = result.unwrap();

        assert_eq!(fields.get("EREF"), Some(&"GSLNVSHSUTKWDR".to_string()));
        assert_eq!(fields.get("CounterpartyName"), Some(&"GOLDMAN SACHS BANK USA".to_string()));
        assert_eq!(fields.get("AccountNumber"), Some(&"107045863".to_string()));
        assert_eq!(fields.get("BIC"), Some(&"GSCRUS30XXX".to_string()));
        assert_eq!(fields.get("Description"), Some(&"USD Payment to Vendor".to_string()));
        assert_eq!(fields.get("Purpose"), Some(&"Tag Payment".to_string()));
    }

    #[test]
    fn test_generate_tx_id() {
        let mut fields = HashMap::new();
        fields.insert("EREF".to_string(), "GSLNVSHSUTKWDR".to_string());

        let tx_id = MT940Parser::generate_tx_id(&fields);
        assert!(tx_id > 0);
        assert!(tx_id < 1000000000);
    }

    #[test]
    fn test_write_records() {
        let transactions = vec![
            Transaction {
                tx_id: 1234567890,
                tx_type: TransactionType::Deposit,
                from_user_id: 0,
                to_user_id: 9876543210,
                amount: 100000, // 1000.00
                timestamp: 1672531200000,
                status: TransactionStatus::Success,
                description: "Test deposit".to_string(),
            },
            Transaction {
                tx_id: 9876543210,
                tx_type: TransactionType::Withdrawal,
                from_user_id: 1234567890,
                to_user_id: 0,
                amount: -50000, // -500.00
                timestamp: 1672534800000,
                status: TransactionStatus::Success,
                description: "Test withdrawal".to_string(),
            },
        ];

        let mut buffer = Vec::new();
        let result = MT940Parser::write_records(&transactions, &mut buffer);

        assert!(result.is_ok());

        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("MT940 Format Export"));
        assert!(output.contains("Transaction 1:"));
        assert!(output.contains("Transaction 2:"));
        assert!(output.contains(":61:"));
        assert!(output.contains(":86:"));
    }

    #[test]
    fn test_parse_amount() {
        let mut fields = HashMap::new();
        fields.insert("AmountRaw".to_string(), "12,01".to_string());
        fields.insert("DC".to_string(), "D".to_string());

        let amount = MT940Parser::parse_amount(&fields, 1);
        assert!(amount.is_ok());
        // 12.01 * 100 = 1201, с отрицательным знаком из-за D
        assert_eq!(amount.unwrap(), -1201);
    }

    #[test]
    fn test_parse_timestamp() {
        let mut fields = HashMap::new();
        fields.insert("Date".to_string(), "250218".to_string());

        let timestamp = MT940Parser::parse_timestamp(&fields, 1);
        assert!(timestamp.is_ok());

        // Проверяем что это разумная дата (25 февраля 2018)
        let ts = timestamp.unwrap();
        if let Some(datetime) = DateTime::from_timestamp_millis(ts as i64) {
            assert_eq!(datetime.year(), 2018);
            assert_eq!(datetime.month(), 2);
            assert_eq!(datetime.day(), 25);
        } else {
            panic!("Invalid timestamp");
        }
    }
}
use crate::{
    BinaryTransactions, ParseFromRead, ParserError, Transaction, TransactionStatus,
    TransactionType, WriteTo,
};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

const MAGIC: [u8; 4] = [0x59, 0x50, 0x42, 0x4E]; // 'YPBN'

/// Парсер для работы с бинарным форматом банковских транзакций.
///
/// `BinaryParser` предоставляет методы для чтения и записи транзакций
/// в бинарном формате. Этот формат является наиболее эффективным по
/// занимаемому месту и скорости обработки.
///
/// # Структура бинарного формата
///
/// Каждая запись в бинарном формате имеет следующую структуру:
///
/// ```text
/// +----------------+----------------+----------------+----------------+
/// | Магическое     | Размер записи  | TX_ID (u64)    | TX_TYPE (u8)   |
/// | число 'YPBN'   | (u32, BE)      | (BE)           |                |
/// | (4 байта)      |                |                |                |
/// +----------------+----------------+----------------+----------------+
/// | FROM_USER_ID   | TO_USER_ID     | AMOUNT (i64)   | TIMESTAMP      |
/// | (u64, BE)      | (u64, BE)      | (BE)           | (u64, BE)      |
/// +----------------+----------------+----------------+----------------+
/// | STATUS (u8)    | Длина описания | ОПИСАНИЕ       |                |
/// |                | (u32, BE)      | (UTF-8)        |                |
/// +----------------+----------------+----------------+----------------+
/// ```
///
/// Где:
/// - BE = Big-Endian порядок байтов
/// - Все числовые поля имеют фиксированный размер
/// - Длина описания ограничена 1 МБ (1,048,576 байт)
/// - Размер записи = 46 байт (фиксированная часть) + длина описания
pub struct BinaryParser;

impl BinaryParser {
    /// Парсит транзакции из бинарного потока данных.
    ///
    /// Читает последовательность бинарных записей из входного потока
    /// и преобразует их в вектор транзакций. Функция читает данные
    /// до конца потока (EOF) или до первой ошибки парсинга.
    pub fn parse_records<R: Read>(mut reader: R) -> Result<Vec<Transaction>, ParserError> {
        let mut records = Vec::new();

        loop {
            match BinaryRecord::from_read(&mut reader) {
                Ok(record) => records.push(record.into()),
                Err(ParserError::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }
        }

        Ok(records)
    }

    /// Записывает транзакции в бинарный формат в записываемый поток
    ///
    /// # Аргументы
    /// * `records` - Список транзакций для записи
    /// * `writer` - Записываемый поток (например, файл или буфер)
    ///
    /// # Возвращает
    /// * `Ok(())` - Успешная запись
    /// * `Err(ParserError)` - Ошибка записи
    pub fn write_records<W: Write>(
        records: &[Transaction],
        writer: &mut W,
    ) -> Result<(), ParserError> {
        for record in records {
            let binary_record: BinaryRecord = record.into();
            binary_record.write_to(writer)?;
        }
        Ok(())
    }
}

// Реализуем трейт ParseFromRead для BinaryTransactions
impl<R: Read> ParseFromRead<R> for BinaryTransactions {
    fn parse(reader: &mut R) -> Result<Self, ParserError> {
        let transactions = BinaryParser::parse_records(reader)?;
        Ok(BinaryTransactions(transactions))
    }
}

// Реализуем трейт WriteTo для BinaryTransactions
impl<W: Write> WriteTo<W> for BinaryTransactions {
    fn write(&self, writer: &mut W) -> Result<(), ParserError> {
        BinaryParser::write_records(&self.0, writer)
    }
}

// Реализуем WriteTo для среза BinaryTransactions
impl<W: Write> WriteTo<W> for [BinaryTransactions] {
    fn write(&self, writer: &mut W) -> Result<(), ParserError> {
        for transactions in self {
            transactions.write(writer)?;
        }
        Ok(())
    }
}

/// Бинарное представление банковской транзакции.
///
/// Структура содержит все поля транзакции в формате, оптимизированном
/// для хранения и передачи. Используется бинарным парсером для
/// чтения и записи данных.
///
/// # Пример
///
/// ```
/// use parser_lib::{BinaryRecord, TransactionType, TransactionStatus};
///
/// let record = BinaryRecord {
///     tx_id: 1001,
///     tx_type: TransactionType::Deposit,
///     from_user_id: 0,
///     to_user_id: 501,
///     amount: 50000,
///     timestamp: 1672531200000,
///     status: TransactionStatus::Success,
///     description: "Initial deposit".to_string(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryRecord {
    /// Уникальный идентификатор транзакции
    pub tx_id: u64,

    /// Тип транзакции (депозит, перевод или вывод)
    pub tx_type: TransactionType,

    /// ID отправителя средств (0 для депозитов)
    pub from_user_id: u64,

    /// ID получателя средств (0 для выводов)
    pub to_user_id: u64,

    /// Сумма транзакции (может быть отрицательной)
    pub amount: i64,

    /// Временная метка в миллисекундах с эпохи UNIX
    pub timestamp: u64,

    /// Статус выполнения транзакции
    pub status: TransactionStatus,

    /// Описание транзакции в UTF-8 (максимум 1 МБ)
    pub description: String,
}

impl BinaryRecord {
    /// Считывает и парсит бинарную запись из потока данных.
    ///
    /// Эта функция читает данные из предоставленного потока в соответствии
    /// с бинарным форматом транзакций и создает экземпляр `BinaryRecord`.
    /// Формат данных должен строго соответствовать спецификации.
    ///
    /// # Алгоритм работы
    ///
    /// 1. Проверяет магическое число 'YPBN' (4 байта)
    /// 2. Читает размер записи (u32, big-endian)
    /// 3. Читает основные поля транзакции (ID, тип, пользователи, сумма, время)
    /// 4. Проверяет статус транзакции
    /// 5. Читает длину описания и само описание в UTF-8
    /// 6. Валидирует размеры и целостность данных
    /// 7. Нормализует описание (убирает кавычки при необходимости)
    ///
    /// # Аргументы
    ///
    /// * `reader` - Мутабельная ссылка на поток, реализующий трейт `Read`.
    ///   Поток должен содержать корректные бинарные данные в формате транзакций.
    ///
    /// # Возвращаемое значение
    ///
    /// * `Ok(BinaryRecord)` - Успешно распарсенная бинарная запись
    /// * `Err(ParserError)` - Ошибка парсинга или ввода-вывода
    ///
    /// # Ошибки
    ///
    /// Функция может вернуть следующие ошибки:
    ///
    /// * `ParserError::Io` - Ошибка чтения из потока
    /// * `ParserError::Parse` с сообщениями:
    ///   - "Invalid magic number" - неверное магическое число
    ///   - "Invalid TX_TYPE" - некорректный тип транзакции
    ///   - "Invalid STATUS" - некорректный статус транзакции
    ///   - "Record size mismatch" - несоответствие размера записи
    ///   - "Description too long" - описание превышает лимит (1 МБ)
    ///   - "Invalid UTF-8 in description" - описание содержит некорректный UTF-8
    ///
    /// # Примеры
    ///
    /// ## Базовое использование
    ///
    /// ```no_run
    /// use parser_lib::BinaryRecord;
    /// use std::fs::File;
    /// use std::io::BufReader;
    ///
    /// fn parse_binary_file() -> Result<(), parser_lib::ParserError> {
    ///     let file = File::open("transactions.bin")?;
    ///     let mut reader = BufReader::new(file);
    ///
    ///     // Чтение первой записи
    ///     let record = BinaryRecord::from_read(&mut reader)?;
    ///     println!("Прочитана транзакция ID: {}", record.tx_id);
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// ## Чтение нескольких записей
    ///
    /// ```no_run
    /// use parser_lib::BinaryRecord;
    /// use std::io::Cursor;
    ///
    /// fn parse_multiple_records(data: &[u8]) -> Result<Vec<BinaryRecord>, parser_lib::ParserError> {
    ///     let mut cursor = Cursor::new(data);
    ///     let mut records = Vec::new();
    ///
    ///     loop {
    ///         match BinaryRecord::from_read(&mut cursor) {
    ///             Ok(record) => records.push(record),
    ///             Err(parser_lib::ParserError::Io(ref e))
    ///                 if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
    ///             Err(e) => return Err(e),
    ///         }
    ///     }
    ///
    ///     Ok(records)
    /// }
    /// ```
    ///
    /// ## Обработка ошибок
    ///
    /// ```no_run
    /// use parser_lib::{BinaryRecord, ParserError};
    /// use std::io::Cursor;
    ///
    /// fn try_parse_record(data: &[u8]) -> Result<(), ParserError> {
    ///     let mut cursor = Cursor::new(data);
    ///
    ///     match BinaryRecord::from_read(&mut cursor) {
    ///         Ok(record) => {
    ///             println!("Успешно: ID={}, сумма={}", record.tx_id, record.amount);
    ///             Ok(())
    ///         }
    ///         Err(ParserError::Parse(msg)) => {
    ///             eprintln!("Ошибка формата: {}", msg);
    ///             Err(ParserError::Parse(msg))
    ///         }
    ///         Err(ParserError::Io(e)) => {
    ///             eprintln!("Ошибка ввода-вывода: {}", e);
    ///             Err(ParserError::Io(e))
    ///         }
    ///         Err(e) => {
    ///             eprintln!("Другая ошибка: {}", e);
    ///             Err(e)
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// # Примечания
    ///
    /// 1. **Порядок байтов**: Все числовые значения читаются в big-endian порядке
    /// 2. **Размеры полей**:
    ///    - `tx_id`, `from_user_id`, `to_user_id`, `timestamp`: 8 байт каждое
    ///    - `amount`: 8 байт со знаком
    ///    - `tx_type`, `status`: по 1 байту
    ///    - Описание: переменной длины (до 1 МБ)
    /// 3. **Магическое число**: Должно быть `[0x59, 0x50, 0x42, 0x4E]` ('YPBN')
    /// 4. **Валидация**: Проверяются все поля на корректность и целостность
    /// 5. **Нормализация описания**: Если описание начинается и заканчивается кавычками,
    ///    они удаляются. Также обрезаются лишние пробелы.
    ///
    /// # Ограничения
    ///
    /// * Максимальная длина описания: 1,048,576 байт (1 МБ)
    /// * Размер записи не может превышать `u32::MAX`
    /// * Поддерживаются только UTF-8 описания
    ///
    /// # Смотрите также
    ///
    /// * [`BinaryParser::parse_records`] - для чтения нескольких записей
    /// * [`BinaryRecord::write_to`] - для записи обратно в поток
    /// * [`BinaryTransactions`] - обертка для работы с коллекцией записей
    pub fn from_read<R: Read>(reader: &mut R) -> Result<Self, ParserError> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;

        if magic != MAGIC {
            return Err(ParserError::Parse(format!(
                "Invalid magic number: {:?}, expected {:?}",
                magic, MAGIC
            )));
        }

        let record_size = reader.read_u32::<BigEndian>()?;

        let tx_id = reader.read_u64::<BigEndian>()?;

        let tx_type_byte = reader.read_u8()?;
        let tx_type = match tx_type_byte {
            0 => TransactionType::Deposit,
            1 => TransactionType::Transfer,
            2 => TransactionType::Withdrawal,
            _ => {
                return Err(ParserError::Parse(format!(
                    "Invalid TX_TYPE: {}",
                    tx_type_byte
                )));
            }
        };

        let from_user_id = reader.read_u64::<BigEndian>()?;

        let to_user_id = reader.read_u64::<BigEndian>()?;

        let amount = reader.read_i64::<BigEndian>()?;

        let timestamp = reader.read_u64::<BigEndian>()?;

        let status_byte = reader.read_u8()?;
        let status = match status_byte {
            0 => TransactionStatus::Success,
            1 => TransactionStatus::Failure,
            2 => TransactionStatus::Pending,
            _ => {
                return Err(ParserError::Parse(format!(
                    "Invalid STATUS: {}",
                    status_byte
                )));
            }
        };

        let desc_len = reader.read_u32::<BigEndian>()?;

        let fixed_size: u64 = 8 +  // tx_id
                        1 +   // tx_type
                        8 +   // from_user_id
                        8 +   // to_user_id
                        8 +   // amount
                        8 +   // timestamp
                        1 +   // status
                        4; // desc_len

        let expected_size = fixed_size.checked_add(desc_len as u64).ok_or_else(|| {
            ParserError::Parse("Record size overflow when calculating total size".to_string())
        })?;

        if record_size as u64 != expected_size {
            return Err(ParserError::Parse(format!(
                "Record size mismatch: header says {}, expected {}",
                record_size, expected_size
            )));
        }

        const MAX_DESC_LEN: u32 = 1024 * 1024;
        if desc_len > MAX_DESC_LEN {
            return Err(ParserError::Parse(format!(
                "Description too long: {} bytes, maximum is {}",
                desc_len, MAX_DESC_LEN
            )));
        }

        let mut description_buf = vec![0u8; desc_len as usize];
        if desc_len > 0 {
            reader.read_exact(&mut description_buf)?;
        }

        let mut description = String::from_utf8(description_buf)
            .map_err(|e| ParserError::Parse(format!("Invalid UTF-8 in description: {}", e)))?;

        description = Self::normalize_description(&description);

        Ok(BinaryRecord {
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

    fn normalize_description(description: &str) -> String {
        let trimmed = description.trim();

        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            trimmed[1..trimmed.len() - 1].to_string()
        } else {
            trimmed.to_string()
        }
    }
    /// Записывает бинарную запись в указанный поток.
    ///
    /// Преобразует структуру в бинарный формат и записывает её в поток.
    /// Формат соответствует спецификации бинарного формата транзакций.
    ///
    /// # Аргументы
    ///
    /// * `writer` - Мутабельная ссылка на поток для записи
    ///
    /// # Возвращает
    ///
    /// * `Ok(())` - Успешная запись
    /// * `Err(ParserError)` - Ошибка записи или валидации
    ///
    /// # Ошибки
    ///
    /// * `ParserError::Io` - Ошибка записи в поток
    /// * `ParserError::Parse` - Описание превышает лимит в 1 МБ
    ///
    /// # Пример
    ///
    /// ```
    /// use parser_lib::{BinaryRecord, TransactionType, TransactionStatus};
    /// use std::io::Cursor;
    ///
    /// let record = BinaryRecord {
    ///     tx_id: 1001,
    ///     tx_type: TransactionType::Deposit,
    ///     from_user_id: 0,
    ///     to_user_id: 501,
    ///     amount: 50000,
    ///     timestamp: 1672531200000,
    ///     status: TransactionStatus::Success,
    ///     description: "Test".to_string(),
    /// };
    ///
    /// let mut buffer = Vec::new();
    /// record.write_to(&mut buffer).unwrap();
    /// ```
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), ParserError> {
        writer.write_all(&MAGIC)?;

        let desc_len = self.description.len() as u32;

        const MAX_DESC_LEN: u32 = 1024 * 1024;
        if desc_len > MAX_DESC_LEN {
            return Err(ParserError::Parse(format!(
                "Description too long: {} bytes, maximum is {}",
                desc_len, MAX_DESC_LEN
            )));
        }

        let fixed_size: u64 = 8 +  // tx_id
                        1 +   // tx_type
                        8 +   // from_user_id
                        8 +   // to_user_id
                        8 +   // amount
                        8 +   // timestamp
                        1 +   // status
                        4; // desc_len

        let record_size = fixed_size.checked_add(desc_len as u64).ok_or_else(|| {
            ParserError::Parse("Record size overflow when calculating total size".to_string())
        })?;

        if record_size > u32::MAX as u64 {
            return Err(ParserError::Parse(
                "Record size exceeds maximum allowed size".to_string(),
            ));
        }

        writer.write_u32::<BigEndian>(record_size as u32)?;

        writer.write_u64::<BigEndian>(self.tx_id)?;

        let tx_type_byte = match self.tx_type {
            TransactionType::Deposit => 0,
            TransactionType::Transfer => 1,
            TransactionType::Withdrawal => 2,
        };
        writer.write_u8(tx_type_byte)?;

        writer.write_u64::<BigEndian>(self.from_user_id)?;
        writer.write_u64::<BigEndian>(self.to_user_id)?;
        writer.write_i64::<BigEndian>(self.amount)?;
        writer.write_u64::<BigEndian>(self.timestamp)?;

        let status_byte = match self.status {
            TransactionStatus::Success => 0,
            TransactionStatus::Failure => 1,
            TransactionStatus::Pending => 2,
        };
        writer.write_u8(status_byte)?;

        writer.write_u32::<BigEndian>(desc_len)?;

        if desc_len > 0 {
            writer.write_all(self.description.as_bytes())?;
        }

        Ok(())
    }
}

impl From<&Transaction> for BinaryRecord {
    fn from(transaction: &Transaction) -> Self {
        BinaryRecord {
            tx_id: transaction.tx_id,
            tx_type: transaction.tx_type,
            from_user_id: transaction.from_user_id,
            to_user_id: transaction.to_user_id,
            amount: transaction.amount,
            timestamp: transaction.timestamp,
            status: transaction.status,
            description: transaction.description.clone(),
        }
    }
}

impl From<Transaction> for BinaryRecord {
    fn from(transaction: Transaction) -> Self {
        BinaryRecord::from(&transaction)
    }
}

impl From<BinaryRecord> for Transaction {
    fn from(record: BinaryRecord) -> Self {
        Transaction {
            tx_id: record.tx_id,
            tx_type: record.tx_type,
            from_user_id: record.from_user_id,
            to_user_id: record.to_user_id,
            amount: record.amount,
            timestamp: record.timestamp,
            status: record.status,
            description: record.description,
        }
    }
}

impl From<&BinaryRecord> for Transaction {
    fn from(record: &BinaryRecord) -> Self {
        Transaction {
            tx_id: record.tx_id,
            tx_type: record.tx_type,
            from_user_id: record.from_user_id,
            to_user_id: record.to_user_id,
            amount: record.amount,
            timestamp: record.timestamp,
            status: record.status,
            description: record.description.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const MAX_DESC_LEN: u32 = 1024 * 1024;

    #[test]
    fn test_binary_record_roundtrip() {
        let original = BinaryRecord {
            tx_id: 123456,
            tx_type: TransactionType::Transfer,
            from_user_id: 100,
            to_user_id: 200,
            amount: 5000,
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description: "Test transaction".to_string(),
        };

        let mut buffer = Vec::new();
        original.write_to(&mut buffer).unwrap();

        let mut cursor = Cursor::new(&buffer);
        let parsed = BinaryRecord::from_read(&mut cursor).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_binary_record_empty_description() {
        let original = BinaryRecord {
            tx_id: 999,
            tx_type: TransactionType::Deposit,
            from_user_id: 0,
            to_user_id: 100,
            amount: 1000,
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description: String::new(),
        };

        let mut buffer = Vec::new();
        original.write_to(&mut buffer).unwrap();

        let mut cursor = Cursor::new(&buffer);
        let parsed = BinaryRecord::from_read(&mut cursor).unwrap();

        assert_eq!(original, parsed);
        assert_eq!(parsed.description, "");
    }

    #[test]
    fn test_invalid_magic() {
        let invalid_data = vec![0x00, 0x00, 0x00, 0x00];
        let mut cursor = Cursor::new(invalid_data);

        let result = BinaryRecord::from_read(&mut cursor);
        assert!(matches!(result, Err(ParserError::Parse(_))));
    }

    #[test]
    fn test_invalid_tx_type() {
        let mut buffer = Vec::new();

        buffer.extend_from_slice(&MAGIC);
        buffer.extend_from_slice(&46u32.to_be_bytes());
        buffer.extend_from_slice(&1001u64.to_be_bytes());
        buffer.push(99);
        buffer.extend_from_slice(&0u64.to_be_bytes());
        buffer.extend_from_slice(&501u64.to_be_bytes());
        buffer.extend_from_slice(&50000i64.to_be_bytes());
        buffer.extend_from_slice(&1672531200000u64.to_be_bytes());
        buffer.push(0); // STATUS
        buffer.extend_from_slice(&0u32.to_be_bytes()); // DESC_LEN = 0

        let mut cursor = Cursor::new(&buffer);
        let result = BinaryRecord::from_read(&mut cursor);

        assert!(matches!(result, Err(ParserError::Parse(_))));
        if let Err(ParserError::Parse(msg)) = result {
            assert!(msg.contains("TX_TYPE"));
        }
    }

    #[test]
    fn test_multiple_records() {
        let records = vec![
            BinaryRecord {
                tx_id: 1001,
                tx_type: TransactionType::Deposit,
                from_user_id: 0,
                to_user_id: 501,
                amount: 50000,
                timestamp: 1672531200000,
                status: TransactionStatus::Success,
                description: "First".to_string(),
            },
            BinaryRecord {
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
        for record in &records {
            record.write_to(&mut buffer).unwrap();
        }

        let mut cursor = Cursor::new(&buffer);
        let parsed_records = BinaryParser::parse_records(&mut cursor).unwrap();

        assert_eq!(parsed_records.len(), 2);
        let transaction1: Transaction = (&records[0]).into();
        let transaction2: Transaction = (&records[1]).into();

        assert_eq!(parsed_records[0], transaction1);
        assert_eq!(parsed_records[1], transaction2);
    }

    #[test]
    fn test_size_overflow_protection() {
        let record = BinaryRecord {
            tx_id: 1001,
            tx_type: TransactionType::Deposit,
            from_user_id: 0,
            to_user_id: 501,
            amount: 50000,
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description: "x".repeat((MAX_DESC_LEN + 100) as usize),
        };

        let mut buffer = Vec::new();
        let result = record.write_to(&mut buffer);
        assert!(matches!(result, Err(ParserError::Parse(_))));
        if let Err(ParserError::Parse(msg)) = result {
            assert!(msg.contains("too long"));
        }
    }

    #[test]
    fn test_size_calculation_overflow() {
        let mut buffer = Vec::new();

        buffer.extend_from_slice(&MAGIC);

        let desc_len = MAX_DESC_LEN + 100;

        let fixed_size: u64 = 46;
        let expected_size = fixed_size + desc_len as u64;

        buffer.extend_from_slice(&(expected_size as u32).to_be_bytes());

        buffer.extend_from_slice(&1u64.to_be_bytes()); // tx_id
        buffer.push(0); // tx_type = DEPOSIT
        buffer.extend_from_slice(&0u64.to_be_bytes()); // from_user_id
        buffer.extend_from_slice(&1u64.to_be_bytes()); // to_user_id
        buffer.extend_from_slice(&1i64.to_be_bytes()); // amount
        buffer.extend_from_slice(&1u64.to_be_bytes()); // timestamp
        buffer.push(0); // status = SUCCESS
        buffer.extend_from_slice(&desc_len.to_be_bytes());

        let mut cursor = Cursor::new(&buffer);
        let result = BinaryRecord::from_read(&mut cursor);

        assert!(matches!(result, Err(ParserError::Parse(_))));

        if let Err(ParserError::Parse(msg)) = result {
            assert!(
                msg.contains("too long"),
                "Expected error about description length, got: '{}'",
                msg
            );
        }
    }

    #[test]
    fn test_valid_large_description() {
        let description = "x".repeat((MAX_DESC_LEN - 100) as usize);

        let record = BinaryRecord {
            tx_id: 1001,
            tx_type: TransactionType::Deposit,
            from_user_id: 0,
            to_user_id: 501,
            amount: 50000,
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description,
        };

        let mut buffer = Vec::new();
        record.write_to(&mut buffer).unwrap();

        let mut cursor = Cursor::new(&buffer);
        let parsed = BinaryRecord::from_read(&mut cursor).unwrap();

        assert_eq!(record.description.len(), parsed.description.len());
        assert_eq!(record, parsed);
    }

    #[test]
    fn test_record_size_exceeds_u32() {
        let mut buffer = Vec::new();

        buffer.extend_from_slice(&MAGIC);

        buffer.extend_from_slice(&0u32.to_be_bytes());

        let mut cursor = Cursor::new(&buffer);
        let result = BinaryRecord::from_read(&mut cursor);

        assert!(matches!(result, Err(_)));
    }
}

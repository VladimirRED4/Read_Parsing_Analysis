use crate::{ParserError, Transaction, TransactionStatus, TransactionType};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

const MAGIC: [u8; 4] = [0x59, 0x50, 0x42, 0x4E]; // 'YPBN'

pub struct BinaryParser;

impl BinaryParser {
    pub fn parse_records<R: Read>(reader: &mut R) -> Result<Vec<Transaction>, ParserError> {
        let mut records = Vec::new();

        loop {
            match BinaryRecord::from_read(reader) {
                Ok(record) => records.push(record.into()),
                Err(ParserError::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }
        }

        Ok(records)
    }

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

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryRecord {
    pub tx_id: u64,
    pub tx_type: TransactionType,
    pub from_user_id: u64,
    pub to_user_id: u64,
    pub amount: i64,
    pub timestamp: u64,
    pub status: TransactionStatus,
    pub description: String,
}

impl BinaryRecord {
    pub fn from_read<R: Read>(reader: &mut R) -> Result<Self, ParserError> {
        // Читаем магическое число
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;

        if magic != MAGIC {
            return Err(ParserError::Parse(format!(
                "Invalid magic number: {:?}, expected {:?}",
                magic, MAGIC
            )));
        }

        // Читаем размер записи
        let record_size = reader.read_u32::<BigEndian>()?;

        // Читаем тело записи
        // TX_ID
        let tx_id = reader.read_u64::<BigEndian>()?;

        // TX_TYPE
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

        // FROM_USER_ID
        let from_user_id = reader.read_u64::<BigEndian>()?;

        // TO_USER_ID
        let to_user_id = reader.read_u64::<BigEndian>()?;

        // AMOUNT
        let amount = reader.read_i64::<BigEndian>()?;

        // TIMESTAMP
        let timestamp = reader.read_u64::<BigEndian>()?;

        // STATUS
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

        // DESC_LEN
        let desc_len = reader.read_u32::<BigEndian>()?;

        // Проверяем соответствие размера
        let expected_size = 8 + 1 + 8 + 8 + 8 + 8 + 1 + 4 + desc_len as u64;
        if record_size as u64 != expected_size {
            return Err(ParserError::Parse(format!(
                "Record size mismatch: header says {}, expected {}",
                record_size, expected_size
            )));
        }

        // DESCRIPTION
        let mut description_buf = vec![0u8; desc_len as usize];
        if desc_len > 0 {
            reader.read_exact(&mut description_buf)?;
        }

        let mut description = String::from_utf8(description_buf)
            .map_err(|e| ParserError::Parse(format!("Invalid UTF-8 in description: {}", e)))?;

        // Убираем окружающие кавычки из описания, если они есть
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

    /// Нормализует описание, убирая лишние окружающие кавычки
    fn normalize_description(description: &str) -> String {
        let trimmed = description.trim();

        // Если описание начинается и заканчивается кавычками, убираем их
        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            trimmed[1..trimmed.len() - 1].to_string()
        } else {
            trimmed.to_string()
        }
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), ParserError> {
        // Записываем магическое число
        writer.write_all(&MAGIC)?;

        // Вычисляем размер записи
        let desc_len = self.description.len() as u32;
        let record_size = 8 + 1 + 8 + 8 + 8 + 8 + 1 + 4 + desc_len;

        // Записываем размер
        writer.write_u32::<BigEndian>(record_size)?;

        // Записываем поля
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

// Реализация преобразований
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
        // Создаем бинарные данные с неверным TX_TYPE
        let mut buffer = Vec::new();

        // MAGIC
        buffer.extend_from_slice(&MAGIC);
        // RECORD_SIZE (минимальный размер)
        buffer.extend_from_slice(&46u32.to_be_bytes());
        // TX_ID
        buffer.extend_from_slice(&1001u64.to_be_bytes());
        // TX_TYPE = 99 (неверный)
        buffer.push(99);
        // Остальные поля (минимальные)
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
}

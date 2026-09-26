#![warn(missing_docs)]

//! Библиотека для парсинга и конвертации банковских транзакций в различных форматах.
//!
//! Эта библиотека предоставляет парсеры для трёх форматов:
//! - CSV (Comma-Separated Values)
//! - Текстовый формат (key-value pairs)
//! - Бинарный формат (custom binary format)
//!
//! # Основные возможности
//!
//! - Парсинг транзакций из CSV, текстового и бинарного форматов
//! - Запись транзакций в CSV, текстовый и бинарный форматы
//! - Валидация бизнес-правил транзакций
//! - Конвертация между форматами
//! - Сравнение транзакций из разных файлов
//!
//! # Форматы данных
//!
//! ## CSV формат
//! - Первая строка содержит заголовки
//! - Значения разделяются запятыми
//! - Описания экранируются двойными кавычками
//!
//! ## Текстовый формат
//! - Каждая запись состоит из пар "KEY: VALUE"
//! - Поддерживает комментарии (строки, начинающиеся с #)
//! - Пустые строки разделяют записи
//! - Описания должны быть в двойных кавычках
//!
//! ## Бинарный формат
//! - Начинается с магического числа 'YPBN' (0x59 0x50 0x42 0x4E)
//! - Все числа записываются в big-endian порядке
//! - Поддерживает отрицательные суммы
//! - Имеет встроенную проверку целостности

mod binary_format;
mod csv_format;
mod error;
mod txt_format;

pub use binary_format::{BinaryParser, BinaryRecord};
pub use csv_format::CsvParser;
pub use error::ParserError;
pub use txt_format::TextParser;

use std::io::{Read, Write};

/// Трейт для парсинга данных из читаемого потока
///
/// Этот трейт предоставляет единый интерфейс для парсинга
/// данных различных форматов. Каждый парсер должен реализовывать
/// этот трейт для своего формата.
///
/// # Типовой параметр
/// * `R` - Тип читаемого потока, должен реализовывать `Read`
///
pub trait ParseFromRead<R: Read> {
    /// Парсит данные из читаемого потока
    ///
    /// # Аргументы
    /// * `reader` - Читаемый поток
    ///
    /// # Возвращает
    /// * `Ok(Self)` - Успешно распарсенные данные
    /// * `Err(ParserError)` - Ошибка парсинга
    fn parse(reader: &mut R) -> Result<Self, ParserError>
    where
        Self: Sized;
}

/// Трейт для записи данных в записываемый поток
///
/// Этот трейт предоставляет единый интерфейс для записи
/// данных в различных форматах. Каждый парсер должен реализовывать
/// этот трейт для своего формата.
///
/// # Типовой параметр
/// * `W` - Тип записываемого потока, должен реализовывать `Write`
///
/// let mut buffer = Vec::new();
/// CsvParser::write(&transactions, &mut buffer).unwrap();
///
/// let output = String::from_utf8(buffer).unwrap();
/// assert!(output.contains("1001,DEPOSIT"));
/// ```
pub trait WriteTo<W: Write> {
    /// Записывает данные в записываемый поток
    ///
    /// # Аргументы
    /// * `writer` - Записываемый поток
    ///
    /// # Возвращает
    /// * `Ok(())` - Успешная запись
    /// * `Err(ParserError)` - Ошибка записи
    fn write(&self, writer: &mut W) -> Result<(), ParserError>;
}

/// Представляет банковскую транзакцию
///
/// Транзакция содержит все данные о финансовой операции,
/// включая тип, сумму, участников и статус выполнения.
#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    /// Уникальный идентификатор транзакции
    ///
    /// Используется для однозначной идентификации операции в системе.
    /// Должен быть уникальным в рамках системы.
    pub tx_id: u64,

    /// Тип транзакции
    ///
    /// Определяет природу финансовой операции.
    pub tx_type: TransactionType,

    /// ID пользователя-отправителя
    ///
    /// Для депозитов всегда равен 0 (система -> пользователь).
    /// Для переводов и выводов должен быть ненулевым.
    pub from_user_id: u64,

    /// ID пользователя-получателя
    ///
    /// Для выводов всегда равен 0 (пользователь -> система).
    /// Для депозитов и переводов должен быть ненулевым.
    pub to_user_id: u64,

    /// Сумма транзакции
    ///
    /// В CSV и текстовом форматах всегда положительная.
    /// В бинарном формате может быть отрицательной для
    /// отражения направления движения средств.
    ///
    /// # Примечание
    /// Для вывода средств рекомендуется использовать положительные значения,
    /// а направление определять по типу транзакции.
    pub amount: i64,

    /// Временная метка транзакции
    ///
    /// Представляет количество миллисекунд с эпохи UNIX
    /// (1 января 1970 года, 00:00:00 UTC).
    pub timestamp: u64,

    /// Статус выполнения транзакции
    ///
    /// Отражает текущее состояние обработки операции.
    pub status: TransactionStatus,

    /// Описание транзакции
    ///
    /// Произвольный текст, описывающий назначение платежа.
    /// Может содержать специальные символы, кавычки и запятые.
    /// В CSV формате экранируется двойными кавычками.
    pub description: String,
}

impl Transaction {
    /// Проверяет бизнес-правила транзакции.
    ///
    /// Правила определяются типом транзакции:
    ///
    /// - [`TransactionType::Deposit`]: `from_user_id == 0` и `to_user_id != 0`
    /// - [`TransactionType::Transfer`]: `from_user_id != 0`, `to_user_id != 0`
    ///   и `from_user_id != to_user_id`
    /// - [`TransactionType::Withdrawal`]: `from_user_id != 0` и `to_user_id == 0`
    ///
    /// Знак суммы здесь не проверяется: в бинарном формате он кодирует направление
    /// движения средств (см. [`Transaction::amount`]), поэтому требование
    /// положительной суммы предъявляют только текстовые форматы — см.
    /// [`Transaction::validate_positive_amount`].
    ///
    /// Метод является единой точкой проверки бизнес-правил: его вызывают
    /// [`CsvParser`], [`TextParser`] и [`BinaryParser`].
    ///
    /// # Ошибки
    ///
    /// Возвращает [`ParserError::Validation`] с описанием первого нарушенного правила.
    ///
    /// # Пример
    ///
    /// ```
    /// use parser_lib::{Transaction, TransactionStatus, TransactionType};
    ///
    /// let transaction = Transaction {
    ///     tx_id: 1001,
    ///     tx_type: TransactionType::Transfer,
    ///     from_user_id: 501,
    ///     to_user_id: 502,
    ///     amount: 50000,
    ///     timestamp: 1672531200000,
    ///     status: TransactionStatus::Success,
    ///     description: String::new(),
    /// };
    ///
    /// assert!(transaction.validate().is_ok());
    ///
    /// // Перевод самому себе нарушает бизнес-правила
    /// let invalid = Transaction { to_user_id: 501, ..transaction };
    /// assert!(invalid.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), ParserError> {
        match self.tx_type {
            TransactionType::Deposit => {
                if self.from_user_id != 0 {
                    return Err(ParserError::Validation(format!(
                        "DEPOSIT must have FROM_USER_ID = 0, got {}",
                        self.from_user_id
                    )));
                }
                if self.to_user_id == 0 {
                    return Err(ParserError::Validation(
                        "DEPOSIT must have TO_USER_ID != 0".to_string(),
                    ));
                }
            }
            TransactionType::Transfer => {
                if self.from_user_id == 0 {
                    return Err(ParserError::Validation(
                        "TRANSFER must have FROM_USER_ID != 0".to_string(),
                    ));
                }
                if self.to_user_id == 0 {
                    return Err(ParserError::Validation(
                        "TRANSFER must have TO_USER_ID != 0".to_string(),
                    ));
                }
                if self.from_user_id == self.to_user_id {
                    return Err(ParserError::Validation(format!(
                        "TRANSFER must have FROM_USER_ID != TO_USER_ID, got {}",
                        self.from_user_id
                    )));
                }
            }
            TransactionType::Withdrawal => {
                if self.to_user_id != 0 {
                    return Err(ParserError::Validation(format!(
                        "WITHDRAWAL must have TO_USER_ID = 0, got {}",
                        self.to_user_id
                    )));
                }
                if self.from_user_id == 0 {
                    return Err(ParserError::Validation(
                        "WITHDRAWAL must have FROM_USER_ID != 0".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Проверяет, что сумма транзакции положительна.
    ///
    /// CSV и текстовый формат хранят только положительные суммы, поэтому их парсеры
    /// вызывают эту проверку вместе с [`Transaction::validate`]. Бинарный формат
    /// знак суммы использует для кодирования направления движения средств и эту
    /// проверку не применяет.
    ///
    /// # Ошибки
    ///
    /// Возвращает [`ParserError::Validation`], если `amount <= 0`.
    ///
    /// # Пример
    ///
    /// ```
    /// use parser_lib::{Transaction, TransactionStatus, TransactionType};
    ///
    /// let transaction = Transaction {
    ///     tx_id: 1001,
    ///     tx_type: TransactionType::Deposit,
    ///     from_user_id: 0,
    ///     to_user_id: 501,
    ///     amount: 50000,
    ///     timestamp: 1672531200000,
    ///     status: TransactionStatus::Success,
    ///     description: String::new(),
    /// };
    ///
    /// assert!(transaction.validate_positive_amount().is_ok());
    ///
    /// let invalid = Transaction { amount: -1, ..transaction };
    /// assert!(invalid.validate_positive_amount().is_err());
    /// ```
    pub fn validate_positive_amount(&self) -> Result<(), ParserError> {
        if self.amount <= 0 {
            return Err(ParserError::Validation(format!(
                "AMOUNT must be positive, got {}",
                self.amount
            )));
        }

        Ok(())
    }
}

// lib.rs - добавляем после определения Transaction

/// Обертка для парсинга CSV формата
pub struct CsvTransactions(pub Vec<Transaction>);

/// Обертка для парсинга текстового формата
pub struct TextTransactions(pub Vec<Transaction>);

/// Обертка для парсинга бинарного формата
pub struct BinaryTransactions(pub Vec<Transaction>);

/// Типы банковских транзакций
///
/// Определяет природу финансовой операции и правила валидации.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionType {
    /// Депозит (пополнение счёта)
    ///
    /// Деньги поступают от системы к пользователю.
    /// Бизнес-правила:
    /// - `from_user_id` должен быть равен 0
    /// - `to_user_id` должен быть ненулевым
    /// - `amount` должен быть положительным
    Deposit,

    /// Перевод между пользователями
    ///
    /// Деньги переводятся от одного пользователя к другому.
    /// Бизнес-правила:
    /// - `from_user_id` должен быть ненулевым
    /// - `to_user_id` должен быть ненулевым
    /// - `from_user_id` и `to_user_id` не должны совпадать
    /// - `amount` должен быть положительным
    Transfer,

    /// Вывод средств
    ///
    /// Деньги выводятся от пользователя к системе.
    /// Бизнес-правила:
    /// - `from_user_id` должен быть ненулевым
    /// - `to_user_id` должен быть равен 0
    /// - `amount` должен быть положительным
    Withdrawal,
}

/// Статусы выполнения транзакций
///
/// Отражает текущее состояние обработки операции в системе.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionStatus {
    /// Транзакция успешно выполнена
    ///
    /// Операция завершена, средства переведены,
    /// все проверки пройдены успешно.
    Success,

    /// Транзакция не выполнена (ошибка)
    ///
    /// Операция не выполнена из-за ошибки:
    /// - недостаточно средств
    /// - техническая ошибка
    /// - нарушение бизнес-правил
    Failure,

    /// Транзакция в процессе обработки
    ///
    /// Операция принята системой, но ещё не обработана.
    /// Может перейти в статус Success или Failure.
    Pending,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_transaction(
        tx_type: TransactionType,
        from_user_id: u64,
        to_user_id: u64,
        amount: i64,
    ) -> Transaction {
        Transaction {
            tx_id: 1001,
            tx_type,
            from_user_id,
            to_user_id,
            amount,
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description: "Test".to_string(),
        }
    }

    #[test]
    fn test_validate_allows_valid_transactions() {
        assert!(
            make_transaction(TransactionType::Deposit, 0, 501, 50000)
                .validate()
                .is_ok()
        );
        assert!(
            make_transaction(TransactionType::Transfer, 501, 502, 5000)
                .validate()
                .is_ok()
        );
        assert!(
            make_transaction(TransactionType::Withdrawal, 501, 0, 1000)
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn test_validate_deposit() {
        let with_sender = make_transaction(TransactionType::Deposit, 999, 501, 50000);
        assert!(matches!(
            with_sender.validate(),
            Err(ParserError::Validation(msg)) if msg.contains("FROM_USER_ID")
        ));

        let without_receiver = make_transaction(TransactionType::Deposit, 0, 0, 50000);
        assert!(matches!(
            without_receiver.validate(),
            Err(ParserError::Validation(msg)) if msg.contains("TO_USER_ID")
        ));
    }

    #[test]
    fn test_validate_transfer() {
        let without_sender = make_transaction(TransactionType::Transfer, 0, 502, 5000);
        assert!(matches!(
            without_sender.validate(),
            Err(ParserError::Validation(msg)) if msg.contains("FROM_USER_ID")
        ));

        let without_receiver = make_transaction(TransactionType::Transfer, 501, 0, 5000);
        assert!(matches!(
            without_receiver.validate(),
            Err(ParserError::Validation(msg)) if msg.contains("TO_USER_ID")
        ));

        let to_self = make_transaction(TransactionType::Transfer, 501, 501, 5000);
        assert!(matches!(
            to_self.validate(),
            Err(ParserError::Validation(msg)) if msg.contains("FROM_USER_ID != TO_USER_ID")
        ));
    }

    #[test]
    fn test_validate_withdrawal() {
        let with_receiver = make_transaction(TransactionType::Withdrawal, 501, 999, 1000);
        assert!(matches!(
            with_receiver.validate(),
            Err(ParserError::Validation(msg)) if msg.contains("TO_USER_ID")
        ));

        let without_sender = make_transaction(TransactionType::Withdrawal, 0, 0, 1000);
        assert!(matches!(
            without_sender.validate(),
            Err(ParserError::Validation(msg)) if msg.contains("FROM_USER_ID")
        ));
    }

    #[test]
    fn test_validate_ignores_amount_sign() {
        // Знак суммы не входит в бизнес-правила: его проверяют только текстовые формы
        let transaction = make_transaction(TransactionType::Transfer, 501, 502, -5000);

        assert!(transaction.validate().is_ok());
        assert!(matches!(
            transaction.validate_positive_amount(),
            Err(ParserError::Validation(msg)) if msg.contains("positive")
        ));
    }

    #[test]
    fn test_validate_positive_amount_rejects_zero() {
        let transaction = make_transaction(TransactionType::Deposit, 0, 501, 0);

        assert!(transaction.validate().is_ok());
        assert!(matches!(
            transaction.validate_positive_amount(),
            Err(ParserError::Validation(_))
        ));
    }
}

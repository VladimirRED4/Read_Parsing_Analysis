use std::fmt;

/// Ошибки, возникающие при парсинге и обработке транзакций.
///
/// Этот enum объединяет все возможные ошибки, которые могут возникнуть
/// при работе с библиотекой парсинга транзакций.
///
/// # Примеры
///
/// ```no run
/// use parser_lib::ParserError;
///
/// // Создание ошибки парсинга
/// let parse_error = ParserError::Parse("Некорректный формат TX_ID".to_string());
///
/// // Создание ошибки валидации
/// let validation_error = ParserError::Validation("Сумма не может быть отрицательной".to_string());
/// ```
#[derive(Debug)]
pub enum ParserError {
    /// Ошибка ввода-вывода при чтении или записи файлов.
    ///
    /// Возникает при проблемах с доступом к файлам, чтением или записью данных.
    Io(std::io::Error),

    /// Ошибка парсинга данных.
    ///
    /// Возникает при несоответствии данных ожидаемому формату,
    /// синтаксическим ошибкам или некорректным значениям полей.
    Parse(String),

    /// Ошибка валидации бизнес-правил.
    ///
    /// Возникает при нарушении бизнес-логики транзакций:
    /// - Неправильные типы транзакций для заданных ID пользователей
    /// - Некорректные суммы
    /// - Нарушение других бизнес-ограничений
    Validation(String),

    /// Неподдерживаемый формат данных.
    ///
    /// Возникает при попытке работы с форматом, который не поддерживается библиотекой.
    UnsupportedFormat,

    /// Ошибка конвертации между форматами или типами.
    ///
    /// Возникает при проблемах преобразования данных между разными представлениями.
    Conversion(String),
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParserError::Io(e) => write!(f, "IO error: {}", e),
            ParserError::Parse(msg) => write!(f, "Parse error: {}", msg),
            ParserError::Validation(msg) => write!(f, "Validation error: {}", msg),
            ParserError::UnsupportedFormat => write!(f, "Unsupported format"),
            ParserError::Conversion(msg) => write!(f, "Conversion error: {}", msg),
        }
    }
}

impl std::error::Error for ParserError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParserError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ParserError {
    /// Преобразует ошибку ввода-вывода в `ParserError::Io`.
    ///
    /// Это позволяет использовать оператор `?` с функциями ввода-вывода
    /// и автоматически преобразовывать ошибки.
    ///
    /// # Пример
    ///
    /// ```no run
    /// use parser_lib::ParserError;
    /// use std::fs::File;
    ///
    /// fn read_file() -> Result<(), ParserError> {
    ///     let file = File::open("несуществующий_файл.txt")?; // Автоматически преобразуется в ParserError
    ///     Ok(())
    /// }
    /// ```
    fn from(error: std::io::Error) -> Self {
        ParserError::Io(error)
    }
}

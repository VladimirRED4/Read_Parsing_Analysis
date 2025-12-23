# Read Parsing Analysis

![Tests](https://github.com/VladimirRED4/Read_Parsing_Analysis/actions/workflows/rust.yml/badge.svg)
![Security Audit](https://github.com/VladimirRED4/Read_Parsing_Analysis/actions/workflows/audit.yml/badge.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)

## YPBank Transaction Parser & Converter

Библиотека и утилиты для работы с транзакционными данными в различных форматах.

## Форматы

- **CSV** - стандартный CSV с заголовком
- **Text** (YPBankText) - ключ-значение с комментариями
- **Binary** - бинарный формат с магическим числом `YPBN`

## Установка

```bash
git clone <repository-url>
cd read_parsing_analysis
cargo build --release
```

## Использование

### 1. Конвертер форматов (ypbank_converter)

#### Конвертирует файлы между поддерживаемыми форматами

```bash
# CSV -> Text (вывод в stdout)
cargo run --bin ypbank_converter -- --input examples/records_example.csv --input-format csv --output-format txt

# Text -> CSV (в файл)
cargo run --bin ypbank_converter -- --input examples/records_example.txt --input-format txt --output-format csv --output output.csv

# Binary -> Text (в файл)
cargo run --bin ypbank_converter -- --input examples/records_example.bin --input-format bin --output-format txt --output output.txt

# CSV -> Binary (обязательно указывать --output)
cargo run --bin ypbank_converter -- --input examples/records_example.csv --input-format csv --output-format bin --output output.bin
```

### 2. Компаратор файлов (comparer)

#### Сравнивает из двух файлов в разных форматах

```bash
# Сравнение бинарного и CSV файлов
cargo run --bin comparer -- --file1 examples/records_example.bin --format1 bin --file2 examples/records_example.csv --format2 csv

# Сравнение с подробным выводом
cargo run --bin comparer -- --file1 file1.csv --format1 csv --file2 file2.txt --format2 txt --verbose

# Игнорировать различия в описании
cargo run --bin comparer -- --file1 data1.bin --format1 bin --file2 data2.csv --format2 csv --ignore-description

# Игнорировать различия в статусе
cargo run --bin comparer -- --file1 data1.txt --format1 txt --file2 data2.csv --format2 csv --ignore-status
```

## Примеры файлов

В корне проекта необходимо создать папку `examples` в которой разместить тестовые файлы в разных форматах:

- **records_example.csv** - CSV формат
- **records_example.txt** - текстовый формат
- **records_example.bin** - бинарный формат

все три формата содержат одинаковые данные для тестирования конвертации и сравнения.

## Структура проекта

```text
read_parsing_analysis/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs              # Основная библиотека
│   ├── main.rs             # Конвертер (ypbank_converter)
│   ├── binary_format.rs    # Парсер бинарного формата
│   ├── csv_format.rs       # Парсер CSV формата
│   ├── txt_format.rs       # Парсер текстового формата
│   ├── error.rs            # Обработка ошибок
│   └── bin/
│       ├── comparer.rs     # Компаратор файлов
│       ├── test_binary.rs  # Тестовые утилиты
│       ├── test_csv.rs
│       ├── test_txt.rs
│       └── debug_binary.rs
├── examples/               # Примеры файлов
│   ├── records_example.csv
│   ├── records_example.txt
│   └── records_example.bin
└── tests/                  # Интеграционные тесты
    ├── parser_integration.rs
    ├── binary_integration.rs
    └── comparer_integration.rs
```

## API библиотеки

```rust
use parser_lib::{CsvParser, TextParser, BinaryParser, Transaction};

// Чтение из CSV
let file = File::open("data.csv")?;
let transactions = CsvParser::parse_records(file)?;

// Запись в Text формат
let mut buffer = Vec::new();
TextParser::write_records(&transactions, &mut buffer)?;

// Чтение из бинарного формата
let mut reader = BufReader::new(File::open("data.bin")?);
let transactions = BinaryParser::parse_records(&mut reader)?;
```

## Тестирование

```bash
# Все тесты
cargo test

# Конкретные тесты
cargo test --lib
cargo test --test parser_integration
cargo test --test comparer_integration

# Запуск отдельных утилит
cargo run --bin test_csv
```

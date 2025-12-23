use clap::Parser;
use parser_lib::{BinaryParser, CsvParser, TextParser, Transaction};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "ypbank_converter")]
#[command(version = "1.0")]
#[command(about = "Конвертирует файлы между форматами YPBank (CSV, Text, Binary)", long_about = None)]
struct Args {
    #[arg(short, long, value_name = "FILE")]
    input: PathBuf,

    #[arg(
        long = "input-format",
        value_name = "FORMAT",
        value_enum,
        ignore_case = true
    )]
    input_format: Format,

    #[arg(
        long = "output-format",
        value_name = "FORMAT",
        value_enum,
        ignore_case = true
    )]
    output_format: Format,

    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    #[arg(long, default_value_t = false)]
    skip_validation: bool,
}

#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
enum Format {
    Csv,
    Txt,
    Bin,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if !args.input.exists() {
        eprintln!("Ошибка: входной файл '{}' не найден", args.input.display());

        let examples_dir = Path::new("examples");
        if examples_dir.exists() {
            eprintln!("Доступные примеры файлов в папке 'examples/':");
            for entry in std::fs::read_dir(examples_dir)?.flatten() {
                if let Some(ext) = entry.path().extension() {
                    let ext_str = ext.to_string_lossy();
                    let format = match ext_str.to_lowercase().as_str() {
                        "csv" => "CSV",
                        "txt" => "Text",
                        "bin" => "Binary",
                        _ => "Unknown",
                    };
                    eprintln!("  - {} ({})", entry.path().display(), format);
                }
            }
            eprintln!("\nПример использования:");
            eprintln!(
                "  ypbank_converter --input examples/records_example.csv --input-format csv --output-format txt"
            );
        }
        std::process::exit(1);
    }

    if args.verbose {
        eprintln!("=== YPBank Converter ===");
        eprintln!("Входной файл: {}", args.input.display());
        eprintln!("Входной формат: {:?}", args.input_format);
        eprintln!("Выходной формат: {:?}", args.output_format);
        if let Some(output) = &args.output {
            eprintln!("Выходной файл: {}", output.display());
        } else {
            eprintln!("Выходной файл: <stdout>");
        }
        if args.skip_validation {
            eprintln!("Режим: пропуск проверки бизнес-правил");
        }
    }

    let transactions = read_transactions(&args.input, &args.input_format, args.skip_validation)?;

    if args.verbose {
        eprintln!("Прочитано {} транзакций", transactions.len());
        if !transactions.is_empty() {
            eprintln!(
                "Первая транзакция: ID={}, Тип={:?}, Сумма={}, Статус={:?}",
                transactions[0].tx_id,
                transactions[0].tx_type,
                transactions[0].amount,
                transactions[0].status
            );
            if transactions.len() > 1 {
                eprintln!(
                    "Последняя транзакция: ID={}, Тип={:?}, Сумма={}",
                    transactions.last().unwrap().tx_id,
                    transactions.last().unwrap().tx_type,
                    transactions.last().unwrap().amount
                );
            }
        }
    }

    write_transactions(
        &transactions,
        &args.output_format,
        args.output.as_ref(),
        args.verbose,
    )?;

    if args.verbose {
        eprintln!("Конвертация завершена успешно!");
    }

    Ok(())
}

fn read_transactions(
    input_path: &Path,
    format: &Format,
    skip_validation: bool,
) -> Result<Vec<Transaction>, Box<dyn std::error::Error>> {
    let file = File::open(input_path)
        .map_err(|e| format!("Не удалось открыть файл '{}': {}", input_path.display(), e))?;

    if skip_validation {
        eprintln!("Предупреждение: проверка бизнес-правил отключена");
    }

    match format {
        Format::Csv => {
            let file = File::open(input_path)
                .map_err(|e| format!("Не удалось открыть CSV файл: {}", e))?;

            CsvParser::parse_records(file).map_err(|e| format!("Ошибка парсинга CSV: {}", e).into())
        }
        Format::Txt => {
            let file = File::open(input_path)
                .map_err(|e| format!("Не удалось открыть текстовый файл: {}", e))?;

            TextParser::parse_records(file)
                .map_err(|e| format!("Ошибка парсинга текстового файла: {}", e).into())
        }
        Format::Bin => {
            let mut reader = BufReader::new(file);

            let mut magic = [0u8; 4];
            if reader.read_exact(&mut magic).is_ok() {
                let expected_magic = [0x59, 0x50, 0x42, 0x4E]; // "YPBN"
                if magic != expected_magic {
                    eprintln!(
                        "Предупреждение: файл не начинается с ожидаемого магического числа 'YPBN'"
                    );
                    eprintln!("  Получено: {:?}", magic);
                    eprintln!("  Ожидалось: {:?}", expected_magic);
                    eprintln!("  Продолжаем парсинг, но возможны ошибки...");
                }
                let file = File::open(input_path)?;
                let mut reader = BufReader::new(file);
                BinaryParser::parse_records(&mut reader)
                    .map_err(|e| format!("Ошибка парсинга бинарного файла: {}", e).into())
            } else {
                Err("Файл слишком мал для бинарного формата".into())
            }
        }
    }
}

fn write_transactions(
    transactions: &[Transaction],
    format: &Format,
    output_path: Option<&PathBuf>,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if verbose && output_path.is_none() {
        eprintln!("Вывод будет отправлен в стандартный вывод (stdout)");
        eprintln!("Используйте --output <файл> для сохранения в файл");
    }

    if output_path.is_none() && matches!(format, Format::Bin) {
        return Err("Ошибка: Для бинарного формата необходимо указать выходной файл с помощью --output <файл>".into());
    }

    match output_path {
        Some(path) => {
            if path.exists() && verbose {
                eprintln!("Файл '{}' будет перезаписан", path.display());
            }

            let file = File::create(path)
                .map_err(|e| format!("Не удалось создать файл '{}': {}", path.display(), e))?;
            let mut writer = BufWriter::new(file);
            write_to_writer(transactions, format, &mut writer, verbose)
        }
        None => {
            let stdout = io::stdout();
            let mut writer = BufWriter::new(stdout.lock());
            write_to_writer(transactions, format, &mut writer, verbose)
        }
    }
}

fn write_to_writer<W: std::io::Write>(
    transactions: &[Transaction],
    format: &Format,
    writer: &mut W,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if verbose {
        eprintln!(
            "Запись {} транзакций в формат {:?}...",
            transactions.len(),
            format
        );
    }

    match format {
        Format::Csv => {
            if verbose {
                eprintln!("Формат: CSV (заголовок + данные)");
            }
            CsvParser::write_records(transactions, writer)
                .map_err(|e| format!("Ошибка записи CSV: {}", e).into())
        }
        Format::Txt => {
            if verbose {
                eprintln!("Формат: Text (KEY: VALUE с комментариями)");
            }
            TextParser::write_records(transactions, writer)
                .map_err(|e| format!("Ошибка записи текстового формата: {}", e).into())
        }
        Format::Bin => {
            if verbose {
                eprintln!("Формат: Binary (магическое число YPBN + бинарные данные)");
                eprintln!(
                    "Размер одной записи: ~{} байт + размер описания",
                    std::mem::size_of::<u64>() * 5 + 2
                ); // 5 u64 + 2 u8
            }
            BinaryParser::write_records(transactions, writer)
                .map_err(|e| format!("Ошибка записи бинарного формата: {}", e).into())
        }
    }
}

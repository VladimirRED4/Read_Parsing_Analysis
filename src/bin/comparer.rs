use clap::Parser;
use parser_lib::{BinaryParser, CsvParser, TextParser, Transaction};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "ypbank_compare")]
#[command(about = "Сравнивает транзакции из двух файлов в разных форматах", long_about = None)]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Args {
    #[arg(long = "file1", value_name = "FILE")]
    file1: PathBuf,

    #[arg(
        long = "format1",
        value_name = "FORMAT",
        value_enum,
        ignore_case = true
    )]
    format1: Format,

    #[arg(long = "file2", value_name = "FILE")]
    file2: PathBuf,

    #[arg(
        long = "format2",
        value_name = "FORMAT",
        value_enum,
        ignore_case = true
    )]
    format2: Format,

    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    #[arg(long = "ignore-description", default_value_t = false)]
    ignore_description: bool,

    #[arg(long = "ignore-status", default_value_t = false)]
    ignore_status: bool,
}

#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
enum Format {
    Csv,
    Txt,
    Bin,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.verbose {
        eprintln!("=== YPBank Comparer ===");
        eprintln!("Сравниваем файлы:");
        eprintln!(
            "  Файл 1: {} (формат: {:?})",
            args.file1.display(),
            args.format1
        );
        eprintln!(
            "  Файл 2: {} (формат: {:?})",
            args.file2.display(),
            args.format2
        );
        if args.ignore_description {
            eprintln!("  Игнорируем различия в описаниях");
        }
        if args.ignore_status {
            eprintln!("  Игнорируем различия в статусах");
        }
    }

    if !args.file1.exists() {
        eprintln!("Ошибка: файл '{}' не найден", args.file1.display());
        std::process::exit(1);
    }
    if !args.file2.exists() {
        eprintln!("Ошибка: файл '{}' не найден", args.file2.display());
        std::process::exit(1);
    }

    let transactions1 = read_transactions(&args.file1, &args.format1)?;
    let transactions2 = read_transactions(&args.file2, &args.format2)?;

    if args.verbose {
        eprintln!("Прочитано транзакций:");
        eprintln!("  Из файла 1: {}", transactions1.len());
        eprintln!("  Из файла 2: {}", transactions2.len());
    }

    compare_transactions(&transactions1, &transactions2, &args)?;

    Ok(())
}

fn read_transactions(
    file_path: &PathBuf,
    format: &Format,
) -> Result<Vec<Transaction>, Box<dyn std::error::Error>> {
    match format {
        Format::Csv => {
            let file = File::open(file_path).map_err(|e| {
                format!(
                    "Не удалось открыть CSV файл '{}': {}",
                    file_path.display(),
                    e
                )
            })?;
            CsvParser::parse_records(file).map_err(|e| {
                format!("Ошибка парсинга CSV файла '{}': {}", file_path.display(), e).into()
            })
        }
        Format::Txt => {
            let file = File::open(file_path).map_err(|e| {
                format!(
                    "Не удалось открыть текстовый файл '{}': {}",
                    file_path.display(),
                    e
                )
            })?;
            TextParser::parse_records(file).map_err(|e| {
                format!(
                    "Ошибка парсинга текстового файла '{}': {}",
                    file_path.display(),
                    e
                )
                .into()
            })
        }
        Format::Bin => {
            let file = File::open(file_path).map_err(|e| {
                format!(
                    "Не удалось открыть бинарный файл '{}': {}",
                    file_path.display(),
                    e
                )
            })?;
            let mut reader = BufReader::new(file);
            BinaryParser::parse_records(&mut reader).map_err(|e| {
                format!(
                    "Ошибка парсинга бинарного файла '{}': {}",
                    file_path.display(),
                    e
                )
                .into()
            })
        }
    }
}

fn compare_transactions(
    txs1: &[Transaction],
    txs2: &[Transaction],
    args: &Args,
) -> Result<(), Box<dyn std::error::Error>> {
    if txs1.len() != txs2.len() {
        println!("Файлы содержат разное количество транзакций:");
        println!("  В '{}': {} транзакций", args.file1.display(), txs1.len());
        println!("  В '{}': {} транзакций", args.file2.display(), txs2.len());
        return Ok(());
    }

    if txs1.is_empty() {
        println!("Оба файла пусты.");
        return Ok(());
    }

    let mut mismatches = Vec::new();
    let mut identical_count = 0;

    for (i, (tx1, tx2)) in txs1.iter().zip(txs2.iter()).enumerate() {
        if transactions_equal(tx1, tx2, args) {
            identical_count += 1;
        } else {
            mismatches.push((i, tx1, tx2));
        }
    }

    if mismatches.is_empty() {
        println!(
            "Транзакции в '{}' и '{}' идентичны.",
            args.file1.display(),
            args.file2.display()
        );
        if args.verbose {
            println!("Все {} транзакций совпадают.", identical_count);
        }
    } else {
        println!(
            "Найдено {} несоответствий из {} транзакций:",
            mismatches.len(),
            txs1.len()
        );

        for (i, tx1, tx2) in mismatches.iter().take(10) {
            println!(
                "\nНесоответствие в транзакции #{} (ID: {}):",
                i + 1,
                tx1.tx_id
            );
            print_differences(tx1, tx2, args);
        }

        if mismatches.len() > 10 {
            println!("\n... и еще {} несоответствий.", mismatches.len() - 10);
        }

        if args.verbose {
            println!("\nСтатистика:");
            println!("  Идентичных транзакций: {}", identical_count);
            println!("  Несовпадающих транзакций: {}", mismatches.len());
            println!("  Всего транзакций: {}", txs1.len());
        }
    }

    Ok(())
}

fn transactions_equal(tx1: &Transaction, tx2: &Transaction, args: &Args) -> bool {
    if tx1.tx_id != tx2.tx_id {
        return false;
    }
    if tx1.tx_type != tx2.tx_type {
        return false;
    }
    if tx1.from_user_id != tx2.from_user_id {
        return false;
    }
    if tx1.to_user_id != tx2.to_user_id {
        return false;
    }
    if tx1.amount != tx2.amount {
        return false;
    }
    if tx1.timestamp != tx2.timestamp {
        return false;
    }
    if !args.ignore_status && tx1.status != tx2.status {
        return false;
    }
    if !args.ignore_description && tx1.description != tx2.description {
        return false;
    }
    true
}

fn print_differences(tx1: &Transaction, tx2: &Transaction, args: &Args) {
    if tx1.tx_id != tx2.tx_id {
        println!("  TX_ID: {} != {}", tx1.tx_id, tx2.tx_id);
    }
    if tx1.tx_type != tx2.tx_type {
        println!("  TX_TYPE: {:?} != {:?}", tx1.tx_type, tx2.tx_type);
    }
    if tx1.from_user_id != tx2.from_user_id {
        println!(
            "  FROM_USER_ID: {} != {}",
            tx1.from_user_id, tx2.from_user_id
        );
    }
    if tx1.to_user_id != tx2.to_user_id {
        println!("  TO_USER_ID: {} != {}", tx1.to_user_id, tx2.to_user_id);
    }
    if tx1.amount != tx2.amount {
        println!("  AMOUNT: {} != {}", tx1.amount, tx2.amount);
    }
    if tx1.timestamp != tx2.timestamp {
        println!("  TIMESTAMP: {} != {}", tx1.timestamp, tx2.timestamp);
    }
    if !args.ignore_status && tx1.status != tx2.status {
        println!("  STATUS: {:?} != {:?}", tx1.status, tx2.status);
    }
    if !args.ignore_description && tx1.description != tx2.description {
        println!(
            "  DESCRIPTION: '{}' != '{}'",
            tx1.description, tx2.description
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parser_lib::{TransactionStatus, TransactionType};
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_transaction(id: u64) -> Transaction {
        Transaction {
            tx_id: id,
            tx_type: TransactionType::Deposit,
            from_user_id: 0,
            to_user_id: 501,
            amount: 50000,
            timestamp: 1672531200000,
            status: TransactionStatus::Success,
            description: format!("Test transaction {}", id),
        }
    }

    #[test]
    fn test_transactions_equal_basic() {
        let tx1 = create_test_transaction(1001);
        let tx2 = create_test_transaction(1001);

        let args = Args {
            file1: PathBuf::from("test1.csv"),
            format1: Format::Csv,
            file2: PathBuf::from("test2.csv"),
            format2: Format::Csv,
            verbose: false,
            ignore_description: false,
            ignore_status: false,
        };

        assert!(transactions_equal(&tx1, &tx2, &args));
    }

    #[test]
    fn test_transactions_equal_ignore_description() {
        let mut tx1 = create_test_transaction(1001);
        let mut tx2 = create_test_transaction(1001);
        tx1.description = "Description 1".to_string();
        tx2.description = "Description 2".to_string();

        let args = Args {
            file1: PathBuf::from("test1.csv"),
            format1: Format::Csv,
            file2: PathBuf::from("test2.csv"),
            format2: Format::Csv,
            verbose: false,
            ignore_description: true,
            ignore_status: false,
        };

        assert!(transactions_equal(&tx1, &tx2, &args));
    }

    #[test]
    fn test_transactions_equal_ignore_status() {
        let mut tx1 = create_test_transaction(1001);
        let mut tx2 = create_test_transaction(1001);
        tx1.status = TransactionStatus::Success;
        tx2.status = TransactionStatus::Failure;

        let args = Args {
            file1: PathBuf::from("test1.csv"),
            format1: Format::Csv,
            file2: PathBuf::from("test2.csv"),
            format2: Format::Csv,
            verbose: false,
            ignore_description: false,
            ignore_status: true,
        };

        assert!(transactions_equal(&tx1, &tx2, &args));
    }

    #[test]
    fn test_transactions_not_equal() {
        let tx1 = create_test_transaction(1001);
        let tx2 = create_test_transaction(1002); // Разный ID

        let args = Args {
            file1: PathBuf::from("test1.csv"),
            format1: Format::Csv,
            file2: PathBuf::from("test2.csv"),
            format2: Format::Csv,
            verbose: false,
            ignore_description: false,
            ignore_status: false,
        };

        assert!(!transactions_equal(&tx1, &tx2, &args));
    }

    #[test]
    fn test_create_csv_file() -> Result<(), Box<dyn std::error::Error>> {
        let mut file = NamedTempFile::new()?;
        writeln!(
            file,
            "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION"
        )?;
        writeln!(
            file,
            "1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,\"Test\""
        )?;

        let transactions = read_transactions(&file.path().to_path_buf(), &Format::Csv)?;
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].tx_id, 1001);

        Ok(())
    }

    #[test]
    fn test_create_text_file() -> Result<(), Box<dyn std::error::Error>> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "TX_ID: 1001")?;
        writeln!(file, "TX_TYPE: DEPOSIT")?;
        writeln!(file, "FROM_USER_ID: 0")?;
        writeln!(file, "TO_USER_ID: 501")?;
        writeln!(file, "AMOUNT: 50000")?;
        writeln!(file, "TIMESTAMP: 1672531200000")?;
        writeln!(file, "STATUS: SUCCESS")?;
        writeln!(file, "DESCRIPTION: \"Test\"")?;

        let transactions = read_transactions(&file.path().to_path_buf(), &Format::Txt)?;
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].tx_id, 1001);

        Ok(())
    }

    #[test]
    fn test_print_differences() {
        let tx1 = create_test_transaction(1001);
        let mut tx2 = create_test_transaction(1001);
        tx2.amount = 60000;
        tx2.description = "Different".to_string();

        let args = Args {
            file1: PathBuf::from("test1.csv"),
            format1: Format::Csv,
            file2: PathBuf::from("test2.csv"),
            format2: Format::Csv,
            verbose: false,
            ignore_description: false,
            ignore_status: false,
        };

        print_differences(&tx1, &tx2, &args);
    }

    #[test]
    fn test_compare_empty_lists() {
        let args = Args {
            file1: PathBuf::from("test1.csv"),
            format1: Format::Csv,
            file2: PathBuf::from("test2.csv"),
            format2: Format::Csv,
            verbose: false,
            ignore_description: false,
            ignore_status: false,
        };

        let empty: Vec<Transaction> = Vec::new();
        let result = compare_transactions(&empty, &empty, &args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compare_different_lengths() {
        let args = Args {
            file1: PathBuf::from("test1.csv"),
            format1: Format::Csv,
            file2: PathBuf::from("test2.csv"),
            format2: Format::Csv,
            verbose: false,
            ignore_description: false,
            ignore_status: false,
        };

        let tx1 = create_test_transaction(1001);
        let tx2 = create_test_transaction(1002);

        let list1 = vec![tx1.clone(), tx2.clone()];
        let list2 = vec![tx1];

        let result = compare_transactions(&list1, &list2, &args);
        assert!(result.is_ok());
    }
}

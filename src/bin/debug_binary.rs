use parser_lib::BinaryParser;
use std::fs::File;
use std::io::BufReader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Диагностика бинарного файла ===");

    let file = File::open("examples/records_example.bin")?;
    let mut reader = BufReader::new(file);
    let transactions = BinaryParser::parse_records(&mut reader)?;

    println!("Прочитано {} транзакций:", transactions.len());
    for (i, tx) in transactions.iter().enumerate() {
        println!("\nТранзакция {}:", i + 1);
        println!("  ID: {}", tx.tx_id);
        println!("  Описание: '{}'", tx.description);
        println!("  Длина: {}", tx.description.len());
        println!("  Первый символ: '{}'", tx.description.chars().next().unwrap_or(' '));
        println!("  Последний символ: '{}'", tx.description.chars().last().unwrap_or(' '));
        println!("  Байты: {:?}", tx.description.as_bytes());
    }

    Ok(())
}
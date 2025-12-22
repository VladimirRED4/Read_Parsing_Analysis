use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn build_and_get_binary() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Сначала собираем бинарник
    let build_status = Command::new("cargo")
        .args(["build", "--bin", "ypbank_converter", "--quiet"])
        .status()
        .expect("Failed to build binary");

    assert!(build_status.success(), "Failed to build ypbank_converter");

    // Путь к бинарнику
    let mut binary_path = manifest_dir
        .join("target")
        .join("debug")
        .join("ypbank_converter");

    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    assert!(
        binary_path.exists(),
        "Binary not found at {:?}",
        binary_path
    );
    binary_path
}

#[test]
fn test_cli_help() {
    let binary_path = build_and_get_binary();

    let output = Command::new(&binary_path)
        .arg("--help")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ypbank_converter"));
    assert!(stdout.contains("--input"));
    assert!(stdout.contains("--input-format"));
}

#[test]
fn test_cli_version() {
    let binary_path = build_and_get_binary();

    let output = Command::new(&binary_path)
        .arg("--version")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ypbank_converter"));
}

#[test]
fn test_csv_to_txt() {
    let binary_path = build_and_get_binary();
    let temp_dir = TempDir::new().unwrap();

    // Создаем тестовый CSV файл
    let csv_path = temp_dir.path().join("test.csv");
    let mut csv_file = File::create(&csv_path).unwrap();
    writeln!(
        csv_file,
        "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION"
    )
    .unwrap();
    writeln!(
        csv_file,
        "1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,\"Test deposit\""
    )
    .unwrap();

    let output_path = temp_dir.path().join("output.txt");

    let output = Command::new(&binary_path)
        .args([
            "--input",
            csv_path.to_str().unwrap(),
            "--input-format",
            "csv",
            "--output-format",
            "txt",
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed:\nStdout: {}\nStderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(output_path.exists());
    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("TX_ID: 1001"));
    assert!(content.contains("DEPOSIT"));
}

#[test]
fn test_missing_file_error() {
    let binary_path = build_and_get_binary();

    let output = Command::new(&binary_path)
        .args([
            "--input",
            "non_existent_file.csv",
            "--input-format",
            "csv",
            "--output-format",
            "txt",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success(), "Command should have failed");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("не найден") || stderr.contains("not found"));
}

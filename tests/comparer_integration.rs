use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn build_and_get_binary(binary_name: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let build_status = Command::new("cargo")
        .args(["build", "--bin", binary_name, "--quiet"])
        .status()
        .expect("Failed to build binary");

    assert!(build_status.success(), "Failed to build {}", binary_name);

    let mut binary_path = manifest_dir.join("target").join("debug").join(binary_name);

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
fn test_comparer_help() {
    let binary_path = build_and_get_binary("comparer");

    let output = Command::new(&binary_path)
        .arg("--help")
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed:\nStdout: {}\nStderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("Сравнивает транзакции"),
        "Missing description"
    );
    assert!(stdout.contains("--file1"), "Missing --file1");
    assert!(stdout.contains("--format1"), "Missing --format1");
    assert!(stdout.contains("--file2"), "Missing --file2");
    assert!(stdout.contains("--format2"), "Missing --format2");
    assert!(stdout.contains("--verbose"), "Missing --verbose");
    assert!(
        stdout.contains("--ignore-description"),
        "Missing --ignore-description"
    );
    assert!(
        stdout.contains("--ignore-status"),
        "Missing --ignore-status"
    );
}

#[test]
fn test_comparer_version() {
    let binary_path = build_and_get_binary("comparer");

    let output = Command::new(&binary_path)
        .arg("--version")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("0.1"),
        "Version 0.1 not found. Output: {}",
        stdout
    );
}

#[test]
fn test_comparer_identical_files() {
    let binary_path = build_and_get_binary("comparer");
    let temp_dir = TempDir::new().unwrap();

    let csv1_path = temp_dir.path().join("file1.csv");
    let csv2_path = temp_dir.path().join("file2.csv");

    let csv_content = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n\
                       1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,\"Test\"";

    fs::write(&csv1_path, csv_content).unwrap();
    fs::write(&csv2_path, csv_content).unwrap();

    let output = Command::new(&binary_path)
        .args([
            "--file1",
            csv1_path.to_str().unwrap(),
            "--format1",
            "csv",
            "--file2",
            csv2_path.to_str().unwrap(),
            "--format2",
            "csv",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("идентичны"));
}

#[test]
fn test_comparer_different_files() {
    let binary_path = build_and_get_binary("comparer");
    let temp_dir = TempDir::new().unwrap();

    let csv1_path = temp_dir.path().join("file1.csv");
    let csv2_path = temp_dir.path().join("file2.csv");

    let csv1_content = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n\
                        1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,\"Test 1\"";

    let csv2_content = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n\
                        1001,DEPOSIT,0,501,60000,1672531200000,SUCCESS,\"Test 2\"";

    fs::write(&csv1_path, csv1_content).unwrap();
    fs::write(&csv2_path, csv2_content).unwrap();

    let output = Command::new(&binary_path)
        .args([
            "--file1",
            csv1_path.to_str().unwrap(),
            "--format1",
            "csv",
            "--file2",
            csv2_path.to_str().unwrap(),
            "--format2",
            "csv",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("несоответствий") || stdout.contains("AMOUNT:"));
}

#[test]
fn test_comparer_different_formats() {
    let binary_path = build_and_get_binary("comparer");
    let temp_dir = TempDir::new().unwrap();

    let csv_path = temp_dir.path().join("file.csv");
    let csv_content = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n\
                       1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,\"Test\"";
    fs::write(&csv_path, csv_content).unwrap();

    let txt_path = temp_dir.path().join("file.txt");
    let txt_content = r#"TX_ID: 1001
TX_TYPE: DEPOSIT
FROM_USER_ID: 0
TO_USER_ID: 501
AMOUNT: 50000
TIMESTAMP: 1672531200000
STATUS: SUCCESS
DESCRIPTION: "Test""#;
    fs::write(&txt_path, txt_content).unwrap();

    let output = Command::new(&binary_path)
        .args([
            "--file1",
            csv_path.to_str().unwrap(),
            "--format1",
            "csv",
            "--file2",
            txt_path.to_str().unwrap(),
            "--format2",
            "txt",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("идентичны"));
}

#[test]
fn test_comparer_missing_file() {
    let binary_path = build_and_get_binary("comparer");

    let output = Command::new(&binary_path)
        .args([
            "--file1",
            "non_existent1.csv",
            "--format1",
            "csv",
            "--file2",
            "non_existent2.csv",
            "--format2",
            "csv",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("не найден"));
}

#[test]
fn test_comparer_ignore_description() {
    let binary_path = build_and_get_binary("comparer");
    let temp_dir = TempDir::new().unwrap();

    let csv1_path = temp_dir.path().join("file1.csv");
    let csv2_path = temp_dir.path().join("file2.csv");

    let csv1_content = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n\
                        1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,\"Description 1\"";

    let csv2_content = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n\
                        1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,\"Description 2\"";

    fs::write(&csv1_path, csv1_content).unwrap();
    fs::write(&csv2_path, csv2_content).unwrap();

    let output = Command::new(&binary_path)
        .args([
            "--file1",
            csv1_path.to_str().unwrap(),
            "--format1",
            "csv",
            "--file2",
            csv2_path.to_str().unwrap(),
            "--format2",
            "csv",
            "--ignore-description",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("идентичны"));
}

#[test]
fn test_comparer_different_lengths() {
    let binary_path = build_and_get_binary("comparer");
    let temp_dir = TempDir::new().unwrap();

    let csv1_path = temp_dir.path().join("file1.csv");
    let csv2_path = temp_dir.path().join("file2.csv");

    let csv1_content = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n\
                        1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,\"Test 1\"\n\
                        1002,TRANSFER,501,502,15000,1672534800000,FAILURE,\"Test 2\"";

    let csv2_content = "TX_ID,TX_TYPE,FROM_USER_ID,TO_USER_ID,AMOUNT,TIMESTAMP,STATUS,DESCRIPTION\n\
                        1001,DEPOSIT,0,501,50000,1672531200000,SUCCESS,\"Test 1\"";

    fs::write(&csv1_path, csv1_content).unwrap();
    fs::write(&csv2_path, csv2_content).unwrap();

    let output = Command::new(&binary_path)
        .args([
            "--file1",
            csv1_path.to_str().unwrap(),
            "--format1",
            "csv",
            "--file2",
            csv2_path.to_str().unwrap(),
            "--format2",
            "csv",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("разное количество") || stdout.contains("2 транзакций"));
}

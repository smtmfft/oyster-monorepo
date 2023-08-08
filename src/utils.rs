use std::fs::OpenOptions;
use std::io::Write;

pub fn log_data(log_data: String) {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open("requests.log")
        .unwrap();

    file.write_all(log_data.as_bytes()).unwrap();
}

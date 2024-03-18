use std::fs::OpenOptions;
use std::io::Write;

pub const EC2_SERVICE_CODE: &str = "ec2";
pub const VCPU_QUOTA_NAME: &str = "vcpu";
pub const ELASTIC_IP_QUOTA_NAME: &str = "elastic_ip";

pub fn map_quota_to_code(quota_name: &str) -> Option<String> {
    match quota_name {
        VCPU_QUOTA_NAME => Some(String::from("L-1216C47A")),
        ELASTIC_IP_QUOTA_NAME => Some(String::from("L-0263D0A3")),
        _ => None,
    }
}

pub fn log_data(log_data: String) {
    match OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open("requests.log")
    {
        Ok(mut file) => {
            if let Err(err) = file.write_all(log_data.as_bytes()) {
                eprintln!("Error writing to the log file requests.log: {}", err);
            }
        }
        Err(err) => eprintln!("Error creating/opening log file requests.log: {}", err),
    }
}

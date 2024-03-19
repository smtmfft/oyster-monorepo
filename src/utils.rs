pub const EC2_SERVICE_CODE: &str = "ec2";
pub const VCPU_QUOTA_NAME: &str = "vcpu";
pub const ELASTIC_IP_QUOTA_NAME: &str = "eip";

pub fn map_quota_to_code(quota_name: &str) -> Option<String> {
    match quota_name {
        VCPU_QUOTA_NAME => Some(String::from("L-1216C47A")),
        ELASTIC_IP_QUOTA_NAME => Some(String::from("L-0263D0A3")),
        _ => None,
    }
}

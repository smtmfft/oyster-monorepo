pub const EC2_SERVICE_CODE: &str = "ec2";
pub const VCPU_QUOTA_NAME: &str = "vcpu";
pub const ELASTIC_IP_QUOTA_NAME: &str = "eip";

#[derive(Clone, Debug)]
pub enum Quota {
    Vcpu,
    Eip,
}

impl Quota {
    pub fn from_name(name: &str) -> anyhow::Result<Quota> {
        match name {
            "vcpus" => Ok(Quota::Vcpu),
            "eips" => Ok(Quota::Eip),
            _ => Err(anyhow::anyhow!("invalid quota, should be vcpus or eips")),
        }
    }

    pub fn to_code(&self) -> String {
        match self {
            Vcpu => "L-1216C47A".to_owned(),
            Eip => "L-0263D0A3".to_owned(),
        }
    }
}

pub fn map_quota_to_code(quota_name: &str) -> Option<String> {
    match quota_name {
        VCPU_QUOTA_NAME => Some(String::from("L-1216C47A")),
        ELASTIC_IP_QUOTA_NAME => Some(String::from("L-0263D0A3")),
        _ => None,
    }
}

use std::fmt::Display;

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
        use Quota::*;
        match self {
            Vcpu => "L-1216C47A".to_owned(),
            Eip => "L-0263D0A3".to_owned(),
        }
    }
}

impl Display for Quota {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Quota::*;
        match self {
            Vcpu => write!(f, "vcpus"),
            Eip => write!(f, "eips"),
        }
    }
}

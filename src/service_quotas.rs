use anyhow::{Context, Result};
use aws_config;
use aws_sdk_servicequotas;

pub const VCPU_QUOTA_CODE: &str = "L-1216C47A";
pub const ELASTIC_IP_QUOTA_CODE: &str = "L-0263D0A3";
pub const EC2_SERVICE_CODE: &str = "ec2";

pub async fn get_service_quota_limit(service: String, quota_code: String) -> Result<f64> {
    let config = aws_config::load_from_env().await;
    let client = aws_sdk_servicequotas::Client::new(&config);
    let get_service_quota = client
        .get_service_quota()
        .quota_code(quota_code)
        .service_code(service);

    let res = get_service_quota
        .send()
        .await
        .context("Error getting service quota")?;

    let quota = res.quota().unwrap().value().unwrap();

    Ok(quota)
}

pub async fn request_service_quota_increase(
    service: String,
    quota_code: String,
    desired_value: f64,
) -> Result<String> {
    let config = aws_config::load_from_env().await;
    let client = aws_sdk_servicequotas::Client::new(&config);
    let request_service_quota_increase = client
        .request_service_quota_increase()
        .quota_code(quota_code)
        .service_code(service)
        .desired_value(desired_value);

    let res = request_service_quota_increase
        .send()
        .await
        .context("Error requesting service quota increase")?;

    let request_service_quota_change_id: String =
        res.requested_quota().unwrap().id().unwrap().to_string();

    Ok(request_service_quota_change_id)
}

pub async fn get_requested_service_quota_status(request_id: String) -> Result<String> {
    let config = aws_config::load_from_env().await;
    let client = aws_sdk_servicequotas::Client::new(&config);

    let get_requested_service_quota_change = client
        .get_requested_service_quota_change()
        .request_id(request_id);

    let res = get_requested_service_quota_change
        .send()
        .await
        .context("Error getting service quota increase request status")?;

    let status = res
        .requested_quota()
        .unwrap()
        .status()
        .unwrap()
        .to_owned()
        .as_str()
        .to_string();

    Ok(status)
}

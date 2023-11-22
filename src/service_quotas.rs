use crate::utils::log_data;

use anyhow::{Result, Context, anyhow};
use aws_config;
use aws_sdk_servicequotas;
use chrono::{DateTime, Local, TimeZone, LocalResult::Single};

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

pub async fn get_latest_request_id(service: String, quota_code: String) -> Option<String> {
    let config = aws_config::load_from_env().await;
    let client = aws_sdk_servicequotas::Client::new(&config);

    let get_requested_service_quota_change_list = client
        .list_requested_service_quota_change_history_by_quota()
        .quota_code(&quota_code)
        .service_code(&service);
        
    match get_requested_service_quota_change_list.send().await {
        Ok(res) => {
            let requested_quota_change_history = res.requested_quotas.unwrap_or(Vec::new());

            if requested_quota_change_history.is_empty() {
                None
            }else {
                let latest_request_id = requested_quota_change_history
                    .first()
                    .unwrap()
                    .id()
                    .unwrap()
                    .to_string();
                
                Some(latest_request_id)
            }
        }

        Err(e) => {
            log_data(format!("\n[SCHEDULER] Error fetching list of service quota change requests for service {} and code {}: {:?}", service, quota_code, e));
            None
        }
    }
}
use crate::current_usage;
use crate::service_quotas;
use crate::utils;

use anyhow::{Result, Context};
use chrono::Local;

pub async fn get_id(quota_name: &str) -> Option<String> {
    let quota_code = utils::map_quota_to_code(quota_name);
    if quota_code.is_none() {
        utils::log_data(format!("Invalid quota name during monitoring: {}", quota_name));
        return None;
    }

    match service_quotas::get_latest_request_id(
        utils::EC2_SERVICE_CODE.to_string(), 
        quota_code.unwrap()
        )
        .await {
        Ok(request_id) => request_id,
        Err(err) => {            // Can retry here
            utils::log_data(format!("Failed to get latest {} request ID during monitoring: {:?}\n\n", quota_name, err));        
            None
        }
    }
}

pub async fn request_monitor(
    request: Option<String>, 
    quota_name: &str,
    no_update_threshold: i64
) -> Option<String> {
    match request {
        Some(request_id) => {
            match request_check(request_id.as_str(), no_update_threshold).await {
                Ok(request_option) => request_option,
                Err(err) => {
                    utils::log_data(format!("Error occurred while monitoring {} request ID {}: {:?}\n\n", quota_name, request_id, err));
                    Some(request_id)
                }
            }     
        }
        None => None,
    }
}

pub async fn usage_monitor(
    request: Option<String>,  
    quota_name: &str,
    threshold_percent: f64, 
    quota_increment_percent: f64
) -> Option<String> {
    match usage_check(
        request.clone(), 
        quota_name, 
        threshold_percent, 
        quota_increment_percent
        )
        .await {
        Ok(request_option) => request_option,
        Err(err) => {
            utils::log_data(format!("Error occurred while monitoring {} usage against quota: {:?}\n\n", quota_name, err));
            request
        }    
    }
}

async fn request_check(request_id: &str, no_update_threshold: i64) -> Result<Option<String>> {
    let status = service_quotas::get_requested_service_quota_status(request_id.to_string())
        .await
        .context("Error while retrieving status")?;

    match status.as_str() {
        "APPROVED" => Ok(None), 
        "PENDING" | "CASE_OPENED" => {
            let last_updated_time = service_quotas::get_requested_service_quota_last_updated(request_id.to_string())
                .await
                .context("Error while retrieving last updated time")?;

            if Local::now().signed_duration_since(last_updated_time).num_days() > no_update_threshold {
                utils::log_data(format!("Quota change request with ID {} has been found to be pending for a long time during monitoring, Please look into it!\n\n", request_id));
            } 
            Ok(Some(request_id.to_string()))
        }
        _ => {
            utils::log_data(format!("Quota change request with ID {} has been closed with the status {}, Please contact AWS support center for further info!\n\n", request_id, status)); 
            Ok(None)
        }
    }
}

async fn usage_check(
    request: Option<String>, 
    quota_name: &str, 
    threshold_percent: f64, 
    increment_percent: f64
) -> Result<Option<String>> {
    let current_usage = current_usage::get_current_usage(quota_name)
        .await
        .context("Error fetching current usage")? as f64;

    let service_quota = service_quotas::get_service_quota_limit(
        utils::EC2_SERVICE_CODE.to_string(), 
        utils::map_quota_to_code(quota_name).unwrap()
        )
        .await
        .context("Failed to get service quota limit/value")?; 

    if current_usage*100.0/service_quota > threshold_percent {
        let new_quota = service_quota*(1.0 + increment_percent/100.0);
        let request_id = service_quotas::request_service_quota_increase(
            utils::EC2_SERVICE_CODE.to_string(), 
            utils::map_quota_to_code(quota_name).unwrap(), 
            new_quota
            )
            .await
            .context("Failed to request service quota increase")
            .map_err(|err| {
                if request.is_some() {
                    err.context(format!("Another request already open with ID: {}", request.unwrap()))
                }else {
                    err
                }
            })?;

        utils::log_data(format!("Service quota increase requested while monitoring for {} with ID: {}\nDesired quota: {}\nTime: {}\n\n", 
            quota_name, 
            request_id, 
            new_quota, 
            Local::now().format("%Y-%m-%d %H:%M:%S")));
        Ok(Some(request_id))
    }else {
        Ok(request)
    }
}
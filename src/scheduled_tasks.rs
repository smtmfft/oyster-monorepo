use crate::current_usage;
use crate::service_quotas;
use crate::utils::log_data;

use chrono::Local;

pub async fn request_monitor(vcpu_request_id: &mut Option<String>,
    elastic_ip_request_id: &mut Option<String>, 
    no_update_threshold: i64) {
    if vcpu_request_id.is_some() {
        request_check(vcpu_request_id, no_update_threshold)
            .await;
    }

    if elastic_ip_request_id.is_some() {
        request_check(elastic_ip_request_id, no_update_threshold)
            .await;
    }
}

pub async fn usage_monitor(vcpu_request_id: &mut Option<String>, 
    elastic_ip_request_id: &mut Option<String>, 
    vcpu_threshold_percent: f64, 
    vcpu_quota_increment_percent: f64, 
    elastic_ip_threshold_percent: f64, 
    elastic_ip_quota_increment_percent: f64) {
    match current_usage::get_no_of_vcpus().await {
        Ok(no_of_vcpus) => {
            usage_check(no_of_vcpus as f64, 
                service_quotas::EC2_SERVICE_CODE.to_string(), 
                service_quotas::VCPU_QUOTA_CODE.to_string(), 
                vcpu_threshold_percent, 
                vcpu_quota_increment_percent, 
                vcpu_request_id)
                .await;
        }

        Err(e) => {
            log_data(format!("\n[SCHEDULER] Error fetching no. of vCPUs: {:?}", e));
        }
    }

    match current_usage::get_no_elatic_ips().await {
        Ok(no_of_elastic_ips) => {
            usage_check(no_of_elastic_ips as f64, 
                service_quotas::EC2_SERVICE_CODE.to_string(), 
                service_quotas::ELASTIC_IP_QUOTA_CODE.to_string(), 
                elastic_ip_threshold_percent,
                elastic_ip_quota_increment_percent,
                elastic_ip_request_id)
                .await;
        }

        Err(e) => {
            log_data(format!("\n[SCHEDULER] Error fetching no. of Elastic IPs: {:?}", e));
        }
    }
}

async fn request_check(request_id: &mut Option<String>, no_update_threshold: i64) {
    match service_quotas::get_requested_service_quota_status(request_id.clone().unwrap()).await {
        Ok(status) => {
            match status.as_str() {
                "APPROVED" => {
                    *request_id = None;  
                }

                "PENDING" | "CASE_OPENED" => {
                    match service_quotas::get_requested_service_quota_last_updated(request_id.clone().unwrap()).await {
                        Ok(time) => {
                            if Local::now().signed_duration_since(time).num_days() > no_update_threshold {
                                log_data(format!("\n[SCHEDULER] Quota change request with ID {} has been pending for a long time, please look into it!", request_id.clone().unwrap()));
                            }
                        }

                        Err(e) => {
                            log_data(format!("\n[SCHEDULER] Error getting last updated time for pending request ID {}: {:?}", request_id.clone().unwrap(), e));
                        }
                    }
                }

                _ => {
                    log_data(format!("\n[SCHEDULER] Quota change request with ID {} was closed with the status {}, Please contact AWS Support center for further info!", request_id.clone().unwrap(), status)); 
                    *request_id = None;
                }
            }
        } 

        Err(e) => {
            log_data(format!("\n[SCHEDULER] Error fetching status of request ID {}: {:?}", request_id.clone().unwrap(), e));  
        }
    }
}

async fn usage_check(current_usage: f64, 
    service_code: String, 
    quota_code: String, 
    threshold_percent: f64, 
    increment_percent: f64, 
    request: &mut Option<String>) {
    match service_quotas::get_service_quota_limit(service_code.clone(), quota_code.clone()).await {
        Ok(service_quota) => {
            if current_usage*100.0/service_quota > threshold_percent {
                let new_quota = service_quota*(1.0 + increment_percent/100.0);
                match service_quotas::request_service_quota_increase(service_code.clone(), quota_code.clone(), new_quota).await {
                    Ok(request_id) => {
                        log_data(format!("\n[SCHEDULER] Service quota increase requested with ID: {}\nService: {}\nCode: {}\nDesired quota: {}\nTime: {}", request_id, service_code, quota_code, new_quota, Local::now().format("%Y-%m-%d %H:%M:%S")));
                        *request = Some(request_id);
                    }

                    Err(e) => {
                        log_data(format!("\n[SCHEDULER] Failed to request quota increase for service {} and code {}: {:?}", service_code, quota_code, e));
                        if request.is_some() {
                            log_data(format!("\n[SCHEDULER] Another request already in open state with ID: {}", request.clone().unwrap()));
                        } 
                    }
                }
            } 
        }

        Err(e) => {
            log_data(format!("\n[SCHEDULER] Error fetching quota value for service {} and code {}: {:?}", service_code, quota_code, e));
        }
    }
}
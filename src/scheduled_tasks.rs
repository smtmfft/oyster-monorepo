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
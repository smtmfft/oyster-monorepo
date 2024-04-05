use crate::utils;

use anyhow::{anyhow, Context, Result};

pub async fn get_service_quota_limit(
    client: &aws_sdk_servicequotas::Client,
    quota: &utils::Quota,
) -> Result<usize> {
    Ok(client
        .get_service_quota()
        .quota_code(quota.to_code())
        .service_code("ec2")
        .send()
        .await
        .context("Error getting service quota from AWS client")?
        .quota()
        .ok_or(anyhow!("Could not parse service quota from AWS response"))?
        .value()
        .ok_or(anyhow!("Could not parse service quota value"))? as usize)
}

pub async fn last_request(
    client: &aws_sdk_servicequotas::Client,
    quota: &utils::Quota,
) -> Result<Option<aws_sdk_servicequotas::types::RequestedServiceQuotaChange>> {
    Ok(client
        .list_requested_service_quota_change_history_by_quota()
        .quota_code(quota.to_code())
        .service_code("ec2")
        .send()
        .await
        .context("Error getting service quota from AWS client")?
        .requested_quotas
        .ok_or(anyhow!(
            "Could not parse requested quotas from AWS response"
        ))?
        .into_iter()
        .min_by_key(|x| x.created))
}

// pub async fn request_service_quota_increase(
//     config: &SdkConfig,
//     service: String,
//     quota_code: String,
//     desired_value: f64,
// ) -> Result<String> {
//     let client = aws_sdk_servicequotas::Client::new(config);
//
//     Ok(client
//         .request_service_quota_increase()
//         .quota_code(quota_code)
//         .service_code(service)
//         .desired_value(desired_value)
//         .send()
//         .await
//         .context("Error occurred while requesting service quota increase from AWS client")?
//         .requested_quota()
//         .ok_or(anyhow!(
//             "Could not parse requested service quota from AWS response"
//         ))?
//         .id()
//         .ok_or(anyhow!("Could not parse quota request ID"))?
//         .to_string())
// }

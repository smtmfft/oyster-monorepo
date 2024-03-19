use anyhow::{anyhow, Context, Result};
use aws_config::SdkConfig;
use aws_sdk_servicequotas;
use chrono::{DateTime, Local, TimeZone};

pub async fn get_service_quota_limit(
    client: &aws_sdk_servicequotas::Client,
    service: String,
    quota_code: String,
) -> Result<usize> {
    Ok(client
        .get_service_quota()
        .quota_code(quota_code)
        .service_code(service)
        .send()
        .await
        .context("Error getting service quota from AWS client")?
        .quota()
        .ok_or(anyhow!("Could not parse service quota from AWS response"))?
        .value()
        .ok_or(anyhow!("Could not parse service quota value"))? as usize)
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
//
// pub async fn get_requested_service_quota_status(
//     config: &SdkConfig,
//     request_id: String,
// ) -> Result<String> {
//     let client = aws_sdk_servicequotas::Client::new(config);
//
//     Ok(client
//         .get_requested_service_quota_change()
//         .request_id(request_id)
//         .send()
//         .await
//         .context("Error getting service quota change request from AWS client")?
//         .requested_quota()
//         .ok_or(anyhow!(
//             "Could not parse requested service quota from AWS response"
//         ))?
//         .status()
//         .ok_or(anyhow!("Could not parse quota request status"))?
//         .to_owned()
//         .as_str()
//         .to_string())
// }
//
// pub async fn get_requested_service_quota_last_updated(
//     config: &SdkConfig,
//     request_id: String,
// ) -> Result<DateTime<Local>> {
//     let client = aws_sdk_servicequotas::Client::new(config);
//
//     Ok(Local
//         .timestamp_millis_opt(
//             client
//                 .get_requested_service_quota_change()
//                 .request_id(request_id)
//                 .send()
//                 .await
//                 .context("Error getting service quota change request from AWS client")?
//                 .requested_quota()
//                 .ok_or(anyhow!(
//                     "Could not parse requested service quota from AWS response"
//                 ))?
//                 .last_updated()
//                 .ok_or(anyhow!("Could not parse quota request last updated time"))?
//                 .to_owned()
//                 .to_millis()
//                 .context("Error during translation of aws_sdk_ec2 primitive DateTime to millis")?,
//         )
//         .earliest()
//         .ok_or(anyhow!(
//             "Error during conversion of aws_sdk_ec2 primitive DateTime to chrono DateTime"
//         ))?)
// }
//
// pub async fn get_latest_request_id(
//     config: &SdkConfig,
//     service: String,
//     quota_code: String,
// ) -> Result<Option<String>> {
//     let client = aws_sdk_servicequotas::Client::new(config);
//
//     Ok(client
//         .list_requested_service_quota_change_history_by_quota()
//         .quota_code(quota_code)
//         .service_code(service)
//         .send()
//         .await
//         .context("Error fetching the list of service quota change requests from AWS client")?
//         .requested_quotas
//         .ok_or(anyhow!(
//             "Could not parse requested service quotas from AWS response"
//         ))?
//         .first()
//         .map(|x| x.id().ok_or(anyhow!("Could not parse quota request ID")))
//         .transpose()?
//         .map(|id| id.to_string()))
// }

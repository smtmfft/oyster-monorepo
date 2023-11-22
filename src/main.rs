mod current_usage;
mod service_quotas;
mod utils;
mod scheduled_tasks;

use std::collections::HashMap;

use anyhow::Context;
use chrono;
use clap::Parser;
use tokio::time::{Duration, interval};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(long, value_parser)]
    limit_status: bool,

    #[clap(long, value_parser)]
    limit_increase: bool,

    #[clap(long, value_parser)]
    request_status: bool,

    #[clap(long, value_parser)]
    quota_name: Option<String>,

    #[clap(long, value_parser)]
    quota_value: Option<f64>,

    #[clap(long, value_parser)]
    request_id: Option<String>,

    #[clap(long, value_parser, default_value = "900")]
    monitor_interval_secs: u64,

    #[clap(long, value_parser, default_value = "5")]
    no_update_days_threshold: i64,

    #[clap(long, value_parser, default_value = "75.0")]
    vcpu_usage_threshold_percent: f64,

    #[clap(long, value_parser, default_value = "75.0")]
    elastic_ip_usage_threshold_percent: f64,

    #[clap(long, value_parser, default_value = "50.0")]
    vcpu_qouta_increment_percent: f64,

    #[clap(long, value_parser, default_value = "50.0")]
    elastic_ip_quota_increment_percent: f64,
}

async fn limit_status() {
    let no_vcpus = current_usage::get_no_of_vcpus()
        .await
        .context("Failed to get no of vcpus")
        .unwrap();
    let no_elastic_ips = current_usage::get_no_elatic_ips()
        .await
        .context("Failed to get no of elastic ips")
        .unwrap();

    let vcpu_limit = service_quotas::get_service_quota_limit(
        service_quotas::EC2_SERVICE_CODE.to_string(),
        service_quotas::VCPU_QUOTA_CODE.to_string(),
    )
    .await
    .context("Failed to get vcpus quota")
    .unwrap();
    let elastic_ip_limit = service_quotas::get_service_quota_limit(
        service_quotas::EC2_SERVICE_CODE.to_string(),
        service_quotas::ELASTIC_IP_QUOTA_CODE.to_string(),
    )
    .await
    .context("Failed to get elastic ips quota")
    .unwrap();
    println!(
        "VCPU: {}/{},\nElastic IP: {}/{}",
        no_vcpus, vcpu_limit, no_elastic_ips, elastic_ip_limit
    );
}

async fn limit_increase(quota_name: String, quota_value: f64) {
    let possible_quota_names: [&str; 2] = ["vcpu", "elastic_ip"];
    if possible_quota_names.contains(&quota_name.as_str()) {
        if quota_value == 0.0 {
            println!("Quota value must be greater than 0.");
            return;
        }
        let mut quota_code_hash = HashMap::new();
        quota_code_hash.insert("vcpu", service_quotas::VCPU_QUOTA_CODE.to_string());
        quota_code_hash.insert(
            "elastic_ip",
            service_quotas::ELASTIC_IP_QUOTA_CODE.to_string(),
        );

        let request_id = service_quotas::request_service_quota_increase(
            service_quotas::EC2_SERVICE_CODE.to_string(),
            quota_code_hash
                .get(quota_name.as_str())
                .unwrap()
                .to_string(),
            quota_value,
        )
        .await
        .context("Failed to request quota increase")
        .unwrap();

        let log_data = format!(
            "Request ID: {}\nQuota Name: {}\nQuota Value: {}\nTime: {}\n\n",
            request_id,
            quota_name,
            quota_value,
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        utils::log_data(log_data);

        println!("Service quota increase requested.");
        println!("Request ID: {}", request_id);
    } else {
        println!("Quota name must be one of these:\n1. vcpu\n2. elastic_ip");
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.limit_status {
        limit_status().await;
    } else if cli.limit_increase {
        let quota_name = cli.quota_name.unwrap_or(String::new());
        let quota_value = cli.quota_value.unwrap_or(0.0);
        limit_increase(quota_name, quota_value).await;
    } else if cli.request_status {
        let request_id = cli.request_id.unwrap_or(String::new());
        if request_id.is_empty() {
            println!("Request ID must be provided.");
            return;
        }
        let status = service_quotas::get_requested_service_quota_status(request_id)
            .await
            .context("Failed to get requested service quota status")
            .unwrap();

        println!("Status: {}", status);
    } else {
        println!("No recognised action specified.");
    }

    let mut vcpu_request_id = service_quotas::get_latest_request_id(
        service_quotas::EC2_SERVICE_CODE.to_string(), 
        service_quotas::VCPU_QUOTA_CODE.to_string())
        .await;
    let mut elastic_ip_request_id = service_quotas::get_latest_request_id(
        service_quotas::EC2_SERVICE_CODE.to_string(),
        service_quotas::ELASTIC_IP_QUOTA_CODE.to_string())
        .await;
    
    let interval_duration = Duration::from_secs(cli.monitor_interval_secs); 
    let mut interval = interval(interval_duration);
    
    loop {
        interval.tick().await;
    
        scheduled_tasks::request_monitor(&mut vcpu_request_id, 
            &mut elastic_ip_request_id, 
            cli.no_update_days_threshold)
            .await;
    
        scheduled_tasks::usage_monitor(&mut vcpu_request_id, 
            &mut elastic_ip_request_id, 
            cli.vcpu_usage_threshold_percent, 
            cli.vcpu_qouta_increment_percent, 
            cli.elastic_ip_usage_threshold_percent, 
            cli.elastic_ip_quota_increment_percent)
            .await;
    }
}

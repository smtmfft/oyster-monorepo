mod current_usage;
mod service_quotas;
mod utils;

use anyhow::Context;
use chrono;
use clap::Parser;

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
        "VCPU: {}/{}, Elastic IP: {}/{}",
        no_vcpus, vcpu_limit, no_elastic_ips, elastic_ip_limit
    );
}

async fn limit_increase(quota_name: String, quota_value: f64) {
    let possible_quota_names: [&str; 2] = ["ec2", "elastic_ip"];
    if possible_quota_names.contains(&quota_name.as_str()) {
        if quota_value == 0.0 {
            println!("Quota value must be greater than 0.");
            return;
        }
        let request_id = service_quotas::request_service_quota_increase(
            service_quotas::EC2_SERVICE_CODE.to_string(),
            quota_name.to_string(),
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
        println!("Quota name must be one of these:\n1. ec2\n2. elastic_ip");
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
}

mod current_usage;
mod scheduled_tasks;
mod service_quotas;
mod utils;

use anyhow::{Context, Result};
use aws_config::SdkConfig;
use aws_types::region::Region;
use chrono::Local;
use clap::{Parser, Subcommand};
use tokio::time::{interval, Duration};

#[derive(Parser, Clone)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(long, value_parser)]
    limit_status: bool,

    #[clap(long, value_parser)]
    limit_increase: bool,

    #[clap(long, value_parser)]
    request_status: bool,

    #[clap(long, value_parser, default_value = "")]
    quota_name: String,

    #[clap(long, value_parser, default_value = "0.0")]
    quota_value: f64,

    #[clap(long, value_parser, default_value = "")]
    request_id: String,

    #[clap(long, value_parser, default_value = "900")]
    monitor_interval_secs: u64,

    #[clap(long, value_parser, default_value = "5")]
    no_update_days_threshold: i64,

    #[clap(long, value_parser, default_value = "75.0")]
    vcpu_usage_threshold_percent: f64,

    #[clap(long, value_parser, default_value = "75.0")]
    elastic_ip_usage_threshold_percent: f64,

    #[clap(long, value_parser, default_value = "50.0")]
    vcpu_quota_increment_percent: f64,

    #[clap(long, value_parser, default_value = "50.0")]
    elastic_ip_quota_increment_percent: f64,

    #[clap(long, value_parser)]
    aws_profile: String,

    #[clap(
        long,
        value_parser,
        default_value = "us-east-1,us-east-2,us-west-1,us-west-2,ca-central-1,sa-east-1,eu-north-1,eu-west-3,eu-west-2,eu-west-1,eu-central-1,eu-central-2,eu-south-1,eu-south-2,me-south-1,me-central-1,af-south-1,ap-south-1,ap-south-2,ap-northeast-1,ap-northeast-2,ap-northeast-3,ap-southeast-1,ap-southeast-2,ap-southeast-3,ap-southeast-4,ap-east-1"
    )]
    aws_regions: String,

    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    // get status of a specific quota in a specific region
    Status {
        #[clap(long, value_parser = utils::Quota::from_name, num_args = 1.., value_delimiter = ',', default_value = "vcpus,eips")]
        quotas: Vec<utils::Quota>,

        #[clap(long, value_parser, num_args = 1.., value_delimiter = ',',
        default_value = "us-east-1,us-east-2,us-west-1,us-west-2,ca-central-1,sa-east-1,eu-north-1,eu-west-3,eu-west-2,eu-west-1,eu-central-1,eu-central-2,eu-south-1,eu-south-2,me-south-1,me-central-1,af-south-1,ap-south-1,ap-south-2,ap-northeast-1,ap-northeast-2,ap-northeast-3,ap-southeast-1,ap-southeast-2,ap-southeast-3,ap-southeast-4,ap-east-1"
        )]
        regions: Vec<String>,

        #[clap(long, value_parser)]
        profile: String,
    },
}

// async fn limit_status(quota_name: &str, config: &SdkConfig) {
//     let current_usage = current_usage::get_current_usage(quota_name, config).await;
//     if current_usage.is_err() {
//         eprintln!(
//             "Failed to get current usage of {}: {}",
//             quota_name,
//             current_usage.unwrap_err()
//         );
//         return;
//     }
//
//     let quota_limit = service_quotas::get_service_quota_limit(
//         config,
//         utils::EC2_SERVICE_CODE.to_string(),
//         utils::map_quota_to_code(quota_name).unwrap(),
//     )
//     .await;
//     if quota_limit.is_err() {
//         eprintln!(
//             "Failed to get {} quota limit/value: {}",
//             quota_name,
//             quota_limit.unwrap_err()
//         );
//         return;
//     }
//
//     println!(
//         "{}: {}/{}",
//         quota_name,
//         current_usage.unwrap(),
//         quota_limit.unwrap()
//     );
// }
//
// async fn limit_increase(quota_name: &str, quota_value: f64, config: &SdkConfig) {
//     let quota_code = utils::map_quota_to_code(quota_name);
//     if quota_code.is_none() {
//         eprintln!("Quota name must be one of these:\n1. vcpu\n2. elastic_ip");
//         return;
//     }
//
//     if quota_value == 0.0 {
//         eprintln!("Quota value must be greater than 0.0");
//         return;
//     }
//
//     match service_quotas::request_service_quota_increase(
//         config,
//         utils::EC2_SERVICE_CODE.to_string(),
//         quota_code.unwrap(),
//         quota_value,
//     )
//     .await
//     {
//         Ok(id) => {
//             println!(
//                 "Request ID: {}\nQuota Name: {}\nQuota Value: {}\nTime: {}\n\n",
//                 id,
//                 quota_name,
//                 quota_value,
//                 Local::now().format("%Y-%m-%d %H:%M:%S")
//             );
//
//             println!("Service quota increase requested!");
//             println!("Request ID: {}", id);
//         }
//         Err(err) => eprintln!("Failed to request limit increase: {}", err),
//     }
// }
//
// async fn request_status(request_id: &str, config: &SdkConfig) {
//     if request_id.is_empty() {
//         eprintln!("Valid request ID must be provided");
//         return;
//     }
//
//     match service_quotas::get_requested_service_quota_status(config, request_id.to_string()).await {
//         Ok(stat) => println!("Status: {}", stat),
//         Err(err) => eprintln!("Failed to get the status of provided request ID: {}", err),
//     }
// }
//
// async fn schedule_monitoring(cli: Cli, region: String) {
//     let config = aws_config::from_env()
//         .profile_name(cli.aws_profile.as_str())
//         .region(Region::new(region))
//         .load()
//         .await;
//
//     let mut vcpu_request_id = scheduled_tasks::get_id(&config, utils::VCPU_QUOTA_NAME).await;
//     let mut elastic_ip_request_id =
//         scheduled_tasks::get_id(&config, utils::ELASTIC_IP_QUOTA_NAME).await;
//
//     let interval_duration = Duration::from_secs(cli.monitor_interval_secs);
//     let mut interval = interval(interval_duration);
//
//     loop {
//         interval.tick().await;
//
//         vcpu_request_id = scheduled_tasks::request_monitor(
//             &config,
//             vcpu_request_id,
//             utils::VCPU_QUOTA_NAME,
//             cli.no_update_days_threshold,
//         )
//         .await;
//         elastic_ip_request_id = scheduled_tasks::request_monitor(
//             &config,
//             elastic_ip_request_id,
//             utils::ELASTIC_IP_QUOTA_NAME,
//             cli.no_update_days_threshold,
//         )
//         .await;
//
//         vcpu_request_id = scheduled_tasks::usage_monitor(
//             &config,
//             vcpu_request_id,
//             utils::VCPU_QUOTA_NAME,
//             cli.vcpu_usage_threshold_percent,
//             cli.vcpu_quota_increment_percent,
//         )
//         .await;
//         elastic_ip_request_id = scheduled_tasks::usage_monitor(
//             &config,
//             elastic_ip_request_id,
//             utils::ELASTIC_IP_QUOTA_NAME,
//             cli.elastic_ip_usage_threshold_percent,
//             cli.elastic_ip_quota_increment_percent,
//         )
//         .await;
//     }
// }

async fn quota_status(quota: &utils::Quota, region: &str, aws_profile: &str) -> Result<()> {
    let config = aws_config::from_env()
        .profile_name(aws_profile)
        .region(Region::new(region.to_owned()))
        .load()
        .await;

    let ec2_client = aws_sdk_ec2::Client::new(&config);
    let current_usage = current_usage::get_current_usage(&ec2_client, quota)
        .await
        .with_context(|| format!("failed to get current usage of {quota}"))?;

    let sq_client = aws_sdk_servicequotas::Client::new(&config);

    let quota_limit = service_quotas::get_service_quota_limit(&sq_client, quota)
        .await
        .with_context(|| format!("failed to get quota limit of {quota}"))?;

    println!("{region}:\t{quota}:\t{current_usage}/{quota_limit}");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Commands::Status {
            quotas,
            regions,
            profile,
        } => {
            for region in regions {
                for quota in quotas.as_slice() {
                    quota_status(&quota, &region, &profile).await?;
                }
            }
        }
    };

    Ok(())

    // let config = aws_config::load_from_env().await;
    //
    // if cli.limit_status {
    //     limit_status(utils::VCPU_QUOTA_NAME, &config).await;
    //     limit_status(utils::ELASTIC_IP_QUOTA_NAME, &config).await;
    // } else if cli.limit_increase {
    //     limit_increase(cli.quota_name.as_str(), cli.quota_value, &config).await;
    // } else if cli.request_status {
    //     request_status(cli.request_id.as_str(), &config).await;
    // }
    //
    // let regions: Vec<String> = cli.aws_regions.split(',').map(|r| r.into()).collect();
    //
    // let mut handles = vec![];
    // for region in regions {
    //     handles.push(tokio::spawn(schedule_monitoring(cli.clone(), region)));
    // }
    //
    // for handle in handles {
    //     if let Err(err) = handle.await {
    //         println!(
    //             "[{}] Error occurred while running a scheduled monitor: {:?}",
    //             Local::now().format("%Y-%m-%d %H:%M:%S"),
    //             err
    //         );
    //     }
    // }
}

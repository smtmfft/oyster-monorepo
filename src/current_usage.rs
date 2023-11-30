use crate::utils;

use anyhow::{Result, Context, anyhow};
use aws_config;
use aws_sdk_ec2;
use aws_sdk_ec2::types::Filter;

pub async fn get_current_usage(quota_name: &str) -> Result<i32> {
    match quota_name {
        utils::VCPU_QUOTA_NAME => get_no_of_vcpus().await,
        utils::ELASTIC_IP_QUOTA_NAME => get_no_of_elastic_ips().await,
        _ => Err(anyhow!("Invalid quota name, must be one of 'vcpu' or 'elastic_ip'")),
    }
}

async fn get_no_of_vcpus() -> Result<i32> {
    let config = aws_config::load_from_env().await;
    let client = aws_sdk_ec2::Client::new(&config);

    let res = client
        .describe_instances()
        .filters(
            Filter::builder()
                .name("instance-state-name")
                .values("running")
                .build(),
        )
        .send()
        .await
        .context("Error occurred while describing instances from AWS client")?;
    let reservations = res
        .reservations()
        .ok_or(anyhow!("Could not parse reservations from AWS response"))?;

    let mut no_of_vcpus = 0;

    for reservation in reservations {
        let instances = reservation
            .instances()
            .ok_or(anyhow!("Could not parse instances from reservation"))?;

        for instance in instances {
            let cpu_options = instance
                .cpu_options()
                .ok_or(anyhow!("Could not parse cpu options from instance"))?;

            no_of_vcpus += (cpu_options
                .core_count()
                .ok_or(anyhow!("Could not parse core count from cpu options"))?) as i32
                * (cpu_options
                    .threads_per_core()
                    .ok_or(anyhow!("Could not parse threads per core from cpu options"))?) as i32;
        }
    }

    Ok(no_of_vcpus)
}

async fn get_no_of_elastic_ips() -> Result<i32> {
    let config = aws_config::load_from_env().await;
    let client = aws_sdk_ec2::Client::new(&config);

    Ok(client
        .describe_addresses()
        .send()
        .await
        .context("Error occurred while describing addresses from AWS client")?
        .addresses()
        .ok_or(anyhow!("Could not parse addresses from AWS response"))?
        .len() as i32
    )
}

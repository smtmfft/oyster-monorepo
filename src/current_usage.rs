use anyhow::{anyhow, Context, Result};
use aws_config;
use aws_sdk_ec2;
use aws_sdk_ec2::types::Filter;

pub async fn get_no_of_vcpus() -> Result<i32> {
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
        .context("could not describe instances")?;

    let reservations = res
        .reservations()
        .ok_or(anyhow!("could not parse reservations"))?;

    let mut no_of_vcpus = 0;

    for reservation in reservations {
        let instances = reservation
            .instances()
            .ok_or(anyhow!("could not parse instances"))?;

        for instance in instances {
            let cpu_options = instance
                .cpu_options()
                .ok_or(anyhow!("could not parse cpu options"))?;
            no_of_vcpus += (cpu_options
                .core_count()
                .ok_or(anyhow!("could not parse core count"))?) as i32
                * (cpu_options
                    .threads_per_core()
                    .ok_or(anyhow!("could not parse threads per core"))?) as i32;
        }
    }

    Ok(no_of_vcpus)
}

pub async fn get_no_elatic_ips() -> Result<i32> {
    let config = aws_config::load_from_env().await;
    let client = aws_sdk_ec2::Client::new(&config);

    let res = client
        .describe_addresses()
        .send()
        .await
        .context("could not describe addresses")?;

    let no_of_ips = res.addresses().unwrap().len() as i32;

    Ok(no_of_ips)
}

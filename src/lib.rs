use std::time::Duration;

use anyhow::{ensure, Context, Result};
use aws_sdk_ec2::client::Waiters;
use hyper::{client::HttpConnector, Body, Client, Request};
use tokio::time::timeout;

use tracing::{debug, info};

// Acquire a token from the AWS API.
async fn get_ec2_token(client: &Client<HttpConnector>) -> Result<String> {
    const AWS_TOKEN_API_URL: &str = "http://169.254.169.254/latest/api/token";
    let req = Request::builder()
        .method("PUT")
        .uri(AWS_TOKEN_API_URL)
        .header("X-aws-ec2-metadata-token-ttl-seconds", "21600")
        .body(Body::empty())?;
    send(client, req).await.context("failed to get ec2 token")
}

// Read a metadata value from the AWS API.
async fn get_ec2_value(client: &Client<HttpConnector>, url: &str, token: &str) -> Result<String> {
    let req = Request::builder()
        .method("GET")
        .uri(url)
        .header("X-aws-ec2-metadata-token", token)
        .body(Body::empty())?;
    send(client, req).await.context("failed to get ec2 value")
}

// Send an HTTP request, returning the body as a string.
async fn send(client: &Client<HttpConnector>, req: Request<Body>) -> Result<String> {
    let resp = timeout(Duration::from_secs(3), client.request(req)).await??;
    let status = resp.status();
    ensure!(status.is_success(), "failed metadata request: {status}");
    let body_bytes = hyper::body::to_bytes(resp.into_body()).await?.to_vec();
    Ok(std::str::from_utf8(&body_bytes)?.into())
}

async fn get_ec2_instance_id_and_zone() -> Result<(String, String)> {
    const AWS_INSTANCE_ID_URL: &str = "http://169.254.169.254/latest/meta-data/instance-id";
    const AWS_INSTANCE_AVAILABILITY_ZONE: &str =
        "http://169.254.169.254/latest/meta-data/placement/availability-zone";
    let client = Client::new();
    let token = get_ec2_token(&client).await?;
    let id = get_ec2_value(&client, AWS_INSTANCE_ID_URL, &token).await?;
    let zone = get_ec2_value(&client, AWS_INSTANCE_AVAILABILITY_ZONE, &token).await?;
    Ok((id, zone))
}

/// Load an Amazon Machine Image (AMI) to a device on the current EC2 host.
pub async fn load_ami_to_device(ami_id: String, device_path: String) -> Result<()> {
    // TODO: check that host is EC2 instance.
    // TODO: check that ami_id is valid.
    ensure!(
        !std::path::Path::new(&device_path).exists(),
        "device path {} already exists",
        device_path
    );

    // Find the snapshot id
    let ec2_client = aws_sdk_ec2::Client::new(&aws_config::load_from_env().await);
    let describe_images_output = ec2_client
        .describe_images()
        .image_ids(ami_id.clone())
        .send()
        .await?;

    let snapshot_id = describe_images_output
        .images
        .unwrap_or_default()
        .get(0)
        .and_then(|image| {
            image.block_device_mappings.as_ref().and_then(|mappings| {
                mappings.get(0).and_then(|mapping| {
                    mapping.ebs.as_ref().and_then(|ebs| ebs.snapshot_id.clone())
                })
            })
        })
        .expect("Failed to find snapshot ID for the given AMI ID");

    let (ec2_host_instance_id, zone) = get_ec2_instance_id_and_zone().await?;

    info!("ec2 host instance id: {}", ec2_host_instance_id);
    info!("snapshot id: {}", snapshot_id);

    // Create the volume
    let create_volume_output = ec2_client
        .create_volume()
        .availability_zone(zone)
        .snapshot_id(snapshot_id)
        .send()
        .await?;
    let volume_id = create_volume_output
        .volume_id
        .expect("Failed to create volume");

    let max_wait = Duration::from_secs(60);
    info!(
        "waiting up-to {} seconds for volume {} to be available",
        max_wait.as_secs(),
        volume_id
    );
    ec2_client
        .wait_until_volume_available()
        .volume_ids(volume_id.clone())
        .wait(max_wait)
        .await?;

    ec2_client
        .attach_volume()
        .device(device_path.clone())
        .volume_id(volume_id.clone())
        .instance_id(ec2_host_instance_id)
        .send()
        .await?;

    // TODO(Jonathon): this doesn't actually work because EC2 uses dynamic device naming
    // and puts the device at /dev/nvme2n1 or something.
    //
    // If the volume attach actually worked then you'll see a new /dev/nvme* in blockdev --report.
    //
    // $ sudo file -s /dev/nvme2n1
    // /dev/nvme2n1: SGI XFS filesystem data (blksz 4096, inosz 512, v2 dirs)
    //
    // Above you can see the attach worked and gave me an XFS filesystem to mount.
    //
    while !std::path::Path::new(&device_path).exists() {
        tokio::time::sleep(Duration::from_millis(500)).await;
        debug!("still waiting for device to be attached at {}", device_path);
    }

    Ok(())
}

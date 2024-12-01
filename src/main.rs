use anyhow::Result;
use aws_config::load_from_env;
use aws_sdk_s3::Client;
use clap::{Args, Parser, Subcommand};
use tracing::level_filters::LevelFilter;
use tracing_subscriber;
use tracing_subscriber::EnvFilter;

const NAME: &str = "vmi";

/// Virtual machine images made simple!
#[derive(Debug, Parser)]
#[clap(name = NAME, version)]
pub struct App {
    #[clap(flatten)]
    global_opts: GlobalOpts,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Args)]
struct GlobalOpts {
    /// Verbosity level (can be specified multiple times)
    #[clap(long, short, global = true, default_value_t = 0)]
    verbose: usize,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Convert between virtual machine image formats
    Convert {
        /// Source of the virtual machine image (e.g. Amazon Machine Image (AMI)).
        source: Source,
        /// Source ID (e.g. ami-1234 for an Amazon Machine Image (AMI)).
        source_id: String,
        /// Destination of the converted virtual machine image data.
        sink: Sink,
        /// Sink ID (e.g. /dev/xvdg for a device).
        sink_id: String,
    },
    /// Return information on virtual machine images
    Inspect {
        /// Source type of the virtual machine image (e.g. Amazon Machine Image (AMI)).
        source: Source,
        /// Source ID (e.g. /path/to/raw.img for a local Raw format image).
        source_id: String,
    },
}

#[derive(Debug, clap::ValueEnum, Clone)]
enum Source {
    /// Amazon Machine Image (AMI)
    Ami,
    /// Raw format image
    Raw,
    // Add other variants as needed
}

#[derive(Debug, clap::ValueEnum, Clone)]
enum Sink {
    /// Device path on the host machine. e.g /dev/xvdg.
    Device,
    // Add other variants as needed
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = App::parse();

    let level = match cli.global_opts.verbose {
        3.. => LevelFilter::DEBUG.into(),
        2 => LevelFilter::DEBUG.into(),
        1 => LevelFilter::INFO.into(),
        0 => LevelFilter::WARN.into(),
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(level)
                .from_env_lossy(),
        )
        .init();

    let shared_config = load_from_env().await;
    let s3_client = Client::new(&shared_config);

    let response = s3_client.list_buckets().send().await?;

    println!("Buckets:");
    if let Some(buckets) = response.buckets {
        for bucket in buckets {
            let name = bucket.name().unwrap_or("Unnamed");
            let creation_date = bucket
                .creation_date()
                .map_or("Unknown".to_string(), |cd| cd.to_string());
            println!("  - {} (created: {})", name, creation_date);
        }
    } else {
        println!("No buckets found.");
    }

    Ok(())
}

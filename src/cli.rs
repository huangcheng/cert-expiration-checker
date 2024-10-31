use clap::{Parser};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// A file contains list of domains to be checked.
    pub(crate) file: Option<String>,
}

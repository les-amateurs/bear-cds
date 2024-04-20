use anyhow::Context; // Import the Context trait
use crate::{
    Config,
    rctf,
};
// Command to fetch the leaderboard and save it to a file, using the rctf module
pub async fn command(config: Config) -> Result<(), anyhow::Error> {
    // use the rctf fetch_leaderboard function to get the leaderboard
    rctf::fetch_leaderboard(&config).await
        .context("Failed to fetch leaderboard")?; // Add .context() method to the Result
    Ok(())
}

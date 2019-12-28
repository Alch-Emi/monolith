pub mod asset;
pub mod resources;
pub mod util;

use reqwest::Client;
use reqwest::Url;

use asset::Asset;

pub async fn download_complete_page(url: Url) -> asset::Result<String> {
    let client = Client::new();
    let mut asset = Asset::new(
        url,
        "text/plain".to_owned(),
    );
    asset.download_complete(&client).await?;
    asset.try_stringify()
}

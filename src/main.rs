use reqwest::Client;
use reqwest::Url;

use monolith::asset::Asset;

#[tokio::main(single_thread)]
async fn main() {
    let client = Client::new();
    let mut asset = Asset::new(
        Url::parse("http://localhost:2015/").unwrap(),
        "text/plain".to_string(),
    );
    asset.download_complete(&client).await.unwrap();
    println!(
        "{}",
        asset.try_stringify().unwrap()
    );
}

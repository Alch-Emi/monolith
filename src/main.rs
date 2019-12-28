use reqwest::Url;

use monolith;

#[tokio::main(single_thread)]
async fn main() {
    println!(
        "{}",
        monolith::download_complete_page(
            Url::parse(
                &std::env::args()
                    .skip(1)
                    .next()
                    .expect("Missing argument")
            ).expect("Bad URL")
        )
            .await
            .expect("Error getting page")
    );
}

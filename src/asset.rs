use bytes::Bytes;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use reqwest::Url;
use reqwest::Client;

use std::boxed::Box;

// use crate::dummy_resource::DummyResource;
use crate::resources::InertResource;

pub trait Resource {
    fn parse(&mut self, bytes: Bytes) -> Result<()>;
    fn has_data(&self) -> bool;
    fn needed_assets(&mut self) -> Vec<&mut Asset>;
    fn into_bytes(self) -> Result<Bytes>;
}

pub struct Asset {
    pub url: Url,
    pub mime_hint: String,
    pub data: Option<Box<dyn Resource>>,
}

pub enum Error {
    SelfUnloaded,
    AssetsUnloaded,
    HttpError(reqwest::Error),
    ParseError(Box<dyn std::error::Error>),
}
use Error::*;

pub type Result<T> = std::result::Result<T, Error>;

impl Asset {
    pub fn new(url: Url, mime: String) -> Asset {
        Asset { url, mime, data: None };
    }

    pub async fn download(
        &mut self,
        client: &Client
    ) -> Result<Vec<&mut Asset>> {

        // If this asset hasn't formed yet, create a resource for it.
        let inner_resource = self.data.get_or_insert_with(|| {
            // Attempt to pick a default resource type by MIME type
            Box::new(
                //if mime.ignore_ascii_case_eq("text/plain") {
                //    DummyResource::new(self.url.clone())
                //} else {
                    InertResource::default()
                //}
            )
        });

        // If the asset hasn't been filled with data yet, download and fill it
        if !inner_resource.has_data() {
            // Get bytes
            let bytes = match client.get(self.url.clone())
                .send()
                .await
            {
                Ok(response) => match response
                    .bytes()
                    .await
                {
                    Ok(bytes) => bytes,
                    Err(e) => return Err(HttpError(e)),
                },
                Err(e) => return Err(HttpError(e)),
            };

            // Fill
            inner_resource.parse(bytes)?;
        }

        // Return any new assets that need to be downloaded
        inner_resource.needed_assets()
    }

    /// Asyncronously download all needed assets in parallel
    pub async fn download_complete(
        &mut self,
        client: &Client
    ) -> Result<()> {

        // Create a queue of pending futures
        let mut to_download = FuturesUnordered::new();
        to_download.push(self.download(client));

        // When a future becomes ready
        while let Some(download_results) = to_download.next().await {
            match download_results {
                Ok(undownloaded_assets) => {
                    // Will return a list of new assets to be downloaded.
                    // Download each new asset
                    for asset in undownloaded_assets {
                        to_download.push(asset.download(client));
                    }
                },
                Err(Error::SelfUnloaded) | Err(Error::AssetsUnloaded) => {
                    unreachable!();
                },
                Err(HttpError(e)) => {
                    eprintln!("HTTP Error: {}", e);
                },
                Err(ParseError(e)) => {
                    eprintln!("Warning: Parser error: {}", e.as_ref());
                },
            }
        }

        Ok(())
    }
}

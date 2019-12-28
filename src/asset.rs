use bytes::Bytes;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use reqwest::Client;
use reqwest::Url;

use std::boxed::Box;

use crate::resources::DemoResource;
use crate::resources::InertResource;

pub trait Resource {
    fn parse(&mut self, bytes: Bytes) -> Result<()>;
    fn has_data(&self) -> bool;
    fn needed_assets(&mut self) -> Vec<&mut Asset>;
    fn render(&self) -> Result<Bytes>;
}

pub struct Asset {
    pub url: Url,
    pub mime_hint: String,
    pub data: Option<Box<dyn Resource>>,
}

#[derive(Debug)]
pub enum Error {
    AssetUnloaded,
    HttpError(reqwest::Error),
    ParseError(Box<dyn std::error::Error>),
    MissingResource,
}
use Error::*;

pub type Result<T> = std::result::Result<T, Error>;

impl Asset {
    pub fn new(url: Url, mime_hint: String) -> Asset {
        Asset {
            url,
            mime_hint,
            data: None,
        }
    }

    pub async fn download(
        &mut self,
        client: &Client
    ) -> Result<Vec<&mut Asset>> {

        // If this asset hasn't formed yet, throw an error
        let inner_resource = self.data.as_mut().ok_or(MissingResource)?;

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
        Ok(inner_resource.needed_assets())
    }

    pub fn auto_select_resource_type(&mut self) -> &mut Box<dyn Resource> {
        let mime = &self.mime_hint;
        let url = &self.url;
        let inner_resource = self.data.get_or_insert_with(|| {
            // Attempt to pick a default resource type by MIME type
            if mime.eq_ignore_ascii_case("text/plain") {
                Box::new(DemoResource::new(url.clone()))
            } else {
                Box::new(InertResource::default())
            }
        });
        return inner_resource;
    }

    /// Asyncronously download all needed assets in parallel
    pub async fn download_complete(
        &mut self,
        client: &Client
    ) -> Result<()> {
        // Pick a parser, if not already selected
        self.auto_select_resource_type();

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
                        asset.auto_select_resource_type();
                        to_download.push(asset.download(client));
                    }
                },
                Err(Error::AssetUnloaded) | Err(MissingResource) => {
                    unreachable!();
                }
                Err(HttpError(e)) => {
                    eprintln!("HTTP Error: {}", e);
                }
                Err(ParseError(e)) => {
                    eprintln!("Warning: Parser error: {}", e.as_ref());
                }
            }
        }

        Ok(())
    }

    pub fn try_stringify(&self) -> Result<String>  {
        std::str::from_utf8(
            &self.data
                .as_ref()
                .ok_or(Error::MissingResource)?
                .render()?
        )
            .map_err(|e| Error::ParseError(Box::new(e)))
            .map(str::to_owned)
    }
}

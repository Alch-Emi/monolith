//! A framework for the asynchronous download of recursive web assets
//!
//! This module contains two important elements: the trait [Resource] and the
//! struct [Asset].  A resource is effectively a parser for a certain type of
//! thing that might be found on the web.  Resources expose dependent [Asset]s,
//! which are any kind of web thing that is needed the finish parsing or
//! rendering that resource.  An Asset is primarily just a URL and a mime type
//! to hint at the kind of parser it needs.  However, it offers a method to
//! download itself, which creates and downloads a resource and attaches it to
//! the Asset.
//!
//! The end result of this process is a workflow that looks like this:
//!
//!  1. Create an asset with a MIME type and URL
//!  2. Download that asset
//!  3. List the assets needed to render the original Asset
//!  4. Download each of those assets (and repeat)
//!  5. Render original Asset (this calls the [Resource::render] method of the
//!     underlying Resource, which presumably then renders its subordinate
//!     Assets)

use bytes::Bytes;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use reqwest::Client;
use reqwest::Url;

use std::boxed::Box;

use crate::resources::DemoResource;
use crate::resources::InertResource;

/// A parser and renderer for a certain type of data
///
/// A `Resource` effectively serves two purposes:
///
/// * Given some data, determine what [Asset]s need to be downloaded to render
///   that data for offline use (this occurs in the [Resource::needed_assets]
///   method)
/// * Once all needed assets are renderable, render this asset (this occurs in
///   the [Resource::render] method)
pub trait Resource {

    /// Read in some data
    ///
    /// This is the intake method for the resource, where it receives and
    /// parses the its structural data.  Implementers need not assume that this
    /// method will be called more than once per instance.  The passed in
    /// information may or may not conform to the grammar of this resource, but
    /// in any case, unless an error is returned, after this method is completed
    /// it is expected that [Resource::has_data] will return true and
    /// [Resource::needed_assets] will be ready to be called.
    fn parse(&mut self, bytes: Bytes) -> Result<()>;

    /// Determine if the `Resource` has been fed data yet
    ///
    /// Effectively returns true if `parse` has been called
    fn has_data(&self) -> bool;

    /// The [Asset]s that must be downloaded before rendering this `Resource`
    ///
    /// This method produces a [Vec] containing all of the remote assets that
    /// need to be provided in order to [render](Resource::render) this Resource.
    /// Before render is called, it is expected that
    ///
    /// * All of the assets in this vector contain resources
    /// * All of the assets in this vector [have data](Resource::has_data).
    /// * All of the subordinate assets have their needed assets fulfilled
    ///
    /// Because this method returns mutable references to these assets,
    /// inserting and filling the assets with Resources can be done by directly
    /// modifying the returned references.
    ///
    /// This method should not be called before this resource
    /// [has_data](Resource::has_data), and may panic or return erroneous
    /// information if it is.
    fn needed_assets(&mut self) -> Vec<&mut Asset>;

    /// Render this resource with all previously remote assets embedded
    ///
    /// Unless an error occurs, this should produce a [Bytes] object that
    /// approximates the original data passed into [Resource::parse], except
    /// that the remote assets referenced by the original data (and then
    /// expressed as [Asset]s following a call to
    /// [needed_assets][Resource::needed_assets]) have been embedded directly
    /// into the document.
    ///
    /// For an HTML document, this looks like replacing a reference like
    /// ```html
    /// <img src="https://example.com/image.webp"/>
    /// ```
    /// with a dataurl to directly embed that asset into the page
    /// ```html
    /// <img src="data:image/webp;base64,ZDg6Y...YWN"/>
    /// ```
    ///
    /// If this method is called before [Resource::parse] has been successfully
    /// called, then it should return an [Error::AssetUnloaded] to indicate
    /// this.
    fn render(&self) -> Result<Bytes>;
}

/// A wrapper around a reference to some remote data and the downloaded copy.
///
/// `Asset`s are effectively a pairing between a [Url], which is some remote
/// resource, and a [Resource], which is the local copy an structure for that
/// remote resource.
///
/// Assets start off as just URLs.  However, they can then be downloaded,
/// yielding an Asset with an attached [Resource].
///
/// The complete life cycle of an Asset looks something like this:
///
///  1. Create the asset with just a URL and a MIME type
///  2. Add an empty [Resource] to the Asset to serve as a parser
///  3. Feed the Resource data downloaded from the Asset's [Url]
///  4. Download child [Asset]s
///  5. Use the now complete [Resource], discarding the Asset
///
/// The implementation of Asset contains several utility methods to aid this
/// process, most importantly, steps 3 and 4.
pub struct Asset {

    /// The URL that the data for this Asset may be downloaded from
    pub url: Url,

    /// A MIME type to help choose a parser for this resource.
    ///
    /// This may be left blank (i.e. an empty string) if the MIME type is
    /// unknown, or only the first part of the MIME may be provided (e.g.
    /// "image" instead of "image/webp")
    pub mime_hint: String,

    /// The `Resource` for parsing and rendering this Asset
    pub data: Option<Box<dyn Resource>>,
}

#[derive(Debug)]
pub enum Error {

    /// Denotes that there was an attempt to [render][Resource::render] or
    /// otherwise work with a [Resource] that hadn't been supplied data
    AssetUnloaded,

    /// Denotes a network error that occurred while fetching an asset
    HttpError(reqwest::Error),

    /// Denotes some sort of syntactic error that prevented a [Resource] from
    /// completely [parsing][Resource::parse] it's data
    ParseError(Box<dyn std::error::Error>),

    /// Denote an attempt to work with an [Asset] that hadn't had a [Resource]
    /// set, when one was expected.  (i.e. that Asset's `data` was None)
    MissingResource,
}
use Error::*;

pub type Result<T> = std::result::Result<T, Error>;

impl Asset {

    /// Produce a new Asset targeting a certain URL
    ///
    /// For restrictions on the `mime_hint`, please see [Asset::mime_hint]
    /// (unless you're determining the [Resource] yourself).
    pub fn new(url: Url, mime_hint: String) -> Asset {
        Asset {
            url,
            mime_hint,
            data: None,
        }
    }

    /// Connect to the internet and populate this Asset's [Resource] with data
    ///
    /// This method attempts to connect to the internet, download the URL of
    /// this Asset, and send it to the [Resource::parse] method of the
    /// [Resource] in the Asset.
    ///
    /// This then returns [Resource::needed_assets], which are all of the newly
    /// formed Assets that need to be downloaded as well.
    ///
    /// Possible error conditions:
    /// * `MissingResource`: A parser ([Resource]) hasn't been selected yet.  Call
    ///   [Asset::auto_select_resource_type] or set [Asset::data] yourself.
    /// * `HttpError`: An error returned by reqwest while attempting to download
    /// * Other errors can be returned by [Resource::parse]
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

    /// Attempt to select a [Resource] type based on the MIME
    ///
    /// This method uses [Asset::mime_hint] to make a guess about what to
    /// populate [Asset::data] with.  If the resource type has already been
    /// selected, then it is kept.
    ///
    /// The exact mechanics of this method haven't been solidified yet, as not
    /// all relevant Resources have been added.
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

    /// Asynchronously download all needed assets in parallel
    ///
    /// This is a convenience method that automatically handles a lot of the
    /// boilerplate of the Asset life cycle at the cost of some control.
    ///
    /// Specifically, this method handles
    /// * Automatically selecting a resource type
    /// * Downloading and parsing the asset
    /// * and handling all subordinate assets
    ///
    /// Any encountered errors are handled by printing an error to stdout, but
    /// are otherwise ignored.  This will eventually be replaced with a proper
    /// logging protocol, or maybe returning all errors at the end.
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

    /// Attempt to render this Asset as a [String]
    ///
    /// This relies on the [Resource::render] method, and thus requires that the
    /// Asset be downloaded in advance.
    ///
    /// This could error with:
    /// * [`Error::MissingResource`] if a Resource hasn't been selected yet
    /// * [`Error::ParseError`] if the bytes weren't UTF-8 formatted
    /// * Any other error returned by [Resource::render]
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

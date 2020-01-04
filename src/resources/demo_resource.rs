use bytes::Bytes;
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::Url;

use std::ops::Range;
use std::str;

use crate::asset::Asset;
use crate::asset::Error;
use crate::asset::Resource;
use crate::asset::Result;
use crate::util::data_to_dataurl;

lazy_static! {
    static ref URL_MATCH: Regex =
        Regex::new(r###"(?:'|")(?P<inner>[^"'\n\s]+?)(?:"|')"###).unwrap();
}

pub struct DemoResource {
    data: Option<String>,
    url: Url,
    resources: Vec<(Range<usize>, Asset)>,
}

impl DemoResource {
    pub fn new(url: Url) -> DemoResource {
        DemoResource {
            data: None,
            url,
            resources: vec![],
        }
    }
}

impl Resource for DemoResource {
    fn parse(&mut self, bytes: Bytes) -> Result<()> {
        if self.has_data() {
            panic!(".parse() called twice on DemoResource");
        }

        // Insert data
        self.data = Some(
            str::from_utf8(&bytes)
                .map_err(|e| Error::ParseError(Box::new(e)))?
                .to_owned(),
        );
        let data = self.data.as_ref().unwrap();

        // Find any potential URLs
        for link in URL_MATCH.captures_iter(data) {
            // The URL in the match
            let inner_match = link.name("inner").unwrap();

            // Validate URL
            if let Ok(url) = self.url.join(inner_match.as_str()) {
                // Check to see if it's text, by checking extension
                let is_text = inner_match.as_str().ends_with("txt");

                // Determine the range of text that would need to be replaced
                let range = inner_match.start()..inner_match.end();

                // Save as needed resource
                self.resources.push((
                    range,
                    Asset::new(
                        url,
                        if is_text { "text/plain" } else { "" }.to_owned(),
                    )
                ));

            }
        }

        Ok(())
    }

    fn has_data(&self) -> bool {
        self.data.is_some()
    }

    fn needed_assets(&mut self) -> Vec<&mut Asset> {
        self.resources.iter_mut()
            .map(|(_, m)| m)
            .collect()
    }

    fn render(&self) -> Result<Bytes> {

        // Unwrap inner content
        let mut content = self.data.clone().ok_or(Error::ResourceUnloaded)?;

        // Make replacements
        // Reversed so that ranges remain accurate
        for (range, data) in self.resources.iter().rev() {
            content.replace_range(
                range.clone(),
                &data_to_dataurl(
                    &data.mime_hint,
                    &data.data.as_ref().ok_or(
                        Error::ResourceUnloaded
                    )?.render()?,
                )
            );
        }

        // Return fully replaced string
        Ok(content.into())
    }
}

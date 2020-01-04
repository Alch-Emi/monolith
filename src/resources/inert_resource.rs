use bytes::Bytes;

use crate::asset::Asset;
use crate::asset::Error;
use crate::asset::Resource;
use crate::asset::Result;

#[derive(Default, Clone)]
pub struct InertResource {
    data: Option<Bytes>,
}

impl Resource for InertResource {
    fn parse(&mut self, bytes: Bytes) -> Result<()> {
        self.data = Some(bytes);
        Ok(())
    }

    fn has_data(&self) -> bool {
        self.data.is_some()
    }

    fn needed_assets(&mut self) -> Vec<&mut Asset> {
        vec![]
    }

    fn render(&self) -> Result<Bytes> {
        self.data.clone().ok_or(Error::ResourceUnloaded)
    }
}

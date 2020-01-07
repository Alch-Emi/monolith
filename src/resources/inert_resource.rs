use bytes::Bytes;

use crate::asset::Asset;
use crate::asset::Error;
use crate::asset::Resource;
use crate::asset::Result;

/// A Resource type that will never have any children
///
/// Ths resource represents any type of resource that requires no special
/// parsing, and is unable to link to any other remote assets.  Its
/// [needed_assets][Resource::needed_assets] is always an empty vector, and the
/// [render][Resource::render] method always produces the same data that was
/// passed in.
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

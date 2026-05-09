use crate::basic_box::{BasicBox, BoxInfo};
use crate::error::Result;
use std::path::PathBuf;
use std::sync::Arc;

pub struct ByteArrayBox {
    basic: Arc<BasicBox>,
}

impl ByteArrayBox {
    pub(crate) fn open(path: PathBuf, name: String) -> Result<Self> {
        Ok(Self {
            basic: Arc::new(BasicBox::open(path, name)?),
        })
    }

    pub fn put(&self, id: u64, data: Vec<u8>) -> Result<u64> {
        let id = self.basic.check_id_and_get(id)?;
        self.basic.append_store(id, &data)?;
        Ok(id)
    }

    pub fn get(&self, id: u64) -> Result<Vec<u8>> {
        self.basic.get_byte(id)
    }

    pub fn all(&self) -> Result<Vec<Vec<u8>>> {
        self.basic.all_bytes()
    }

    pub fn remove(&self, id: u64) -> Result<()> {
        self.basic.remove_entry(id)
    }

    pub fn clear(&self, re_init: bool) -> Result<()> {
        self.basic.clear(re_init)
    }

    pub fn compact(&self) -> Result<()> {
        self.basic.compact()
    }

    pub fn close(&self) -> Result<()> {
        self.basic.close()
    }

    pub fn get_info(&self) -> BoxInfo {
        self.basic.get_info()
    }
}

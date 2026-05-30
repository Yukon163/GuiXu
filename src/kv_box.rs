use crate::basic_box::{BasicBox, BoxInfo};
use crate::byte_ops::{
    bytes_to_f32_vec, bytes_to_f64_vec, bytes_to_i32, bytes_to_i32_vec, bytes_to_u64,
    bytes_to_u64_vec, f32_slice_to_bytes, f64_slice_to_bytes, i32_slice_to_bytes, i32_to_bytes,
    u64_slice_to_bytes, u64_to_bytes,
};
use crate::error::{GuiXuError, Result};
use crate::file_access::AutoIncreaseFileAccess;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::fs::remove_file;
use std::path::PathBuf;

const KV_ITEM_FIX_LENGTH: u64 = 13;
const BOOLEAN_TRUE: u8 = 1;
const VALUE_TYPE_DELETE: u8 = 0;
const VALUE_TYPE_BOOLEAN: u8 = 1;
const VALUE_TYPE_BYTE: u8 = 2;
const VALUE_TYPE_INT: u8 = 3;
const VALUE_TYPE_LONG: u8 = 4;
const VALUE_TYPE_FLOAT: u8 = 5;
const VALUE_TYPE_DOUBLE: u8 = 6;
const VALUE_TYPE_STRING: u8 = 7;
const VALUE_TYPE_BYTE_ARRAY: u8 = 8;
const VALUE_TYPE_INT_ARRAY: u8 = 9;
const VALUE_TYPE_LONG_ARRAY: u8 = 10;
const VALUE_TYPE_FLOAT_ARRAY: u8 = 11;
const VALUE_TYPE_DOUBLE_ARRAY: u8 = 12;

#[derive(Debug, Clone, PartialEq)]
enum KVValue {
    Deleted,
    Bool(bool),
    Byte(u8),
    Int(i32),
    Long(u64),
    Float(f32),
    Double(f64),
    String(Cow<'static, str>),
    ByteArray(Vec<u8>),
    IntArray(Vec<i32>),
    LongArray(Vec<u64>),
    FloatArray(Vec<f32>),
    DoubleArray(Vec<f64>),
}

#[derive(Debug, Clone)]
struct KVItem {
    file_position: u64,
    id: u64,
    value_type: u8,
    value: KVValue,
}

pub struct KVBox {
    basic: BasicBox,
    map: FxHashMap<String, KVItem>,
    map_file: Option<AutoIncreaseFileAccess>,
    map_file_append_position: u64,
    map_capacity_dirty: bool,
}

impl KVBox {
    pub(crate) fn open(path: PathBuf, name: String) -> Result<Self> {
        let basic = BasicBox::open(path, name)?;
        let map_file = AutoIncreaseFileAccess::open(
            basic.path().join(format!("{}-K2IdMap", basic.name())),
            4,
        )?;
        let mut box_ = Self {
            basic,
            map: FxHashMap::default(),
            map_file: Some(map_file),
            map_file_append_position: 0,
            map_capacity_dirty: false,
        };
        box_.initial_map()?;
        Ok(box_)
    }

    fn map_file(&self) -> Result<&AutoIncreaseFileAccess> {
        self.map_file
            .as_ref()
            .ok_or_else(|| GuiXuError::Corrupt("kv map file is closed".to_string()))
    }

    fn map_file_mut(&mut self) -> Result<&mut AutoIncreaseFileAccess> {
        self.map_file
            .as_mut()
            .ok_or_else(|| GuiXuError::Corrupt("kv map file is closed".to_string()))
    }

    fn initial_map(&mut self) -> Result<()> {
        let map_file = self.map_file()?;
        let length = map_file.length();
        if length < 4 {
            return Err(GuiXuError::Corrupt(
                "kv map file is shorter than 4 bytes".to_string(),
            ));
        }

        let capacity = map_file.read_i32(0)?.max(0) as usize;
        let mut map = FxHashMap::default();
        map.reserve(capacity);
        let mut cursor = 4;

        while cursor < length {
            let file_position = cursor;
            let key_length = map_file.read_i32(cursor)? as usize;
            cursor += 4;
            let value_type = map_file.read_u8(cursor)?;
            cursor += 1;
            let id = map_file.read_u64(cursor)?;
            cursor += 8;

            let mut key_bytes = vec![0; key_length];
            map_file.read_exact_at(cursor, &mut key_bytes)?;
            let key = String::from_utf8(key_bytes)
                .map_err(|error| GuiXuError::Corrupt(error.to_string()))?;
            cursor += key_length as u64;

            let value = if value_type == VALUE_TYPE_DELETE {
                KVValue::Deleted
            } else {
                let bytes = self.basic.get_byte(id)?;
                Self::byte_array_to_value(&bytes, value_type)?
            };

            map.insert(
                key,
                KVItem {
                    file_position,
                    id,
                    value_type,
                    value,
                },
            );
        }

        self.map_file_append_position = length;
        self.map = map;
        Ok(())
    }

    fn value_to_byte_array(value: &KVValue, value_type: u8) -> Result<Vec<u8>> {
        match (value_type, value) {
            (VALUE_TYPE_BOOLEAN, KVValue::Bool(value)) => Ok(vec![if *value { 1 } else { 0 }]),
            (VALUE_TYPE_BYTE, KVValue::Byte(value)) => Ok(vec![*value]),
            (VALUE_TYPE_INT, KVValue::Int(value)) => Ok(i32_to_bytes(*value).to_vec()),
            (VALUE_TYPE_LONG, KVValue::Long(value)) => Ok(u64_to_bytes(*value).to_vec()),
            (VALUE_TYPE_FLOAT, KVValue::Float(value)) => Ok(value.to_bits().to_be_bytes().to_vec()),
            (VALUE_TYPE_DOUBLE, KVValue::Double(value)) => {
                Ok(value.to_bits().to_be_bytes().to_vec())
            }
            (VALUE_TYPE_STRING, KVValue::String(value)) => Ok(value.as_bytes().to_vec()),
            (VALUE_TYPE_BYTE_ARRAY, KVValue::ByteArray(value)) => Ok(value.clone()),
            (VALUE_TYPE_INT_ARRAY, KVValue::IntArray(value)) => Ok(i32_slice_to_bytes(value)),
            (VALUE_TYPE_LONG_ARRAY, KVValue::LongArray(value)) => Ok(u64_slice_to_bytes(value)),
            (VALUE_TYPE_FLOAT_ARRAY, KVValue::FloatArray(value)) => Ok(f32_slice_to_bytes(value)),
            (VALUE_TYPE_DOUBLE_ARRAY, KVValue::DoubleArray(value)) => Ok(f64_slice_to_bytes(value)),
            _ => Err(GuiXuError::TypeError {
                stored: value.kind(),
                expected: value_type,
            }),
        }
    }

    fn byte_array_to_value(bytes: &[u8], value_type: u8) -> Result<KVValue> {
        match value_type {
            VALUE_TYPE_BOOLEAN => Ok(KVValue::Bool(bytes.first().copied() == Some(BOOLEAN_TRUE))),
            VALUE_TYPE_BYTE => Ok(KVValue::Byte(*bytes.first().unwrap_or(&0))),
            VALUE_TYPE_INT => Ok(KVValue::Int(bytes_to_i32(bytes))),
            VALUE_TYPE_LONG => Ok(KVValue::Long(bytes_to_u64(bytes))),
            VALUE_TYPE_FLOAT => Ok(KVValue::Float(f32::from_bits(bytes_to_i32(bytes) as u32))),
            VALUE_TYPE_DOUBLE => Ok(KVValue::Double(f64::from_bits(bytes_to_u64(bytes)))),
            VALUE_TYPE_STRING => Ok(KVValue::String(Cow::Owned(
                String::from_utf8(bytes.to_vec())
                    .map_err(|error| GuiXuError::Corrupt(error.to_string()))?,
            ))),
            VALUE_TYPE_BYTE_ARRAY => Ok(KVValue::ByteArray(bytes.to_vec())),
            VALUE_TYPE_INT_ARRAY => Ok(KVValue::IntArray(bytes_to_i32_vec(bytes))),
            VALUE_TYPE_LONG_ARRAY => Ok(KVValue::LongArray(bytes_to_u64_vec(bytes))),
            VALUE_TYPE_FLOAT_ARRAY => Ok(KVValue::FloatArray(bytes_to_f32_vec(bytes))),
            VALUE_TYPE_DOUBLE_ARRAY => Ok(KVValue::DoubleArray(bytes_to_f64_vec(bytes))),
            _ => Err(GuiXuError::TypeError {
                stored: value_type,
                expected: value_type,
            }),
        }
    }

    fn add_kv_item(
        &mut self,
        key: &str,
        id: u64,
        value_type: u8,
        value: KVValue,
    ) -> Result<KVItem> {
        let key_bytes = key.as_bytes();
        let position = self.map_file_append_position;
        self.map_file_append_position += KV_ITEM_FIX_LENGTH + key_bytes.len() as u64;

        let item = KVItem {
            file_position: position,
            id,
            value_type,
            value,
        };

        let mut buffer = Vec::with_capacity(KV_ITEM_FIX_LENGTH as usize + key_bytes.len());
        buffer.extend_from_slice(&i32_to_bytes(key_bytes.len() as i32));
        buffer.push(value_type);
        buffer.extend_from_slice(&u64_to_bytes(id));
        buffer.extend_from_slice(key_bytes);

        self.map_file_mut()?.write_all_at(position, &buffer)?;
        self.map_capacity_dirty = true;
        Ok(item)
    }

    fn update_kv_item(
        &mut self,
        file_position: u64,
        id: u64,
        old_value_type: u8,
        value_type: u8,
        value: KVValue,
    ) -> Result<KVItem> {
        if old_value_type != VALUE_TYPE_DELETE && old_value_type != value_type {
            return Err(GuiXuError::TypeError {
                stored: old_value_type,
                expected: value_type,
            });
        }
        self.map_file_mut()?
            .write_u8(file_position + 4, value_type)?;
        Ok(KVItem {
            file_position,
            id,
            value_type,
            value,
        })
    }

    fn put_data(&mut self, key: impl Into<String>, value_type: u8, value: KVValue) -> Result<()> {
        let key = key.into();
        let bytes = Self::value_to_byte_array(&value, value_type)?;
        if let Some(old) = self.map.get(&key) {
            let file_position = old.file_position;
            let id = old.id;
            let old_value_type = old.value_type;
            let item = self.update_kv_item(file_position, id, old_value_type, value_type, value)?;
            self.basic.append_store(id, &bytes)?;
            if let Some(old) = self.map.get_mut(&key) {
                *old = item;
            }
            return Ok(());
        } else {
            let id = self.basic.check_id_and_get(0)?;
            let item = self.add_kv_item(&key, id, value_type, value)?;
            self.basic.append_store(id, &bytes)?;
            self.map.insert(key, item);
            Ok(())
        }
    }

    fn get_data(&self, key: &str, expected_type: u8) -> Result<KVValue> {
        let item = self
            .map
            .get(key)
            .ok_or_else(|| GuiXuError::KeyNotFound(key.to_string()))?;
        if item.value_type == VALUE_TYPE_DELETE {
            return Err(GuiXuError::KeyNotFound(key.to_string()));
        }
        if item.value_type != expected_type {
            return Err(GuiXuError::TypeError {
                stored: item.value_type,
                expected: expected_type,
            });
        }
        Ok(item.value.clone())
    }

    pub fn remove(&mut self, key: &str) -> Result<()> {
        let Some(item) = self.map.get(key) else {
            return Ok(());
        };
        let file_position = item.file_position;
        let id = item.id;

        self.basic.remove_entry(id)?;
        self.map_file_mut()?
            .write_u8(file_position + 4, VALUE_TYPE_DELETE)?;
        if let Some(item) = self.map.get_mut(key) {
            *item = KVItem {
                file_position,
                id,
                value_type: VALUE_TYPE_DELETE,
                value: KVValue::Deleted,
            };
        }
        Ok(())
    }

    pub fn put_bool(&mut self, key: impl Into<String>, data: bool) -> Result<()> {
        self.put_data(key, VALUE_TYPE_BOOLEAN, KVValue::Bool(data))
    }

    pub fn put_byte(&mut self, key: impl Into<String>, data: u8) -> Result<()> {
        self.put_data(key, VALUE_TYPE_BYTE, KVValue::Byte(data))
    }

    pub fn put_int(&mut self, key: impl Into<String>, data: i32) -> Result<()> {
        self.put_data(key, VALUE_TYPE_INT, KVValue::Int(data))
    }

    pub fn put_long(&mut self, key: impl Into<String>, data: u64) -> Result<()> {
        self.put_data(key, VALUE_TYPE_LONG, KVValue::Long(data))
    }

    pub fn put_float(&mut self, key: impl Into<String>, data: f32) -> Result<()> {
        self.put_data(key, VALUE_TYPE_FLOAT, KVValue::Float(data))
    }

    pub fn put_double(&mut self, key: impl Into<String>, data: f64) -> Result<()> {
        self.put_data(key, VALUE_TYPE_DOUBLE, KVValue::Double(data))
    }

    pub fn put_string(
        &mut self,
        key: impl Into<String>,
        data: impl Into<Cow<'static, str>>,
    ) -> Result<()> {
        self.put_data(key, VALUE_TYPE_STRING, KVValue::String(data.into()))
    }

    pub fn put_byte_array(&mut self, key: impl Into<String>, data: Vec<u8>) -> Result<()> {
        self.put_data(key, VALUE_TYPE_BYTE_ARRAY, KVValue::ByteArray(data))
    }

    pub fn put_int_array(&mut self, key: impl Into<String>, data: Vec<i32>) -> Result<()> {
        self.put_data(key, VALUE_TYPE_INT_ARRAY, KVValue::IntArray(data))
    }

    pub fn put_long_array(&mut self, key: impl Into<String>, data: Vec<u64>) -> Result<()> {
        self.put_data(key, VALUE_TYPE_LONG_ARRAY, KVValue::LongArray(data))
    }

    pub fn put_float_array(&mut self, key: impl Into<String>, data: Vec<f32>) -> Result<()> {
        self.put_data(key, VALUE_TYPE_FLOAT_ARRAY, KVValue::FloatArray(data))
    }

    pub fn put_double_array(&mut self, key: impl Into<String>, data: Vec<f64>) -> Result<()> {
        self.put_data(key, VALUE_TYPE_DOUBLE_ARRAY, KVValue::DoubleArray(data))
    }

    pub fn get_bool(&self, key: &str) -> Result<bool> {
        match self.get_data(key, VALUE_TYPE_BOOLEAN)? {
            KVValue::Bool(value) => Ok(value),
            value => Err(type_error(value, VALUE_TYPE_BOOLEAN)),
        }
    }

    pub fn get_byte(&self, key: &str) -> Result<u8> {
        match self.get_data(key, VALUE_TYPE_BYTE)? {
            KVValue::Byte(value) => Ok(value),
            value => Err(type_error(value, VALUE_TYPE_BYTE)),
        }
    }

    pub fn get_int(&self, key: &str) -> Result<i32> {
        match self.get_data(key, VALUE_TYPE_INT)? {
            KVValue::Int(value) => Ok(value),
            value => Err(type_error(value, VALUE_TYPE_INT)),
        }
    }

    pub fn get_long(&self, key: &str) -> Result<u64> {
        match self.get_data(key, VALUE_TYPE_LONG)? {
            KVValue::Long(value) => Ok(value),
            value => Err(type_error(value, VALUE_TYPE_LONG)),
        }
    }

    pub fn get_float(&self, key: &str) -> Result<f32> {
        match self.get_data(key, VALUE_TYPE_FLOAT)? {
            KVValue::Float(value) => Ok(value),
            value => Err(type_error(value, VALUE_TYPE_FLOAT)),
        }
    }

    pub fn get_double(&self, key: &str) -> Result<f64> {
        match self.get_data(key, VALUE_TYPE_DOUBLE)? {
            KVValue::Double(value) => Ok(value),
            value => Err(type_error(value, VALUE_TYPE_DOUBLE)),
        }
    }

    pub fn get_string(&self, key: &str) -> Result<&str> {
        let item = self
            .map
            .get(key)
            .ok_or_else(|| GuiXuError::KeyNotFound(key.to_string()))?;
        if item.value_type == VALUE_TYPE_DELETE {
            return Err(GuiXuError::KeyNotFound(key.to_string()));
        }
        if item.value_type != VALUE_TYPE_STRING {
            return Err(GuiXuError::TypeError {
                stored: item.value_type,
                expected: VALUE_TYPE_STRING,
            });
        }
        match &item.value {
            KVValue::String(value) => Ok(value.as_ref()),
            value => Err(type_error(value.clone(), VALUE_TYPE_STRING)),
        }
    }

    pub fn get_byte_array(&self, key: &str) -> Result<Vec<u8>> {
        match self.get_data(key, VALUE_TYPE_BYTE_ARRAY)? {
            KVValue::ByteArray(value) => Ok(value),
            value => Err(type_error(value, VALUE_TYPE_BYTE_ARRAY)),
        }
    }

    pub fn get_int_array(&self, key: &str) -> Result<Vec<i32>> {
        match self.get_data(key, VALUE_TYPE_INT_ARRAY)? {
            KVValue::IntArray(value) => Ok(value),
            value => Err(type_error(value, VALUE_TYPE_INT_ARRAY)),
        }
    }

    pub fn get_long_array(&self, key: &str) -> Result<Vec<u64>> {
        match self.get_data(key, VALUE_TYPE_LONG_ARRAY)? {
            KVValue::LongArray(value) => Ok(value),
            value => Err(type_error(value, VALUE_TYPE_LONG_ARRAY)),
        }
    }

    pub fn get_float_array(&self, key: &str) -> Result<Vec<f32>> {
        match self.get_data(key, VALUE_TYPE_FLOAT_ARRAY)? {
            KVValue::FloatArray(value) => Ok(value),
            value => Err(type_error(value, VALUE_TYPE_FLOAT_ARRAY)),
        }
    }

    pub fn get_double_array(&self, key: &str) -> Result<Vec<f64>> {
        match self.get_data(key, VALUE_TYPE_DOUBLE_ARRAY)? {
            KVValue::DoubleArray(value) => Ok(value),
            value => Err(type_error(value, VALUE_TYPE_DOUBLE_ARRAY)),
        }
    }

    pub fn clear(&mut self, re_init: bool) -> Result<()> {
        let map_file = self.map_file.take();
        let map_path = map_file.as_ref().map(|file| file.path().to_path_buf());
        if let Some(map_file) = map_file {
            map_file.flush()?;
        }
        self.basic.clear(re_init)?;
        if let Some(map_path) = map_path {
            let _ = remove_file(map_path);
        }
        self.map.clear();
        self.map_file_append_position = 0;
        self.map_capacity_dirty = false;

        if re_init {
            let map_file = AutoIncreaseFileAccess::open(
                self.basic
                    .path()
                    .join(format!("{}-K2IdMap", self.basic.name())),
                4,
            )?;
            self.map_file = Some(map_file);
            self.initial_map()?;
        }
        Ok(())
    }

    pub fn compact(&mut self) -> Result<()> {
        self.basic.compact()
    }

    pub fn close(&mut self) -> Result<()> {
        if self.map_capacity_dirty {
            let len = self.map.len();
            if let Some(map_file) = self.map_file.as_mut() {
                map_file.write_i32(0, (len as i32) << 1)?;
            }
            self.map_capacity_dirty = false;
        }
        if let Some(map_file) = self.map_file.as_mut() {
            map_file.flush()?;
        }
        self.basic.close()
    }

    pub fn get_info(&self) -> BoxInfo {
        let mut info = self.basic.get_info();
        if let Some(map_file) = self.map_file.as_ref() {
            info.index_size += map_file.file_size();
        }
        info
    }
}

impl KVValue {
    fn kind(&self) -> u8 {
        match self {
            KVValue::Deleted => VALUE_TYPE_DELETE,
            KVValue::Bool(_) => VALUE_TYPE_BOOLEAN,
            KVValue::Byte(_) => VALUE_TYPE_BYTE,
            KVValue::Int(_) => VALUE_TYPE_INT,
            KVValue::Long(_) => VALUE_TYPE_LONG,
            KVValue::Float(_) => VALUE_TYPE_FLOAT,
            KVValue::Double(_) => VALUE_TYPE_DOUBLE,
            KVValue::String(_) => VALUE_TYPE_STRING,
            KVValue::ByteArray(_) => VALUE_TYPE_BYTE_ARRAY,
            KVValue::IntArray(_) => VALUE_TYPE_INT_ARRAY,
            KVValue::LongArray(_) => VALUE_TYPE_LONG_ARRAY,
            KVValue::FloatArray(_) => VALUE_TYPE_FLOAT_ARRAY,
            KVValue::DoubleArray(_) => VALUE_TYPE_DOUBLE_ARRAY,
        }
    }
}

fn type_error(value: KVValue, expected: u8) -> GuiXuError {
    GuiXuError::TypeError {
        stored: value.kind(),
        expected,
    }
}

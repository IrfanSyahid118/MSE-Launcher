use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use launcher_types::LauncherErrorPayload;
use std::collections::HashMap;
use std::io::{Read, Write};

#[derive(Debug, Clone, PartialEq)]
pub enum NbtValue {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List { item_type: u8, items: Vec<NbtValue> },
    Compound(HashMap<String, NbtTag>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NbtTag {
    pub tag_type: u8,
    pub value: NbtValue,
}

pub struct NbtReader<'a> {
    buf: &'a [u8],
    offset: usize,
}

impl<'a> NbtReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, offset: 0 }
    }

    fn read_byte(&mut self) -> Result<i8, LauncherErrorPayload> {
        let b = *self
            .buf
            .get(self.offset)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT buffer overflow"))?
            as i8;
        self.offset = self
            .offset
            .checked_add(1)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT offset overflow"))?;
        Ok(b)
    }

    fn read_u8(&mut self) -> Result<u8, LauncherErrorPayload> {
        Ok(self.read_byte()? as u8)
    }

    fn read_short(&mut self) -> Result<i16, LauncherErrorPayload> {
        let end = self
            .offset
            .checked_add(2)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT offset overflow"))?;
        let slice = self
            .buf
            .get(self.offset..end)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT buffer overflow"))?;
        let bytes: [u8; 2] = slice
            .try_into()
            .map_err(|_| LauncherErrorPayload::file_system("NBT buffer conversion error"))?;
        self.offset = end;
        Ok(i16::from_be_bytes(bytes))
    }

    fn read_u16(&mut self) -> Result<u16, LauncherErrorPayload> {
        Ok(self.read_short()? as u16)
    }

    fn read_int(&mut self) -> Result<i32, LauncherErrorPayload> {
        let end = self
            .offset
            .checked_add(4)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT offset overflow"))?;
        let slice = self
            .buf
            .get(self.offset..end)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT buffer overflow"))?;
        let bytes: [u8; 4] = slice
            .try_into()
            .map_err(|_| LauncherErrorPayload::file_system("NBT buffer conversion error"))?;
        self.offset = end;
        Ok(i32::from_be_bytes(bytes))
    }

    fn read_long(&mut self) -> Result<i64, LauncherErrorPayload> {
        let end = self
            .offset
            .checked_add(8)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT offset overflow"))?;
        let slice = self
            .buf
            .get(self.offset..end)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT buffer overflow"))?;
        let bytes: [u8; 8] = slice
            .try_into()
            .map_err(|_| LauncherErrorPayload::file_system("NBT buffer conversion error"))?;
        self.offset = end;
        Ok(i64::from_be_bytes(bytes))
    }

    fn read_float(&mut self) -> Result<f32, LauncherErrorPayload> {
        let val = self.read_int()?;
        Ok(f32::from_bits(val as u32))
    }

    fn read_double(&mut self) -> Result<f64, LauncherErrorPayload> {
        let val = self.read_long()?;
        Ok(f64::from_bits(val as u64))
    }

    fn read_string(&mut self) -> Result<String, LauncherErrorPayload> {
        let len = self.read_u16()? as usize;
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT string offset overflow"))?;
        let slice = self
            .buf
            .get(self.offset..end)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT string buffer overflow"))?;
        let s = String::from_utf8_lossy(slice).to_string();
        self.offset = end;
        Ok(s)
    }

    fn read_byte_array(&mut self) -> Result<Vec<u8>, LauncherErrorPayload> {
        let raw_len = self.read_int()?;
        if raw_len < 0 {
            return Err(LauncherErrorPayload::file_system(
                "NBT byte array negative length",
            ));
        }
        let len = raw_len as usize;
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT byte array overflow"))?;
        let slice = self
            .buf
            .get(self.offset..end)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT byte array overflow"))?
            .to_vec();
        self.offset = end;
        Ok(slice)
    }

    fn read_int_array(&mut self) -> Result<Vec<i32>, LauncherErrorPayload> {
        let raw_len = self.read_int()?;
        if raw_len < 0 {
            return Err(LauncherErrorPayload::file_system(
                "NBT int array negative length",
            ));
        }
        let len = raw_len as usize;
        let needed_bytes = len
            .checked_mul(4)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT int array overflow"))?;
        if self
            .offset
            .checked_add(needed_bytes)
            .is_none_or(|end| end > self.buf.len())
        {
            return Err(LauncherErrorPayload::file_system(
                "NBT int array buffer overflow",
            ));
        }
        let mut arr = Vec::with_capacity(len.min(100_000));
        for _ in 0..len {
            arr.push(self.read_int()?);
        }
        Ok(arr)
    }

    fn read_long_array(&mut self) -> Result<Vec<i64>, LauncherErrorPayload> {
        let raw_len = self.read_int()?;
        if raw_len < 0 {
            return Err(LauncherErrorPayload::file_system(
                "NBT long array negative length",
            ));
        }
        let len = raw_len as usize;
        let needed_bytes = len
            .checked_mul(8)
            .ok_or_else(|| LauncherErrorPayload::file_system("NBT long array overflow"))?;
        if self
            .offset
            .checked_add(needed_bytes)
            .is_none_or(|end| end > self.buf.len())
        {
            return Err(LauncherErrorPayload::file_system(
                "NBT long array buffer overflow",
            ));
        }
        let mut arr = Vec::with_capacity(len.min(100_000));
        for _ in 0..len {
            arr.push(self.read_long()?);
        }
        Ok(arr)
    }

    fn read_payload_with_depth(
        &mut self,
        tag_type: u8,
        depth: usize,
    ) -> Result<NbtValue, LauncherErrorPayload> {
        if depth > 512 {
            return Err(LauncherErrorPayload::file_system(
                "NBT nesting depth exceeded 512 levels",
            ));
        }

        match tag_type {
            1 => Ok(NbtValue::Byte(self.read_byte()?)),
            2 => Ok(NbtValue::Short(self.read_short()?)),
            3 => Ok(NbtValue::Int(self.read_int()?)),
            4 => Ok(NbtValue::Long(self.read_long()?)),
            5 => Ok(NbtValue::Float(self.read_float()?)),
            6 => Ok(NbtValue::Double(self.read_double()?)),
            7 => Ok(NbtValue::ByteArray(self.read_byte_array()?)),
            8 => Ok(NbtValue::String(self.read_string()?)),
            9 => {
                let item_type = self.read_u8()?;
                let raw_len = self.read_int()?;
                if raw_len < 0 {
                    return Err(LauncherErrorPayload::file_system(
                        "NBT list negative length",
                    ));
                }
                let len = raw_len as usize;
                if self
                    .offset
                    .checked_add(len)
                    .is_none_or(|end| end > self.buf.len())
                {
                    return Err(LauncherErrorPayload::file_system(
                        "NBT list length exceeds remaining buffer",
                    ));
                }
                let mut items = Vec::with_capacity(len.min(100_000));
                for _ in 0..len {
                    items.push(self.read_payload_with_depth(item_type, depth + 1)?);
                }
                Ok(NbtValue::List { item_type, items })
            }
            10 => Ok(NbtValue::Compound(
                self.read_compound_with_depth(depth + 1)?,
            )),
            11 => Ok(NbtValue::IntArray(self.read_int_array()?)),
            12 => Ok(NbtValue::LongArray(self.read_long_array()?)),
            other => Err(LauncherErrorPayload::file_system(format!(
                "Tipe tag NBT tidak dikenal: {}",
                other
            ))),
        }
    }

    pub fn read_compound(&mut self) -> Result<HashMap<String, NbtTag>, LauncherErrorPayload> {
        self.read_compound_with_depth(0)
    }

    pub fn read_compound_with_depth(
        &mut self,
        depth: usize,
    ) -> Result<HashMap<String, NbtTag>, LauncherErrorPayload> {
        if depth > 512 {
            return Err(LauncherErrorPayload::file_system(
                "NBT nesting depth exceeded 512 levels",
            ));
        }

        let mut map = HashMap::new();
        loop {
            let tag_type = self.read_u8()?;
            if tag_type == 0 {
                // TAG_End
                break;
            }
            let name = self.read_string()?;
            let value = self.read_payload_with_depth(tag_type, depth)?;
            map.insert(name, NbtTag { tag_type, value });
        }
        Ok(map)
    }

    pub fn read_root(&mut self) -> Result<(String, HashMap<String, NbtTag>), LauncherErrorPayload> {
        let root_type = self.read_u8()?;
        if root_type != 10 {
            return Err(LauncherErrorPayload::file_system(
                "NBT root harus bertipe Compound (10)",
            ));
        }
        let root_name = self.read_string()?;
        let compound = self.read_compound()?;
        Ok((root_name, compound))
    }
}

pub struct NbtWriter {
    out: Vec<u8>,
}

impl NbtWriter {
    pub fn new() -> Self {
        Self { out: Vec::new() }
    }

    fn write_byte(&mut self, val: i8) {
        self.out.push(val as u8);
    }

    fn write_u8(&mut self, val: u8) {
        self.out.push(val);
    }

    fn write_short(&mut self, val: i16) {
        self.out.extend_from_slice(&val.to_be_bytes());
    }

    fn write_u16(&mut self, val: u16) {
        self.out.extend_from_slice(&val.to_be_bytes());
    }

    fn write_int(&mut self, val: i32) {
        self.out.extend_from_slice(&val.to_be_bytes());
    }

    fn write_long(&mut self, val: i64) {
        self.out.extend_from_slice(&val.to_be_bytes());
    }

    fn write_float(&mut self, val: f32) {
        self.out.extend_from_slice(&val.to_bits().to_be_bytes());
    }

    fn write_double(&mut self, val: f64) {
        self.out.extend_from_slice(&val.to_bits().to_be_bytes());
    }

    fn write_string(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let len = bytes.len().min(u16::MAX as usize);
        self.write_u16(len as u16);
        if let Some(slice) = bytes.get(..len) {
            self.out.extend_from_slice(slice);
        }
    }

    fn write_payload(&mut self, tag: &NbtTag) -> Result<(), LauncherErrorPayload> {
        match &tag.value {
            NbtValue::Byte(b) => self.write_byte(*b),
            NbtValue::Short(s) => self.write_short(*s),
            NbtValue::Int(i) => self.write_int(*i),
            NbtValue::Long(l) => self.write_long(*l),
            NbtValue::Float(f) => self.write_float(*f),
            NbtValue::Double(d) => self.write_double(*d),
            NbtValue::ByteArray(arr) => {
                let len = arr.len().min(i32::MAX as usize);
                self.write_int(len as i32);
                if let Some(slice) = arr.get(..len) {
                    self.out.extend_from_slice(slice);
                }
            }
            NbtValue::String(s) => self.write_string(s),
            NbtValue::List { item_type, items } => {
                self.write_u8(*item_type);
                let len = items.len().min(i32::MAX as usize);
                self.write_int(len as i32);
                for it in items.iter().take(len) {
                    self.write_payload(&NbtTag {
                        tag_type: *item_type,
                        value: it.clone(),
                    })?;
                }
            }
            NbtValue::Compound(c) => self.write_compound(c)?,
            NbtValue::IntArray(arr) => {
                let len = arr.len().min(i32::MAX as usize);
                self.write_int(len as i32);
                for i in arr.iter().take(len) {
                    self.write_int(*i);
                }
            }
            NbtValue::LongArray(arr) => {
                let len = arr.len().min(i32::MAX as usize);
                self.write_int(len as i32);
                for l in arr.iter().take(len) {
                    self.write_long(*l);
                }
            }
        }
        Ok(())
    }

    pub fn write_compound(
        &mut self,
        compound: &HashMap<String, NbtTag>,
    ) -> Result<(), LauncherErrorPayload> {
        for (name, tag) in compound {
            self.write_u8(tag.tag_type);
            self.write_string(name);
            self.write_payload(tag)?;
        }
        self.write_u8(0); // TAG_End
        Ok(())
    }

    pub fn write_root(
        &mut self,
        name: &str,
        compound: &HashMap<String, NbtTag>,
    ) -> Result<Vec<u8>, LauncherErrorPayload> {
        self.write_u8(10); // TAG_Compound
        self.write_string(name);
        self.write_compound(compound)?;
        Ok(std::mem::take(&mut self.out))
    }
}

impl Default for NbtWriter {
    fn default() -> Self {
        Self::new()
    }
}

pub const MAX_NBT_UNCOMPRESSED_SIZE: u64 = 32 * 1024 * 1024; // 32 MB max (SC-10 Gzip bomb protection)

pub fn read_nbt_buffer(
    raw: &[u8],
) -> Result<(String, HashMap<String, NbtTag>), LauncherErrorPayload> {
    if raw.starts_with(&[0x1f, 0x8b]) {
        let decoder = GzDecoder::new(raw);
        let mut decompressed = Vec::new();
        decoder
            .take(MAX_NBT_UNCOMPRESSED_SIZE + 1)
            .read_to_end(&mut decompressed)
            .map_err(|e| {
                LauncherErrorPayload::file_system(format!("Gagal dekompresi gzip NBT: {}", e))
            })?;

        if decompressed.len() as u64 > MAX_NBT_UNCOMPRESSED_SIZE {
            return Err(LauncherErrorPayload::file_system(
                "Ukuran NBT terdekompresi melebihi batas keamanan 32 MB (Gzip bomb protection SC-10).",
            ));
        }

        let mut reader = NbtReader::new(&decompressed);
        reader.read_root()
    } else {
        let mut reader = NbtReader::new(raw);
        reader.read_root()
    }
}

pub fn write_nbt_buffer(
    name: &str,
    compound: &HashMap<String, NbtTag>,
    compress: bool,
) -> Result<Vec<u8>, LauncherErrorPayload> {
    let mut writer = NbtWriter::new();
    let uncompressed = writer.write_root(name, compound)?;
    if compress {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&uncompressed).map_err(|e| {
            LauncherErrorPayload::file_system(format!("Gagal kompresi gzip NBT: {}", e))
        })?;
        encoder.finish().map_err(|e| {
            LauncherErrorPayload::file_system(format!("Gagal menyelesaikan kompresi NBT: {}", e))
        })
    } else {
        Ok(uncompressed)
    }
}

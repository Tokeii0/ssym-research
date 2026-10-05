//! Experimental SSYM v1: indexed, lossless storage of StarMem's KernelProfile.
//! This is a compiled Windows layout projection, not the full ISF type graph.
//! All integers are little endian; offsets refer to the file, never native pointers.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};

use super::{FieldProfile, KernelProfile, TypeLayout, build_eprocess_profile};

pub const MAGIC: &[u8; 8] = b"SSYM\r\n\x1a\n";
pub const FORMAT_VERSION: u32 = 1;
const HEADER: usize = 192;
const TYPE: usize = 24;
const FIELD: usize = 32;
const SYMBOL: usize = 16;
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Invalidates compiled profiles whenever the PDB projection recipe changes.
pub fn recipe_sha256() -> [u8; 32] {
    Sha256::digest(include_bytes!("../profile.rs")).into()
}

pub fn file_sha256(path: &Path) -> Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest.finalize().into())
}

pub fn is_compiled_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ssym"))
}

pub fn compile_pdb(path: &Path) -> Result<Vec<u8>> {
    // Verify a stable source across compilation. The digest is provenance, not a signature.
    let before = file_sha256(path)?;
    let profile = build_eprocess_profile(path)?;
    let original_age = pdb_original_age(path)?;
    ensure!(before == file_sha256(path)?, "编译期间 PDB 内容发生变化");
    encode(
        &profile,
        path.file_name()
            .and_then(|name| name.to_str())
            .context("PDB 文件名不是 UTF-8")?,
        before,
        original_age,
    )
}

/// DBI retains the linker's age; post-processing may increase PDB info age.
/// Preserve both instead of accepting an arbitrary older image's age.
pub fn pdb_original_age(path: &Path) -> Result<u32> {
    let mut pdb = pdb::PDB::open(File::open(path)?)?;
    pdb.debug_information()?
        .age()
        .context("SSYM v1 要求 PDB 包含 DBI 原始 Age")
}

#[derive(Default)]
struct Strings<'a> {
    offsets: BTreeMap<&'a str, u32>,
    bytes: Vec<u8>,
}

impl<'a> Strings<'a> {
    fn put(&mut self, output: &mut [u8], position: usize, value: &'a str) -> Result<()> {
        let len = u32::try_from(value.len())?;
        let offset = match self.offsets.get(value) {
            Some(offset) => *offset,
            None => {
                let offset = u32::try_from(self.bytes.len())?;
                ensure!(
                    self.bytes
                        .len()
                        .checked_add(value.len())
                        .is_some_and(|n| n <= MAX_BYTES as usize),
                    "SSYM 字符串表过大"
                );
                self.bytes.extend_from_slice(value.as_bytes());
                self.offsets.insert(value, offset);
                offset
            }
        };
        put32(output, position, offset);
        put32(output, position + 4, len);
        Ok(())
    }
}

/// Encodes every member of the input profile, including the separate root field map.
pub fn encode(
    profile: &KernelProfile,
    pdb_name: &str,
    pdb_sha256: [u8; 32],
    original_age: u32,
) -> Result<Vec<u8>> {
    let fields = profile
        .types
        .values()
        .try_fold(profile.fields.len(), |count, ty| {
            count
                .checked_add(ty.fields.len())
                .context("SSYM 字段数溢出")
        })?;
    let type_end = table_end(HEADER, profile.types.len(), TYPE)?;
    let field_end = table_end(type_end, fields, FIELD)?;
    let string_start = table_end(field_end, profile.symbols.len(), SYMBOL)?;
    let mut bytes = vec![0; string_start];
    bytes[..8].copy_from_slice(MAGIC);
    put32(&mut bytes, 8, FORMAT_VERSION);
    put32(&mut bytes, 12, HEADER as u32);
    put32(&mut bytes, 24, profile.schema_version);
    put32(&mut bytes, 28, profile.pdb_age);
    put64(&mut bytes, 48, profile.type_size);
    put32(&mut bytes, 56, u32::try_from(profile.fields.len())?);
    put32(&mut bytes, 60, u32::try_from(profile.types.len())?);
    put32(&mut bytes, 64, u32::try_from(fields)?);
    put32(&mut bytes, 68, u32::try_from(profile.symbols.len())?);
    bytes[80..112].copy_from_slice(&pdb_sha256);
    bytes[112..144].copy_from_slice(&recipe_sha256());
    put32(&mut bytes, 184, original_age);
    let mut strings = Strings::default();
    strings.put(&mut bytes, 32, &profile.pdb_guid)?;
    strings.put(&mut bytes, 40, &profile.type_name)?;
    strings.put(&mut bytes, 176, pdb_name)?;
    for (index, (name, field)) in profile
        .fields
        .iter()
        .chain(profile.types.values().flat_map(|ty| ty.fields.iter()))
        .enumerate()
    {
        let pos = type_end + index * FIELD;
        strings.put(&mut bytes, pos, name)?;
        put64(&mut bytes, pos + 8, field.offset);
        put64(&mut bytes, pos + 16, field.size.unwrap_or(0));
        put32(
            &mut bytes,
            pos + 24,
            u32::from(field.size.is_some())
                | (u32::from(field.bit_position.is_some()) << 1)
                | (u32::from(field.bit_length.is_some()) << 2),
        );
        bytes[pos + 28] = field.bit_position.unwrap_or(0);
        bytes[pos + 29] = field.bit_length.unwrap_or(0);
    }
    let mut first = profile.fields.len();
    for (index, (name, ty)) in profile.types.iter().enumerate() {
        let pos = HEADER + index * TYPE;
        strings.put(&mut bytes, pos, name)?;
        put64(&mut bytes, pos + 8, ty.size);
        put32(&mut bytes, pos + 16, u32::try_from(first)?);
        put32(&mut bytes, pos + 20, u32::try_from(ty.fields.len())?);
        first += ty.fields.len();
    }
    for (index, (name, rva)) in profile.symbols.iter().enumerate() {
        let pos = field_end + index * SYMBOL;
        strings.put(&mut bytes, pos, name)?;
        put32(&mut bytes, pos + 8, *rva);
    }
    put32(&mut bytes, 72, u32::try_from(strings.bytes.len())?);
    bytes.extend_from_slice(&strings.bytes);
    ensure!(
        bytes.len() <= MAX_BYTES as usize,
        "SSYM 文件超过 64 MiB 上限"
    );
    let length = bytes.len() as u64;
    put64(&mut bytes, 16, length);
    let checksum = checksum(&bytes);
    bytes[144..176].copy_from_slice(&checksum);
    // The same validator protects locally produced and externally supplied files.
    CompiledProfile::parse(&bytes)?;
    Ok(bytes)
}

/// Owns one immutable buffer. No String/BTreeMap expansion is needed for queries.
pub struct CompiledProfileFile {
    bytes: Vec<u8>,
    sections: Sections,
}

impl CompiledProfileFile {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("无法打开 SSYM {}", path.display()))?;
        ensure!(
            file.metadata()?.len() <= MAX_BYTES,
            "SSYM 文件超过 64 MiB 上限"
        );
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        let sections = CompiledProfile::parse(&bytes)?.sections;
        Ok(Self { bytes, sections })
    }

    pub fn view(&self) -> CompiledProfile<'_> {
        CompiledProfile {
            bytes: &self.bytes,
            sections: self.sections,
        }
    }
}

#[derive(Clone, Copy)]
struct Sections {
    fields: usize,
    symbols: usize,
    strings: usize,
    type_count: usize,
    field_count: usize,
    symbol_count: usize,
    root_count: usize,
}

#[derive(Clone, Copy)]
pub struct CompiledProfile<'a> {
    bytes: &'a [u8],
    sections: Sections,
}

impl<'a> CompiledProfile<'a> {
    /// Full bounds, UTF-8, ordering, version and checksum validation, once per buffer.
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        ensure!(
            (HEADER..=MAX_BYTES as usize).contains(&bytes.len()),
            "SSYM 长度无效"
        );
        ensure!(&bytes[..8] == MAGIC, "SSYM 签名无效");
        ensure!(
            get32(bytes, 8) == FORMAT_VERSION && get32(bytes, 12) == HEADER as u32,
            "不支持的 SSYM 版本"
        );
        ensure!(
            get64(bytes, 16) == bytes.len() as u64,
            "SSYM 文件长度不匹配"
        );
        ensure!(
            get32(bytes, 76) == 0 && bytes[188..192] == [0; 4],
            "SSYM 保留标志非零"
        );
        ensure!(bytes[144..176] == checksum(bytes), "SSYM SHA-256 校验失败");
        ensure!(
            get32(bytes, 184) > 0 && get32(bytes, 184) <= get32(bytes, 28),
            "SSYM DBI Age 无效"
        );
        let root_count = get32(bytes, 56) as usize;
        let type_count = get32(bytes, 60) as usize;
        let field_count = get32(bytes, 64) as usize;
        let symbol_count = get32(bytes, 68) as usize;
        let fields = table_end(HEADER, type_count, TYPE)?;
        let symbols = table_end(fields, field_count, FIELD)?;
        let strings = table_end(symbols, symbol_count, SYMBOL)?;
        ensure!(
            strings.checked_add(get32(bytes, 72) as usize) == Some(bytes.len()),
            "SSYM 表范围无效"
        );
        ensure!(root_count <= field_count, "SSYM 根字段越界");
        let view = Self {
            bytes,
            sections: Sections {
                fields,
                symbols,
                strings,
                type_count,
                field_count,
                symbol_count,
                root_count,
            },
        };
        ensure!(
            normalize_guid(view.checked_name(32)?)?.len() == 32,
            "SSYM GUID 无效"
        );
        view.checked_name(40)?;
        let name = view.checked_name(176)?;
        ensure!(
            !name.is_empty() && !name.contains(['/', '\\', ':', '\0']),
            "SSYM PDB 文件名无效"
        );
        view.validate_sorted(HEADER, type_count, TYPE)?;
        view.validate_sorted(symbols, symbol_count, SYMBOL)?;
        view.validate_sorted(fields, root_count, FIELD)?;
        let mut next = root_count;
        for index in 0..type_count {
            let pos = HEADER + index * TYPE;
            let first = get32(bytes, pos + 16) as usize;
            let count = get32(bytes, pos + 20) as usize;
            ensure!(first == next, "SSYM 字段归属不连续或重叠");
            next = first.checked_add(count).context("SSYM 字段数溢出")?;
            ensure!(next <= field_count, "SSYM 类型字段越界");
            view.validate_sorted(fields + first * FIELD, count, FIELD)?;
        }
        ensure!(next == field_count, "SSYM 存在无归属字段");
        for index in 0..field_count {
            let pos = fields + index * FIELD;
            let flags = get32(bytes, pos + 24);
            ensure!(
                flags & !7 == 0 && bytes[pos + 30..pos + 32] == [0; 2],
                "SSYM 字段标志无效"
            );
            ensure!(
                flags & 1 != 0 || get64(bytes, pos + 16) == 0,
                "SSYM 缺省字段大小无效"
            );
            ensure!(
                flags & 2 != 0 || bytes[pos + 28] == 0,
                "SSYM 缺省位偏移无效"
            );
            ensure!(
                flags & 4 != 0 || bytes[pos + 29] == 0,
                "SSYM 缺省位长度无效"
            );
        }
        for index in 0..symbol_count {
            ensure!(
                get32(bytes, symbols + index * SYMBOL + 12) == 0,
                "SSYM 符号保留字段非零"
            );
        }
        Ok(view)
    }

    pub fn pdb_guid(&self) -> &'a str {
        self.name(32)
    }
    pub fn pdb_age(&self) -> u32 {
        get32(self.bytes, 28)
    }
    pub fn original_age(&self) -> u32 {
        get32(self.bytes, 184)
    }
    pub fn pdb_name(&self) -> &'a str {
        self.name(176)
    }
    pub fn pdb_sha256(&self) -> &'a [u8] {
        &self.bytes[80..112]
    }
    pub fn type_count(&self) -> usize {
        self.sections.type_count
    }
    pub fn field_count(&self) -> usize {
        self.sections.field_count
    }
    pub fn symbol_count(&self) -> usize {
        self.sections.symbol_count
    }

    pub fn require_current_recipe(&self) -> Result<()> {
        ensure!(
            self.bytes[112..144] == recipe_sha256(),
            "SSYM 的 PDB 提取版本已变化，请从原 PDB 重新生成"
        );
        Ok(())
    }

    pub fn validate_identity(&self, name: &str, guid: &str, age: u32) -> Result<()> {
        ensure!(
            self.pdb_name().eq_ignore_ascii_case(name)
                && normalize_guid(self.pdb_guid())? == normalize_guid(guid)?
                && self.original_age() == age,
            "SSYM PDB 名称/GUID/Age 与目标内核不匹配"
        );
        Ok(())
    }

    pub fn field(&self, name: &str) -> Option<FieldProfile> {
        self.find(self.sections.fields, self.sections.root_count, FIELD, name)
            .map(|pos| self.field_at(pos))
    }

    pub fn type_size(&self, name: &str) -> Option<u64> {
        self.find(HEADER, self.sections.type_count, TYPE, name)
            .map(|pos| get64(self.bytes, pos + 8))
    }

    pub fn type_field(&self, type_name: &str, field_name: &str) -> Option<FieldProfile> {
        let pos = self.find(HEADER, self.sections.type_count, TYPE, type_name)?;
        let first = get32(self.bytes, pos + 16) as usize;
        let count = get32(self.bytes, pos + 20) as usize;
        self.find(
            self.sections.fields + first * FIELD,
            count,
            FIELD,
            field_name,
        )
        .map(|pos| self.field_at(pos))
    }

    pub fn symbol_rva(&self, name: &str) -> Option<u32> {
        self.find(
            self.sections.symbols,
            self.sections.symbol_count,
            SYMBOL,
            name,
        )
        .map(|pos| get32(self.bytes, pos + 8))
    }

    /// Compatibility boundary for current StarMem plugins. This allocates the ordinary maps.
    pub fn to_profile(&self) -> KernelProfile {
        let types = (0..self.sections.type_count)
            .map(|index| {
                let pos = HEADER + index * TYPE;
                (
                    self.name(pos).to_owned(),
                    TypeLayout {
                        size: get64(self.bytes, pos + 8),
                        fields: self.fields(
                            get32(self.bytes, pos + 16) as usize,
                            get32(self.bytes, pos + 20) as usize,
                        ),
                    },
                )
            })
            .collect();
        let symbols = (0..self.sections.symbol_count)
            .map(|index| {
                let pos = self.sections.symbols + index * SYMBOL;
                (self.name(pos).to_owned(), get32(self.bytes, pos + 8))
            })
            .collect();
        KernelProfile {
            schema_version: get32(self.bytes, 24),
            pdb_guid: self.pdb_guid().to_owned(),
            pdb_age: self.pdb_age(),
            type_name: self.name(40).to_owned(),
            type_size: get64(self.bytes, 48),
            fields: self.fields(0, self.sections.root_count),
            types,
            symbols,
        }
    }

    fn fields(&self, first: usize, count: usize) -> BTreeMap<String, FieldProfile> {
        (first..first + count)
            .map(|index| {
                let pos = self.sections.fields + index * FIELD;
                (self.name(pos).to_owned(), self.field_at(pos))
            })
            .collect()
    }

    fn field_at(&self, pos: usize) -> FieldProfile {
        let flags = get32(self.bytes, pos + 24);
        FieldProfile {
            offset: get64(self.bytes, pos + 8),
            size: (flags & 1 != 0).then(|| get64(self.bytes, pos + 16)),
            bit_position: (flags & 2 != 0).then_some(self.bytes[pos + 28]),
            bit_length: (flags & 4 != 0).then_some(self.bytes[pos + 29]),
        }
    }

    fn checked_name(&self, pos: usize) -> Result<&'a str> {
        let start = self
            .sections
            .strings
            .checked_add(get32(self.bytes, pos) as usize)
            .context("SSYM 字符串偏移溢出")?;
        let end = start
            .checked_add(get32(self.bytes, pos + 4) as usize)
            .context("SSYM 字符串长度溢出")?;
        std::str::from_utf8(self.bytes.get(start..end).context("SSYM 字符串越界")?)
            .context("SSYM 字符串不是 UTF-8")
    }

    fn name(&self, pos: usize) -> &'a str {
        // All references were checked before the immutable view was constructed.
        self.checked_name(pos).expect("validated SSYM string")
    }

    fn validate_sorted(&self, start: usize, count: usize, width: usize) -> Result<()> {
        let mut previous = None;
        for index in 0..count {
            let name = self.checked_name(start + index * width)?;
            ensure!(
                previous.is_none_or(|prev: &str| prev < name),
                "SSYM 名称索引重复或未排序"
            );
            previous = Some(name);
        }
        Ok(())
    }

    fn find(&self, start: usize, count: usize, width: usize, name: &str) -> Option<usize> {
        let (mut low, mut high) = (0, count);
        while low < high {
            let mid = low + (high - low) / 2;
            let pos = start + mid * width;
            match self.name(pos).cmp(name) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => return Some(pos),
            }
        }
        None
    }
}

fn normalize_guid(guid: &str) -> Result<String> {
    let normalized = guid.replace('-', "").to_ascii_uppercase();
    ensure!(
        normalized.len() == 32 && normalized.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "PDB GUID 无效"
    );
    Ok(normalized)
}

fn table_end(start: usize, count: usize, width: usize) -> Result<usize> {
    let end = count
        .checked_mul(width)
        .and_then(|size| start.checked_add(size))
        .context("SSYM 表长度溢出")?;
    ensure!(end <= MAX_BYTES as usize, "SSYM 表超过大小上限");
    Ok(end)
}

fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(&bytes[..144]);
    digest.update(&bytes[176..]);
    digest.finalize().into()
}

fn get32(bytes: &[u8], pos: usize) -> u32 {
    u32::from_le_bytes(bytes[pos..pos + 4].try_into().expect("validated SSYM u32"))
}
fn get64(bytes: &[u8], pos: usize) -> u64 {
    u64::from_le_bytes(bytes[pos..pos + 8].try_into().expect("validated SSYM u64"))
}
fn put32(bytes: &mut [u8], pos: usize, value: u32) {
    bytes[pos..pos + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(bytes: &mut [u8], pos: usize, value: u64) {
    bytes[pos..pos + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests;

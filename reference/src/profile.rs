use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use pdb::{FallibleIterator, TypeData, TypeFinder, TypeIndex};
use serde::{Deserialize, Serialize};

pub mod compiled;

const PROFILE_TYPES: &[&str] = &[
    "_EPROCESS",
    "_KPROCESS",
    "_KPCR",
    "_KPRCB",
    "_KTHREAD",
    "_ETHREAD",
    "_CLIENT_ID",
    "_KAPC_STATE",
    "_KTRAP_FRAME",
    "_KPROCESSOR_STATE",
    "_KSPECIAL_REGISTERS",
    "_CONTEXT",
    "_PEB",
    "_PEB_LDR_DATA",
    "_TEB",
    "_NT_TIB",
    "_RTL_USER_PROCESS_PARAMETERS",
    "_CURDIR",
    "_UNICODE_STRING",
    "_TOKEN",
    "_TOKEN_SOURCE",
    "_LUID",
    "_SEP_LOGON_SESSION_REFERENCES",
    "_SEP_TOKEN_PRIVILEGES",
    "_SID_AND_ATTRIBUTES",
    "_SID",
    "_MM_SESSION_SPACE",
    "_EJOB",
    "_JOB_ACCESS_STATE",
    "_DISPATCHER_HEADER",
    "_KWAIT_BLOCK",
    "_PS_PROTECTION",
    "_SE_AUDIT_PROCESS_CREATION_INFO",
    "_OBJECT_NAME_INFORMATION",
    "_KUSER_SHARED_DATA",
    "_KSYSTEM_TIME",
    "_RTL_AVL_TREE",
    "_RTL_BALANCED_NODE",
    "_RTL_BITMAP",
    "_RTL_DYNAMIC_HASH_TABLE",
    "_RTL_DYNAMIC_HASH_TABLE_ENTRY",
    "_MMVAD_SHORT",
    "_MMVAD",
    "_MMVAD_FLAGS",
    "_MMVAD_FLAGS1",
    "_MMVAD_FLAGS2",
    "_SUBSECTION",
    "_CONTROL_AREA",
    "_EX_FAST_REF",
    "_FILE_OBJECT",
    "_FSRTL_ADVANCED_FCB_HEADER",
    "_FSRTL_COMMON_FCB_HEADER",
    "_DEVICE_OBJECT",
    "_DRIVER_OBJECT",
    "_FAST_IO_DISPATCH",
    "_DRIVER_EXTENSION",
    "_OBJECT_SYMBOLIC_LINK",
    "_KMUTANT",
    "_KTIMER",
    "_KTIMER_TABLE",
    "_KTIMER_TABLE_ENTRY",
    "_KDPC",
    "_EX_TIMER",
    "_SECTION_OBJECT_POINTERS",
    "_SHARED_CACHE_MAP",
    "_VACB",
    "_SEGMENT",
    "_MMPTE",
    "_MMPTE_HARDWARE",
    "_MMPTE_SOFTWARE",
    "_MMPTE_TRANSITION",
    "_MI_SYSTEM_INFORMATION",
    "_MI_HARDWARE_STATE",
    "_MI_PARTITION",
    "_MI_VISIBLE_PARTITION",
    "_MMPAGING_FILE",
    "_KLDR_DATA_TABLE_ENTRY",
    "_LDR_DATA_TABLE_ENTRY",
    "_MM_AVL_TABLE",
    "_MMADDRESS_NODE",
    "_HANDLE_TABLE",
    "_HANDLE_TABLE_ENTRY",
    "_OBJECT_HEADER",
    "_OBJECT_HEADER_NAME_INFO",
    "_OBJECT_TYPE",
    "_OBJECT_DIRECTORY",
    "_OBJECT_DIRECTORY_ENTRY",
    "_POOL_HEADER",
    "_POOL_TRACKER_BIG_PAGES",
    "_PHYSICAL_MEMORY_DESCRIPTOR",
    "_PHYSICAL_MEMORY_RUN",
    "_KSERVICE_TABLE_DESCRIPTOR",
    "_MM_UNLOADED_DRIVER",
    "_EX_CALLBACK",
    "_EX_CALLBACK_ROUTINE_BLOCK",
    "_CM_CALLBACK_CONTEXT_BLOCK",
    "_KBUGCHECK_CALLBACK_RECORD",
    "_KBUGCHECK_REASON_CALLBACK_RECORD",
    "_CMHIVE",
    "_HHIVE",
    "_HBASE_BLOCK",
    "_DUAL",
    "_HMAP_DIRECTORY",
    "_HMAP_TABLE",
    "_HMAP_ENTRY",
];

const PROFILE_SYMBOLS: &[&str] = &[
    "PsActiveProcessHead",
    "PsLoadedModuleList",
    "CmpHiveListHead",
    "ObHeaderCookie",
    "ObTypeIndexTable",
    "ObpInfoMaskToOffset",
    "PsInitialSystemProcess",
    "KeBootTime",
    "KeBootTimeBias",
    "KeInterruptTimeBias",
    "NtBuildLab",
    "NtBuildLabEx",
    "SepLogonSessions",
    "SeCreateTokenPrivilege",
    "SeAssignPrimaryTokenPrivilege",
    "SeTcbPrivilege",
    "SeTakeOwnershipPrivilege",
    "SeLoadDriverPrivilege",
    "SeBackupPrivilege",
    "SeRestorePrivilege",
    "SeDebugPrivilege",
    "SeImpersonatePrivilege",
    "NtBuildNumber",
    "MiState",
    "MiSystemPartition",
    "SmGlobals",
    "NtMajorVersion",
    "NtMinorVersion",
    "KeNumberProcessors",
    "CmNtCSDVersion",
    "KiProcessorBlock",
    "PoolBigPageTable",
    "PoolBigPageTableSize",
    "MmPhysicalMemoryBlock",
    "MmSystemRangeStart",
    "MmHighestUserAddress",
    "MmAvailablePages",
    "MmResidentAvailablePages",
    "MmTotalCommittedPages",
    "MmTotalCommitLimit",
    "MmTotalCommitLimitMaximum",
    "PspCidTable",
    "PspCreateProcessNotifyRoutine",
    "PspCreateProcessNotifyRoutineCount",
    "PspCreateProcessNotifyRoutineExCount",
    "PspCreateThreadNotifyRoutine",
    "PspCreateThreadNotifyRoutineCount",
    "PspCreateThreadNotifyRoutineNonSystemCount",
    "PspLoadImageNotifyRoutine",
    "PspLoadImageNotifyRoutineCount",
    "KeBugCheckCallbackListHead",
    "KeBugCheckReasonCallbackListHead",
    // Registry callback globals changed names across kernel generations.  Keep
    // every name exported by Microsoft PDBs and let the callback plugin select
    // the self-consistent list/vector from the captured memory itself.
    "CallbackListHead",
    "CmCallbackListHead",
    "CmpCallBackCount",
    "CmpCallBackVector",
    "KiWaitNever",
    "KiWaitAlways",
    "KeServiceDescriptorTable",
    "KeServiceDescriptorTableShadow",
    "MmUnloadedDrivers",
    "MmLastUnloadedDriver",
    "IopRootDeviceNode",
    "IopTimerQueueHead",
    "SeCiCallbacks",
    "EtwpDebuggerData",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelProfile {
    pub schema_version: u32,
    pub pdb_guid: String,
    pub pdb_age: u32,
    pub type_name: String,
    pub type_size: u64,
    pub fields: BTreeMap<String, FieldProfile>,
    #[serde(default)]
    pub types: BTreeMap<String, TypeLayout>,
    #[serde(default)]
    pub symbols: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeLayout {
    pub size: u64,
    pub fields: BTreeMap<String, FieldProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldProfile {
    pub offset: u64,
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bit_position: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bit_length: Option<u8>,
}

impl KernelProfile {
    pub fn field(&self, name: &str) -> Result<&FieldProfile> {
        self.fields
            .get(name)
            .with_context(|| format!("符号配置中不包含 _EPROCESS.{name}"))
    }

    pub fn type_layout(&self, name: &str) -> Result<&TypeLayout> {
        self.types
            .get(name)
            .with_context(|| format!("符号配置中不包含类型 {name}"))
    }

    pub fn type_field(&self, type_name: &str, field_name: &str) -> Result<&FieldProfile> {
        self.type_layout(type_name)?
            .fields
            .get(field_name)
            .with_context(|| format!("符号配置中不包含 {type_name}.{field_name}"))
    }

    pub fn symbol_rva(&self, name: &str) -> Result<u32> {
        self.symbols
            .get(name)
            .copied()
            .with_context(|| format!("符号配置中不包含符号 {name}"))
    }
}

const XP_SP2_X86_NTKRNLPA_GUID: &str = "BD8F451F3E754ED8A34B50560CEB08E3";

/// Returns the narrowly fingerprinted public layout needed to bootstrap the
/// retail Windows XP SP2 x86 PAE kernel. That generation's public PDB exposes
/// globals but omits private type records such as `_EPROCESS`, so attempting to
/// build a modern type profile from it cannot work.
///
/// This must not become a broad "XP-like" fallback: every offset below is tied
/// to the exact CodeView identity found in the captured kernel.
pub fn legacy_windows_profile(
    pdb_name: &str,
    pdb_guid: &str,
    pdb_age: u32,
) -> Option<KernelProfile> {
    let normalized_guid = pdb_guid
        .chars()
        .filter(|ch| *ch != '-')
        .collect::<String>()
        .to_ascii_uppercase();
    if !pdb_name.eq_ignore_ascii_case("ntkrnlpa.pdb")
        || normalized_guid != XP_SP2_X86_NTKRNLPA_GUID
        || pdb_age != 1
    {
        return None;
    }

    let mut fields = BTreeMap::new();
    for (name, offset, size) in [
        ("Pcb", 0x000, Some(0x6c)),
        ("CreateTime", 0x070, Some(8)),
        ("ExitTime", 0x078, Some(8)),
        ("UniqueProcessId", 0x084, Some(4)),
        ("ActiveProcessLinks", 0x088, Some(8)),
        ("ObjectTable", 0x0c4, Some(4)),
        ("Token", 0x0c8, Some(4)),
        ("InheritedFromUniqueProcessId", 0x14c, Some(4)),
        ("Session", 0x170, Some(4)),
        ("ImageFileName", 0x174, Some(16)),
        ("ActiveThreads", 0x1a0, Some(4)),
        ("Peb", 0x1b0, Some(4)),
    ] {
        fields.insert(
            name.to_owned(),
            FieldProfile {
                offset,
                size,
                bit_position: None,
                bit_length: None,
            },
        );
    }

    let mut kprocess_fields = BTreeMap::new();
    kprocess_fields.insert(
        "DirectoryTableBase".to_owned(),
        FieldProfile {
            offset: 0x18,
            size: Some(4),
            bit_position: None,
            bit_length: None,
        },
    );
    let mut types = BTreeMap::new();
    types.insert(
        "_EPROCESS".to_owned(),
        TypeLayout {
            size: 0x260,
            fields: fields.clone(),
        },
    );
    types.insert(
        "_KPROCESS".to_owned(),
        TypeLayout {
            size: 0x6c,
            fields: kprocess_fields,
        },
    );
    types.insert(
        "_PEB".to_owned(),
        legacy_type_layout(
            0x210,
            &[
                ("BeingDebugged", 0x002, 1),
                ("ImageBaseAddress", 0x008, 4),
                ("ProcessParameters", 0x010, 4),
                ("NumberOfProcessors", 0x064, 4),
                ("NtGlobalFlag", 0x068, 4),
                ("OSMajorVersion", 0x0a4, 4),
                ("OSMinorVersion", 0x0a8, 4),
                ("OSBuildNumber", 0x0ac, 2),
                ("SessionId", 0x1d4, 4),
            ],
        ),
    );
    types.insert(
        "_RTL_USER_PROCESS_PARAMETERS".to_owned(),
        legacy_type_layout(
            0x290,
            &[
                ("CurrentDirectory", 0x24, 0x0c),
                ("DllPath", 0x30, 8),
                ("ImagePathName", 0x38, 8),
                ("CommandLine", 0x40, 8),
                ("Environment", 0x48, 4),
                ("WindowTitle", 0x70, 8),
                ("DesktopInfo", 0x78, 8),
            ],
        ),
    );
    types.insert(
        "_CURDIR".to_owned(),
        legacy_type_layout(0x0c, &[("DosPath", 0, 8)]),
    );
    types.insert(
        "_UNICODE_STRING".to_owned(),
        legacy_type_layout(
            8,
            &[("Length", 0, 2), ("MaximumLength", 2, 2), ("Buffer", 4, 4)],
        ),
    );
    types.insert(
        "_KUSER_SHARED_DATA".to_owned(),
        legacy_type_layout(
            0x338,
            &[
                ("InterruptTime", 0x008, 0x0c),
                ("SystemTime", 0x014, 0x0c),
                ("TimeZoneBias", 0x020, 0x0c),
                ("NtSystemRoot", 0x030, 520),
                ("NtProductType", 0x264, 4),
                ("NtMajorVersion", 0x26c, 4),
                ("NtMinorVersion", 0x270, 4),
                ("SuiteMask", 0x2d0, 4),
                ("KdDebuggerEnabled", 0x2d4, 1),
                ("NumberOfPhysicalPages", 0x2e8, 4),
                ("SafeBootMode", 0x2ec, 1),
            ],
        ),
    );

    let symbols = BTreeMap::from([
        ("PsLoadedModuleList".to_owned(), 0x0007_c1a0),
        ("PsActiveProcessHead".to_owned(), 0x0008_2258),
    ]);
    Some(KernelProfile {
        schema_version: 3,
        pdb_guid: pdb_guid.to_ascii_uppercase(),
        pdb_age,
        type_name: "_EPROCESS".to_owned(),
        type_size: 0x260,
        fields,
        types,
        symbols,
    })
}

fn legacy_type_layout(size: u64, fields: &[(&str, u64, u64)]) -> TypeLayout {
    TypeLayout {
        size,
        fields: fields
            .iter()
            .map(|(name, offset, field_size)| {
                (
                    (*name).to_owned(),
                    FieldProfile {
                        offset: *offset,
                        size: Some(*field_size),
                        bit_position: None,
                        bit_length: None,
                    },
                )
            })
            .collect(),
    }
}

pub fn is_x86_pae_legacy_profile(profile: &KernelProfile) -> bool {
    profile.pdb_age == 1
        && profile
            .pdb_guid
            .chars()
            .filter(|ch| *ch != '-')
            .collect::<String>()
            .eq_ignore_ascii_case(XP_SP2_X86_NTKRNLPA_GUID)
}

pub fn build_eprocess_profile(path: &Path) -> Result<KernelProfile> {
    let file = File::open(path).with_context(|| format!("无法打开 PDB {}", path.display()))?;
    let mut pdb = pdb::PDB::open(file)?;
    let (pdb_guid, pdb_age) = {
        let info = pdb.pdb_information()?;
        (info.guid.to_string().to_uppercase(), info.age)
    };

    let types = {
        let type_information = pdb.type_information()?;
        let mut finder = type_information.finder();
        let mut iterator = type_information.iter();
        let mut targets: BTreeMap<String, (TypeIndex, u64)> = BTreeMap::new();
        while let Some(record) = iterator.next()? {
            finder.update(&iterator);
            let parsed = match record.parse() {
                Ok(parsed) => parsed,
                Err(_) => continue,
            };
            match parsed {
                TypeData::Class(class)
                    if PROFILE_TYPES.contains(&class.name.to_string().as_ref())
                        && !class.properties.forward_reference() =>
                {
                    if let Some(fields) = class.fields {
                        targets.insert(class.name.to_string().into_owned(), (fields, class.size));
                    }
                }
                TypeData::Union(union)
                    if PROFILE_TYPES.contains(&union.name.to_string().as_ref())
                        && !union.properties.forward_reference() =>
                {
                    targets.insert(
                        union.name.to_string().into_owned(),
                        (union.fields, union.size),
                    );
                }
                _ => {}
            }
        }

        let mut layouts = BTreeMap::new();
        for (name, (field_list, size)) in targets {
            let mut fields = BTreeMap::new();
            collect_members(&finder, field_list, &mut fields)?;
            layouts.insert(name, TypeLayout { size, fields });
        }
        layouts
    };

    let eprocess = types
        .get("_EPROCESS")
        .context("此 PDB 中没有找到 _EPROCESS")?;
    for required in ["UniqueProcessId", "ActiveProcessLinks", "ImageFileName"] {
        ensure!(
            eprocess.fields.contains_key(required),
            "_EPROCESS 符号配置缺少 {required}"
        );
    }
    let fields = eprocess.fields.clone();
    let type_size = eprocess.size;
    let symbols = collect_symbol_rvas(&mut pdb)?;

    Ok(KernelProfile {
        schema_version: 3,
        pdb_guid,
        pdb_age,
        type_name: "_EPROCESS".to_owned(),
        type_size,
        fields,
        types,
        symbols,
    })
}

fn collect_symbol_rvas(pdb: &mut pdb::PDB<'_, File>) -> Result<BTreeMap<String, u32>> {
    collect_selected_symbol_rvas(pdb, PROFILE_SYMBOLS)
}

/// Reads selected public/data symbol RVAs from any Microsoft PDB.  This is
/// intentionally independent of `KernelProfile`: driver-backed plugins such
/// as netstat, consoles and GUI enumeration must bind globals to the exact
/// module captured in memory, not to a Windows build-number table.
pub fn read_pdb_symbol_rvas(path: &Path, names: &[&str]) -> Result<BTreeMap<String, u32>> {
    let file = File::open(path).with_context(|| format!("无法打开 PDB {}", path.display()))?;
    let mut pdb = pdb::PDB::open(file)?;
    collect_selected_symbol_rvas(&mut pdb, names)
}

/// Reads selected concrete type layouts from any Microsoft PDB. Driver-backed
/// plugins use this alongside `read_pdb_symbol_rvas` so private Windows
/// structures follow the exact module captured in memory instead of a build
/// number table.
pub fn read_pdb_type_layouts(path: &Path, names: &[&str]) -> Result<BTreeMap<String, TypeLayout>> {
    let file = File::open(path).with_context(|| format!("无法打开 PDB {}", path.display()))?;
    let mut pdb = pdb::PDB::open(file)?;
    let type_information = pdb.type_information()?;
    let mut finder = type_information.finder();
    let mut iterator = type_information.iter();
    let mut targets = BTreeMap::<String, (TypeIndex, u64)>::new();
    while let Some(record) = iterator.next()? {
        finder.update(&iterator);
        let parsed = match record.parse() {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };
        match parsed {
            TypeData::Class(class)
                if names.contains(&class.name.to_string().as_ref())
                    && !class.properties.forward_reference() =>
            {
                if let Some(fields) = class.fields {
                    targets.insert(class.name.to_string().into_owned(), (fields, class.size));
                }
            }
            TypeData::Union(union)
                if names.contains(&union.name.to_string().as_ref())
                    && !union.properties.forward_reference() =>
            {
                targets.insert(
                    union.name.to_string().into_owned(),
                    (union.fields, union.size),
                );
            }
            _ => {}
        }
    }

    let mut layouts = BTreeMap::new();
    for (name, (field_list, size)) in targets {
        let mut fields = BTreeMap::new();
        collect_members(&finder, field_list, &mut fields)?;
        layouts.insert(name, TypeLayout { size, fields });
    }
    Ok(layouts)
}

fn collect_selected_symbol_rvas(
    pdb: &mut pdb::PDB<'_, File>,
    names: &[&str],
) -> Result<BTreeMap<String, u32>> {
    let address_map = pdb.address_map()?;
    let mut symbols = BTreeMap::new();
    {
        let table = pdb.global_symbols()?;
        let mut iterator = table.iter();
        while let Some(symbol) = iterator.next()? {
            let parsed = match symbol.parse() {
                Ok(parsed) => parsed,
                Err(_) => continue,
            };
            let (name, offset) = match parsed {
                pdb::SymbolData::Public(data) => (data.name, data.offset),
                pdb::SymbolData::Data(data) => (data.name, data.offset),
                _ => continue,
            };
            let raw_name = name.to_string();
            let Some(name) = requested_symbol_name(raw_name.as_ref(), names) else {
                continue;
            };
            if let Some(rva) = offset.to_rva(&address_map) {
                symbols.insert(name.to_owned(), rva.0);
            }
        }
    }

    // Some exact Microsoft PDBs keep internal kernel globals (notably the
    // registry callback list/count) only in a compilation module's private
    // symbol stream.  Scan those streams once for still-missing requested
    // names.  Global data wins over a same-named local static, and the public
    // stream above always has highest precedence.
    if symbols.len() < names.len() {
        let public_names = symbols.keys().cloned().collect::<BTreeSet<_>>();
        let mut private_global = BTreeMap::<String, bool>::new();
        let debug = pdb.debug_information()?;
        let mut modules = debug.modules()?;
        while let Some(module) = modules.next()? {
            let Some(info) = pdb.module_info(&module)? else {
                continue;
            };
            let mut iterator = info.symbols()?;
            while let Some(symbol) = iterator.next()? {
                let Ok(pdb::SymbolData::Data(data)) = symbol.parse() else {
                    continue;
                };
                let raw_name = data.name.to_string();
                let Some(name) = requested_symbol_name(raw_name.as_ref(), names) else {
                    continue;
                };
                if public_names.contains(name) {
                    continue;
                }
                let replace = !private_global.contains_key(name)
                    || data.global && private_global.get(name) == Some(&false);
                if replace && let Some(rva) = data.offset.to_rva(&address_map) {
                    symbols.insert(name.to_owned(), rva.0);
                    private_global.insert(name.to_owned(), data.global);
                }
            }
        }
    }
    Ok(symbols)
}

fn requested_symbol_name<'a>(raw_name: &str, names: &[&'a str]) -> Option<&'a str> {
    if let Some(name) = names.iter().copied().find(|name| *name == raw_name) {
        return Some(name);
    }
    let decorated = raw_name.strip_prefix('?')?;
    let (base_name, suffix) = decorated.split_once("@@")?;
    if !suffix.starts_with('3') {
        return None;
    }
    names.iter().copied().find(|name| *name == base_name)
}

fn collect_members(
    finder: &TypeFinder<'_>,
    field_list: TypeIndex,
    output: &mut BTreeMap<String, FieldProfile>,
) -> Result<()> {
    let parsed = finder.find(field_list)?.parse()?;
    let TypeData::FieldList(list) = parsed else {
        bail!("类型 {field_list} 不是字段列表");
    };
    for field in list.fields {
        if let TypeData::Member(member) = field {
            let name = member.name.to_string().into_owned();
            let member_type = finder.find(member.field_type)?.parse()?;
            let (bit_position, bit_length) = match member_type {
                TypeData::Bitfield(bitfield) => (Some(bitfield.position), Some(bitfield.length)),
                _ => (None, None),
            };
            let size = type_size(finder, member.field_type, 0).ok().flatten();
            output.insert(
                name,
                FieldProfile {
                    offset: member.offset,
                    size,
                    bit_position,
                    bit_length,
                },
            );
        }
    }
    if let Some(continuation) = list.continuation {
        collect_members(finder, continuation, output)?;
    }
    Ok(())
}

fn type_size(finder: &TypeFinder<'_>, index: TypeIndex, depth: usize) -> Result<Option<u64>> {
    if depth > 16 {
        return Ok(None);
    }
    let size = match finder.find(index)?.parse()? {
        TypeData::Primitive(primitive) if primitive.indirection.is_some() => Some(8),
        TypeData::Primitive(primitive) => match primitive.kind {
            pdb::PrimitiveKind::Char
            | pdb::PrimitiveKind::UChar
            | pdb::PrimitiveKind::I8
            | pdb::PrimitiveKind::U8
            | pdb::PrimitiveKind::Bool8 => Some(1),
            pdb::PrimitiveKind::I16 | pdb::PrimitiveKind::U16 => Some(2),
            pdb::PrimitiveKind::I32 | pdb::PrimitiveKind::U32 | pdb::PrimitiveKind::F32 => Some(4),
            pdb::PrimitiveKind::I64 | pdb::PrimitiveKind::U64 | pdb::PrimitiveKind::F64 => Some(8),
            _ => None,
        },
        TypeData::Pointer(_) => Some(8),
        TypeData::Array(array) => array.dimensions.last().copied().map(u64::from),
        TypeData::Class(class) => Some(class.size),
        TypeData::Union(union) => Some(union.size),
        TypeData::Modifier(modifier) => type_size(finder, modifier.underlying_type, depth + 1)?,
        TypeData::Bitfield(bitfield) => type_size(finder, bitfield.underlying_type, depth + 1)?,
        _ => None,
    };
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::requested_symbol_name;

    #[test]
    fn recognizes_msvc_decorated_data_globals_only() {
        let names = ["ndisGlobalTriageBlock"];
        assert_eq!(
            requested_symbol_name(
                "?ndisGlobalTriageBlock@@3U_NDIS_GLOBAL_TRIAGE_BLOCK@@A",
                &names,
            ),
            Some("ndisGlobalTriageBlock")
        );
        assert_eq!(
            requested_symbol_name("ndisGlobalTriageBlock", &names),
            Some("ndisGlobalTriageBlock")
        );
        assert_eq!(
            requested_symbol_name("?ndisGlobalTriageBlock@@YAXXZ", &names),
            None
        );
    }
}

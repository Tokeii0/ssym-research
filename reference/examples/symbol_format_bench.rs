//! Reproducible symbol-format experiment. Timings exclude licensing and report serialization.
use std::alloc::{GlobalAlloc, Layout, System};
use std::fs::File;
use std::hint::black_box;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

use anyhow::{Result, ensure};
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;
use sha2::{Digest, Sha256};
use starmem::output_policy::OutputPolicy;
use starmem::profile::compiled::{self, CompiledProfileFile};
use starmem::profile::{KernelProfile, build_eprocess_profile};
use starmem::session::{AnalysisSession, SessionOptions};

// Requested live/peak heap bytes, not RSS or allocator-reserved memory. Every
// backend uses this same allocator. The counters add overhead to allocations.
struct Counted;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicUsize = AtomicUsize::new(0);
static COUNT: AtomicBool = AtomicBool::new(false);
static TRACK: AtomicBool = AtomicBool::new(true);

fn allocated(size: usize) {
    let live = LIVE.fetch_add(size, Relaxed) + size;
    PEAK.fetch_max(live, Relaxed);
    if COUNT.load(Relaxed) {
        CALLS.fetch_add(1, Relaxed);
    }
}

unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() && TRACK.load(Relaxed) {
            allocated(layout.size());
        }
        ptr
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() && TRACK.load(Relaxed) {
            allocated(layout.size());
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe {
            System.dealloc(ptr, layout);
        }
        if TRACK.load(Relaxed) {
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(ptr, layout, size) };
        if !result.is_null() && TRACK.load(Relaxed) {
            LIVE.fetch_sub(layout.size(), Relaxed);
            allocated(size);
        }
        result
    }
}

#[global_allocator]
static ALLOCATOR: Counted = Counted;

#[derive(Parser)]
struct Args {
    /// Separate allocation instrumentation from latency measurements.
    #[arg(long)]
    measure_heap: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Resolve the exact image PDB and create semantically identical JSON/SSYM inputs.
    Prepare {
        #[arg(long)]
        image: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long = "symbol-path")]
        symbol_paths: Vec<PathBuf>,
    },
    /// One fresh-process load, plus repeated fixed field/symbol lookups.
    Load {
        #[arg(long, value_enum)]
        format: Format,
        #[arg(long)]
        path: PathBuf,
        #[arg(long, default_value_t = 10_000)]
        lookups: usize,
    },
    /// Exercise the actual AnalysisSession + pslist path, preserving the full report.
    Session {
        #[arg(long)]
        image: PathBuf,
        #[arg(long, value_enum)]
        format: SessionFormat,
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum, Debug)]
enum Format {
    Pdb,
    JsonReader,
    JsonSlice,
    JsonZstd,
    Ssym,
    SsymOwned,
}
#[derive(Clone, Copy, ValueEnum, Debug)]
enum SessionFormat {
    Pdb,
    Json,
    Ssym,
}

fn main() -> Result<()> {
    let args = Args::parse();
    starmem::lovelymem_license::verify_current_license().map_err(anyhow::Error::msg)?;
    let executor = starmem::resources::AnalysisExecutor::new(
        starmem::resources::AnalysisResourceLimits::new(2, 2 * 1024 * 1024 * 1024)?,
    )?;
    TRACK.store(args.measure_heap, Relaxed);
    let result = executor.install(|| run(args))?;
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

fn run(args: Args) -> Result<serde_json::Value> {
    match args.command {
        Command::Prepare {
            image,
            output,
            symbol_paths,
        } => prepare(&image, &output, &symbol_paths),
        Command::Load {
            format,
            path,
            lookups,
        } => load(format, &path, lookups),
        Command::Session {
            image,
            format,
            path,
            report,
        } => {
            let mut policy = OutputPolicy::new(false);
            policy
                .protect_file("image", &image)
                .protect_file("symbols", &path)
                .output_file("report", &report);
            policy.preflight()?;
            let mut options = SessionOptions {
                allow_compatible_local_symbols: false,
                ..Default::default()
            };
            match format {
                SessionFormat::Pdb => options.pdb = Some(path.clone()),
                SessionFormat::Json | SessionFormat::Ssym => options.profile = Some(path.clone()),
            }
            let started = Instant::now();
            let session = AnalysisSession::open(&image, options)?;
            let open_ms = started.elapsed().as_secs_f64() * 1000.0;
            let pslist_started = Instant::now();
            let processes = session.processes(16_384)?;
            let pslist_ms = pslist_started.elapsed().as_secs_f64() * 1000.0;
            let count = processes.processes.len();
            let complete = processes.complete;
            let result = json!({"kernel":session.kernel(), "processes":processes, "session_warnings":session.warnings()});
            let bytes = serde_json::to_vec(&result)?;
            policy.write_file_atomic(&report, |file| Ok(file.write_all(&bytes)?))?;
            Ok(
                json!({"image":image,"format":format!("{format:?}"),"open_ms":open_ms,
                "pslist_ms":pslist_ms,"total_ms":open_ms+pslist_ms,"records":count,"complete":complete,
                "report_sha256":hex::encode(Sha256::digest(&bytes)),"report":report,
                "build":starmem::build_info::current_build_info()}),
            )
        }
    }
}

fn prepare(image_path: &Path, output: &Path, roots: &[PathBuf]) -> Result<serde_json::Value> {
    let image_sha256 = hex::encode(compiled::file_sha256(image_path)?);
    let image = starmem::image::MemoryImage::open(image_path)?;
    let mut store = starmem::symbols::default_symbol_store();
    for root in roots {
        store.add_search_root(root.clone());
    }
    let resolved = starmem::symbols::resolve_kernel_pdb_with_store(&image, &store)?;
    let source_hash = compiled::file_sha256(&resolved.path)?;
    let started = Instant::now();
    let profile = build_eprocess_profile(&resolved.path)?;
    let pdb_extract_ms = started.elapsed().as_secs_f64() * 1000.0;
    ensure!(
        source_hash == compiled::file_sha256(&resolved.path)?,
        "PDB changed while compiling"
    );
    let started = Instant::now();
    let original_age = compiled::pdb_original_age(&resolved.path)?;
    let binary = compiled::encode(
        &profile,
        &resolved.record.pdb_name,
        source_hash,
        original_age,
    )?;
    let ssym_encode_validate_ms = started.elapsed().as_secs_f64() * 1000.0;
    let view = compiled::CompiledProfile::parse(&binary)?;
    view.validate_identity(
        &resolved.record.pdb_name,
        &resolved.record.guid,
        resolved.record.age,
    )?;
    let compact = serde_json::to_vec(&profile)?;
    ensure!(
        compact == serde_json::to_vec(&view.to_profile())?,
        "SSYM roundtrip mismatch"
    );
    let pretty = serde_json::to_vec_pretty(&profile)?;
    let compressed = zstd::stream::encode_all(compact.as_slice(), 3)?;
    let files = [
        ("profile.json", compact),
        ("profile.pretty.json", pretty),
        ("profile.json.zst", compressed),
        ("profile.ssym", binary),
    ];
    let mut policy = OutputPolicy::new(false);
    policy
        .protect_file("image", image_path)
        .protect_file("pdb", &resolved.path);
    for (name, _) in &files {
        policy.output_file(*name, output.join(name));
    }
    policy.output_file("manifest", output.join("manifest.json"));
    policy.preflight()?;
    std::fs::create_dir_all(output)?;
    let mut artifacts = Vec::new();
    for (name, bytes) in &files {
        policy.write_file_atomic(&output.join(name), |file| Ok(file.write_all(bytes)?))?;
        artifacts.push(json!({"path":output.join(name),"bytes":bytes.len(),"sha256":hex::encode(Sha256::digest(bytes))}));
    }
    let manifest = json!({"image":image_path,"image_bytes":std::fs::metadata(image_path)?.len(),"image_sha256":image_sha256,
        "pdb":resolved.path,"pdb_bytes":std::fs::metadata(&resolved.path)?.len(),"pdb_sha256":hex::encode(source_hash),
        "identity":resolved.record,"recipe_sha256":hex::encode(compiled::recipe_sha256()),
        "build":starmem::build_info::current_build_info(),"threads":2,
        "pdb_extract_ms":pdb_extract_ms,"ssym_encode_validate_ms":ssym_encode_validate_ms,
        "pdb_info_age":profile.pdb_age,"pdb_dbi_age":original_age,
        "type_count":profile.types.len(),"root_fields":profile.fields.len(),
        "type_fields":profile.types.values().map(|ty| ty.fields.len()).sum::<usize>(),
        "symbol_count":profile.symbols.len(),"profile_sha256":hex::encode(Sha256::digest(&files[0].1)),
        "artifacts":artifacts});
    policy.write_file_atomic(&output.join("manifest.json"), |file| {
        Ok(serde_json::to_writer_pretty(file, &manifest)?)
    })?;
    Ok(manifest)
}

enum Loaded {
    Owned(KernelProfile),
    Indexed(CompiledProfileFile),
}

impl Loaded {
    fn query(&self, index: usize) -> u64 {
        const FIELDS: &[&str] = &[
            "UniqueProcessId",
            "ActiveProcessLinks",
            "ImageFileName",
            "Token",
            "MissingField",
        ];
        const SYMBOLS: &[&str] = &[
            "PsActiveProcessHead",
            "PsLoadedModuleList",
            "NtBuildNumber",
            "MissingSymbol",
        ];
        let field = FIELDS[index % FIELDS.len()];
        let symbol = SYMBOLS[index % SYMBOLS.len()];
        match self {
            Self::Owned(profile) => {
                profile
                    .types
                    .get("_EPROCESS")
                    .and_then(|ty| ty.fields.get(field))
                    .map_or(0, |f| f.offset)
                    ^ u64::from(profile.symbols.get(symbol).copied().unwrap_or(0))
            }
            Self::Indexed(file) => {
                file.view()
                    .type_field("_EPROCESS", field)
                    .map_or(0, |f| f.offset)
                    ^ u64::from(file.view().symbol_rva(symbol).unwrap_or(0))
            }
        }
    }
}

fn bounded_read(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(compiled::MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= compiled::MAX_BYTES as usize,
        "symbol input too large"
    );
    Ok(bytes)
}

fn load(format: Format, path: &Path, lookups: usize) -> Result<serde_json::Value> {
    ensure!(
        (1..=10_000_000).contains(&lookups),
        "lookups must be 1..10000000"
    );
    let baseline = LIVE.load(Relaxed);
    PEAK.store(baseline, Relaxed);
    CALLS.store(0, Relaxed);
    COUNT.store(true, Relaxed);
    let started = Instant::now();
    let loaded = match format {
        Format::Pdb => Loaded::Owned(build_eprocess_profile(path)?),
        Format::JsonReader => {
            Loaded::Owned(serde_json::from_reader(BufReader::new(File::open(path)?))?)
        }
        Format::JsonSlice => Loaded::Owned(serde_json::from_slice(&bounded_read(path)?)?),
        Format::JsonZstd => {
            let mut bytes = Vec::new();
            zstd::stream::read::Decoder::new(File::open(path)?)?
                .take(compiled::MAX_BYTES + 1)
                .read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() <= compiled::MAX_BYTES as usize,
                "decompressed profile too large"
            );
            Loaded::Owned(serde_json::from_slice(&bytes)?)
        }
        Format::Ssym => {
            let file = CompiledProfileFile::open(path)?;
            file.view().require_current_recipe()?;
            Loaded::Indexed(file)
        }
        Format::SsymOwned => {
            let file = CompiledProfileFile::open(path)?;
            file.view().require_current_recipe()?;
            Loaded::Owned(file.view().to_profile())
        }
    };
    let load_us = started.elapsed().as_secs_f64() * 1_000_000.0;
    COUNT.store(false, Relaxed);
    let peak_heap_bytes = PEAK.load(Relaxed).saturating_sub(baseline);
    let retained_heap_bytes = LIVE.load(Relaxed).saturating_sub(baseline);
    let allocation_calls = CALLS.load(Relaxed);
    let started = Instant::now();
    let mut sum = 0u64;
    for index in 0..lookups {
        sum = sum.wrapping_add(black_box(&loaded).query(black_box(index)));
    }
    let lookup_ns = started.elapsed().as_secs_f64() * 1_000_000_000.0 / lookups as f64;
    let profile = match &loaded {
        Loaded::Owned(profile) => serde_json::to_vec(profile)?,
        Loaded::Indexed(file) => serde_json::to_vec(&file.view().to_profile())?,
    };
    Ok(
        json!({"format":format!("{format:?}"),"path":path,"file_bytes":std::fs::metadata(path)?.len(),
        "load_us":load_us,"lookup_pair_ns":lookup_ns,"lookups":lookups,"lookup_sum":black_box(sum),
        "heap_instrumented":TRACK.load(Relaxed),
        "peak_heap_bytes":TRACK.load(Relaxed).then_some(peak_heap_bytes),
        "retained_heap_bytes":TRACK.load(Relaxed).then_some(retained_heap_bytes),
        "allocation_calls":TRACK.load(Relaxed).then_some(allocation_calls),
        "profile_sha256":hex::encode(Sha256::digest(&profile)),"build_id":starmem::BUILD_ID}),
    )
}

use std::sync::LazyLock;

/// Whether to use WriteMap (memory-mapped writes) for MDBX (default: true)
pub(super) static MDBX_USE_WRITEMAP: LazyLock<bool> =
	lazy_env_parse!("SURREAL_MDBX_USE_WRITEMAP", bool, true);

/// Whether to use NoTLS mode for MDBX (default: false)
pub(super) static MDBX_NO_TLS: LazyLock<bool> =
	lazy_env_parse!("SURREAL_MDBX_NO_TLS", bool, false);

/// Whether to use exclusive access mode for MDBX (default: false)
pub(super) static MDBX_EXCLUSIVE: LazyLock<bool> =
	lazy_env_parse!("SURREAL_MDBX_EXCLUSIVE", bool, false);

/// Whether to use accede mode for MDBX (default: false)
pub(super) static MDBX_ACCEDE: LazyLock<bool> =
	lazy_env_parse!("SURREAL_MDBX_ACCEDE", bool, false);

/// Whether database should be opened in read-only mode (default: false)
pub(super) static MDBX_READ_ONLY: LazyLock<bool> =
	lazy_env_parse!("SURREAL_MDBX_READ_ONLY", bool, false);

/// Whether to disable subdir mode (single file instead of directory) (default: false)
pub(super) static MDBX_NO_SUBDIR: LazyLock<bool> =
	lazy_env_parse!("SURREAL_MDBX_NO_SUBDIR", bool, false);

/// MDBX sync mode: "durable", "no_meta_sync", "safe_no_sync", "utterly_no_sync" (default: "durable")
pub(super) static MDBX_SYNC_MODE: LazyLock<String> =
	lazy_env_parse!("SURREAL_MDBX_SYNC_MODE", String, "durable".to_string());

/// Maximum database size in bytes (default: 128 GiB)
pub(super) static MDBX_MAX_DB_SIZE: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_MAX_DB_SIZE", usize, 128 * 1024 * 1024 * 1024);

/// Maximum number of database tables (default: 256)
pub(super) static MDBX_MAX_TABLES: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_MAX_TABLES", usize, 256);

/// Maximum number of readers/threads (default: 126)
pub(super) static MDBX_MAX_READERS: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_MAX_READERS", usize, 126);

/// Page size in bytes (default: 4096)
pub(super) static MDBX_PAGE_SIZE: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_PAGE_SIZE", usize, 4096);

/// Growth step in bytes (default: 16 MiB)
pub(super) static MDBX_GROWTH_STEP: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_GROWTH_STEP", usize, 16 * 1024 * 1024);

/// Shrink threshold in bytes (default: 32 MiB)
pub(super) static MDBX_SHRINK_THRESHOLD: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_SHRINK_THRESHOLD", usize, 32 * 1024 * 1024);

/// Database compaction threshold (default: 8)
pub(super) static MDBX_COMPACTION_THRESHOLD: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_COMPACTION_THRESHOLD", usize, 8);

/// Merge threshold 16dot16 percent (default: 65536 = 100%)
pub(super) static MDBX_MERGE_THRESHOLD: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_MERGE_THRESHOLD", usize, 65536);

/// DP reserve limit (default: 1024)
pub(super) static MDBX_DP_RESERVE_LIMIT: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_DP_RESERVE_LIMIT", usize, 1024);

/// Transaction DP initial allocation (default: 1024)
pub(super) static MDBX_TXN_DP_INITIAL: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_TXN_DP_INITIAL", usize, 1024);

/// Transaction DP limit (default: 65536)
pub(super) static MDBX_TXN_DP_LIMIT: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_TXN_DP_LIMIT", usize, 65536);

/// Spill max denominator (default: 8)
pub(super) static MDBX_SPILL_MAX_DENOMINATOR: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_SPILL_MAX_DENOMINATOR", usize, 8);

/// Spill min denominator (default: 16)
pub(super) static MDBX_SPILL_MIN_DENOMINATOR: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_SPILL_MIN_DENOMINATOR", usize, 16);

/// Spill parent4child denominator (default: 0)
pub(super) static MDBX_SPILL_PARENT4CHILD_DENOMINATOR: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_SPILL_PARENT4CHILD_DENOMINATOR", usize, 0);

/// PNL initial allocation (default: 1024)
pub(super) static MDBX_PNL_INITIAL: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_PNL_INITIAL", usize, 1024);

/// PNL maximum size (default: 131072)
pub(super) static MDBX_PNL_MAX: LazyLock<usize> =
	lazy_env_parse!("SURREAL_MDBX_PNL_MAX", usize, 131072);

/// Returns the MDBX map mode as a string for diagnostics/logging
pub fn mdbx_map_mode() -> &'static str {
	if *MDBX_USE_WRITEMAP {
		"WriteMap"
	} else {
		"NoWriteMap"
	}
}

/// Returns the MDBX sync mode as a string for diagnostics/logging
pub fn mdbx_sync_mode() -> &'static str {
	MDBX_SYNC_MODE.as_str()
}

/// Returns the MDBX configuration summary as a string for diagnostics/logging
pub fn mdbx_config_summary() -> String {
	format!(
		"MDBX Config: map_mode={}, sync_mode={}, max_db_size={}MB, max_tables={}, max_readers={}, page_size={}KB",
		mdbx_map_mode(),
		mdbx_sync_mode(),
		*MDBX_MAX_DB_SIZE / (1024 * 1024),
		*MDBX_MAX_TABLES,
		*MDBX_MAX_READERS,
		*MDBX_PAGE_SIZE / 1024
	)
}

/// Returns environment flags based on configuration
pub fn get_env_flags() -> Vec<String> {
	let mut flags = Vec::new();
	
	if *MDBX_READ_ONLY {
		flags.push("RDONLY".to_string());
	}
	if *MDBX_EXCLUSIVE {
		flags.push("EXCLUSIVE".to_string());
	}
	if *MDBX_ACCEDE {
		flags.push("ACCEDE".to_string());
	}
	if *MDBX_NO_SUBDIR {
		flags.push("NOSUBDIR".to_string());
	}
	if *MDBX_NO_TLS {
		flags.push("NOTLS".to_string());
	}
	
	flags
}

/// Returns performance tuning settings summary
pub fn mdbx_performance_summary() -> String {
	format!(
		"MDBX Performance: growth_step={}MB, shrink_threshold={}MB, compaction_threshold={}, merge_threshold={}%",
		*MDBX_GROWTH_STEP / (1024 * 1024),
		*MDBX_SHRINK_THRESHOLD / (1024 * 1024),
		*MDBX_COMPACTION_THRESHOLD,
		(*MDBX_MERGE_THRESHOLD * 100) / 65536
	)
}
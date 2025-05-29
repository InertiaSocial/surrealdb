#![cfg(feature = "kv-mdbx")]

mod cnf;

use crate::err::Error;
use crate::kvs::api as kvs_api;
use crate::kvs::savepoint::SavePoints;
use crate::kvs::{Check, Key, Val};
use async_trait::async_trait;
use libmdbx::{Database, Table, Transaction as MdbxTxn, WriteFlags, WriteMap, RW};
use std::path::Path;
use std::sync::Arc;

pub struct Datastore {
	db: Arc<Database<WriteMap>>,
}

pub struct Transaction {
	db: Arc<Database<WriteMap>>,
	txn: Option<MdbxTxn<'static, RW, WriteMap>>,
	done: bool,
	write: bool,
	check: Check,
	save_points: SavePoints,
}

impl Transaction {
	pub fn new(
		db: Arc<Database<WriteMap>>,
		txn: MdbxTxn<'static, RW, WriteMap>,
		write: bool,
		check: Check,
	) -> Self {
		Self {
			db,
			txn: Some(txn),
			done: false,
			write,
			check,
			save_points: SavePoints::default(),
		}
	}
}

#[async_trait]
impl kvs_api::Transaction for Transaction {
	fn kind(&self) -> &'static str {
		"mdbx"
	}

	fn get_save_points(&mut self) -> &mut SavePoints {
		&mut self.save_points
	}

	fn check_level(&mut self, check: Check) {
		self.check = check;
	}

	fn closed(&self) -> bool {
		self.done
	}

	fn writeable(&self) -> bool {
		self.write
	}

	async fn cancel(&mut self) -> Result<(), Error> {
		self.done = true;
		self.txn.take();
		Ok(())
	}

	async fn commit(&mut self) -> Result<(), Error> {
		if let Some(txn) = self.txn.take() {
			self.done = true;
			txn.commit().map(|_| ()).map_err(|e| Error::Ds(e.to_string()))
		} else {
			Err(Error::Ds("No active transaction".to_string()))
		}
	}

	async fn exists(&mut self, key: Key, version: Option<u64>) -> Result<bool, Error> {
		// MDBX does not support versioned queries.
		if version.is_some() {
			return Err(Error::UnsupportedVersionedQueries);
		}
		let result: Result<Option<Val>, Error> = self.get(key, None).await;
		result.map(|v| v.is_some())
	}

	async fn get(&mut self, key: Key, version: Option<u64>) -> Result<Option<Val>, Error> {
		// MDBX does not support versioned queries.
		if version.is_some() {
			return Err(Error::UnsupportedVersionedQueries);
		}
		if let Some(txn) = self.txn.as_ref() {
			let table: Table = txn.open_table(None).map_err(|e| Error::Ds(e.to_string()))?;
			let result: Result<Option<std::borrow::Cow<[u8]>>, _> = txn.get(&table, &key);
			let res: Option<Val> =
				result.map_err(|e| Error::Ds(e.to_string()))?.map(|cow| cow.to_vec());
			Ok(res)
		} else {
			Err(Error::Ds("No active transaction".to_string()))
		}
	}

	async fn set(&mut self, key: Key, val: Val, version: Option<u64>) -> Result<(), Error> {
		// MDBX does not support versioned queries.
		if version.is_some() {
			return Err(Error::UnsupportedVersionedQueries);
		}
		if let Some(txn) = self.txn.as_mut() {
			let table: Table = txn.open_table(None).map_err(|e| Error::Ds(e.to_string()))?;
			txn.put(&table, &key, &val, WriteFlags::empty()).map_err(|e| Error::Ds(e.to_string()))
		} else {
			Err(Error::Ds("No active transaction".to_string()))
		}
	}

	async fn put(&mut self, key: Key, val: Val, version: Option<u64>) -> Result<(), Error> {
		// MDBX does not support versioned queries.
		if version.is_some() {
			return Err(Error::UnsupportedVersionedQueries);
		}
		self.set(key, val, None).await
	}

	async fn putc(&mut self, key: Key, val: Val, _chk: Option<Val>) -> Result<(), Error> {
		self.set(key, val, None).await
	}

	async fn replace(&mut self, key: Key, val: Val) -> Result<(), Error> {
		self.set(key, val, None).await
	}

	async fn del(&mut self, key: Key) -> Result<(), Error> {
		if let Some(txn) = self.txn.as_mut() {
			let table: Table = txn.open_table(None).map_err(|e| Error::Ds(e.to_string()))?;
			let result: Result<bool, _> = txn.del(&table, &key, None);
			result.map(|_| ()).map_err(|e| Error::Ds(e.to_string()))
		} else {
			Err(Error::Ds("No active transaction".to_string()))
		}
	}

	async fn delc(&mut self, key: Key, _chk: Option<Val>) -> Result<(), Error> {
		self.del(key).await
	}

	async fn keys(
		&mut self,
		rng: std::ops::Range<Key>,
		limit: u32,
		version: Option<u64>,
	) -> Result<Vec<Key>, Error> {
		// MDBX does not support versioned queries.
		if version.is_some() {
			return Err(Error::UnsupportedVersionedQueries);
		}
		if let Some(txn) = self.txn.as_ref() {
			let table: Table = txn.open_table(None).map_err(|e| Error::Ds(e.to_string()))?;
			let mut cursor = txn.cursor(&table).map_err(|e| Error::Ds(e.to_string()))?;
			let mut keys = Vec::new();
			let mut count = 0u32;

			for item in cursor.iter::<std::borrow::Cow<[u8]>, std::borrow::Cow<[u8]>>() {
				if count >= limit {
					break;
				}
				match item {
					Ok((key_bytes, _)) => {
						let key = key_bytes.to_vec();
						if key >= rng.start && key < rng.end {
							keys.push(key);
							count += 1;
						} else if key >= rng.end {
							break;
						}
					}
					Err(e) => return Err(Error::Ds(e.to_string())),
				}
			}
			Ok(keys)
		} else {
			Err(Error::Ds("No active transaction".to_string()))
		}
	}

	async fn keysr(
		&mut self,
		rng: std::ops::Range<Key>,
		limit: u32,
		version: Option<u64>,
	) -> Result<Vec<Key>, Error> {
		// MDBX does not support versioned queries.
		if version.is_some() {
			return Err(Error::UnsupportedVersionedQueries);
		}
		if let Some(txn) = self.txn.as_ref() {
			let table: Table = txn.open_table(None).map_err(|e| Error::Ds(e.to_string()))?;
			let mut cursor = txn.cursor(&table).map_err(|e| Error::Ds(e.to_string()))?;
			let mut keys = Vec::new();
			let mut count = 0u32;

			// For reverse scan, collect all keys in range first, then reverse
			for item in cursor.iter::<std::borrow::Cow<[u8]>, std::borrow::Cow<[u8]>>() {
				if count >= limit {
					break;
				}
				match item {
					Ok((key_bytes, _)) => {
						let key = key_bytes.to_vec();
						if key >= rng.start && key < rng.end {
							keys.push(key);
							count += 1;
						} else if key >= rng.end {
							break;
						}
					}
					Err(e) => return Err(Error::Ds(e.to_string())),
				}
			}
			keys.reverse();
			Ok(keys)
		} else {
			Err(Error::Ds("No active transaction".to_string()))
		}
	}

	async fn scan(
		&mut self,
		rng: std::ops::Range<Key>,
		limit: u32,
		version: Option<u64>,
	) -> Result<Vec<(Key, Val)>, Error> {
		// MDBX does not support versioned queries.
		if version.is_some() {
			return Err(Error::UnsupportedVersionedQueries);
		}
		if let Some(txn) = self.txn.as_ref() {
			let table: Table = txn.open_table(None).map_err(|e| Error::Ds(e.to_string()))?;
			let mut cursor = txn.cursor(&table).map_err(|e| Error::Ds(e.to_string()))?;
			let mut pairs = Vec::new();
			let mut count = 0u32;

			for item in cursor.iter::<std::borrow::Cow<[u8]>, std::borrow::Cow<[u8]>>() {
				if count >= limit {
					break;
				}
				match item {
					Ok((key_bytes, val_bytes)) => {
						let key = key_bytes.to_vec();
						if key >= rng.start && key < rng.end {
							pairs.push((key, val_bytes.to_vec()));
							count += 1;
						} else if key >= rng.end {
							break;
						}
					}
					Err(e) => return Err(Error::Ds(e.to_string())),
				}
			}
			Ok(pairs)
		} else {
			Err(Error::Ds("No active transaction".to_string()))
		}
	}

	async fn scanr(
		&mut self,
		rng: std::ops::Range<Key>,
		limit: u32,
		version: Option<u64>,
	) -> Result<Vec<(Key, Val)>, Error> {
		// MDBX does not support versioned queries.
		if version.is_some() {
			return Err(Error::UnsupportedVersionedQueries);
		}
		if let Some(txn) = self.txn.as_ref() {
			let table: Table = txn.open_table(None).map_err(|e| Error::Ds(e.to_string()))?;
			let mut cursor = txn.cursor(&table).map_err(|e| Error::Ds(e.to_string()))?;
			let mut pairs = Vec::new();
			let mut count = 0u32;

			// For reverse scan, collect all pairs in range first, then reverse
			for item in cursor.iter::<std::borrow::Cow<[u8]>, std::borrow::Cow<[u8]>>() {
				if count >= limit {
					break;
				}
				match item {
					Ok((key_bytes, val_bytes)) => {
						let key = key_bytes.to_vec();
						if key >= rng.start && key < rng.end {
							pairs.push((key, val_bytes.to_vec()));
							count += 1;
						} else if key >= rng.end {
							break;
						}
					}
					Err(e) => return Err(Error::Ds(e.to_string())),
				}
			}
			pairs.reverse();
			Ok(pairs)
		} else {
			Err(Error::Ds("No active transaction".to_string()))
		}
	}

	async fn getr(
		&mut self,
		rng: std::ops::Range<Key>,
		version: Option<u64>,
	) -> Result<Vec<(Key, Val)>, Error> {
		// MDBX does not support versioned queries.
		if version.is_some() {
			return Err(Error::UnsupportedVersionedQueries);
		}
		if let Some(txn) = self.txn.as_ref() {
			let table: Table = txn.open_table(None).map_err(|e| Error::Ds(e.to_string()))?;
			let mut cursor = txn.cursor(&table).map_err(|e| Error::Ds(e.to_string()))?;
			let mut pairs = Vec::new();

			for item in cursor.iter::<std::borrow::Cow<[u8]>, std::borrow::Cow<[u8]>>() {
				match item {
					Ok((key_bytes, val_bytes)) => {
						let key = key_bytes.to_vec();
						if key >= rng.start && key < rng.end {
							pairs.push((key, val_bytes.to_vec()));
						} else if key >= rng.end {
							break;
						}
					}
					Err(e) => return Err(Error::Ds(e.to_string())),
				}
			}
			Ok(pairs)
		} else {
			Err(Error::Ds("No active transaction".to_string()))
		}
	}

	async fn scan_all_versions(
		&mut self,
		_rng: std::ops::Range<Key>,
		_limit: u32,
	) -> Result<Vec<(Key, Val, crate::kvs::Version, bool)>, Error> {
		Err(Error::UnsupportedVersionedQueries)
	}

	async fn batch_keys(
		&mut self,
		rng: std::ops::Range<Key>,
		batch: u32,
		version: Option<u64>,
	) -> Result<crate::kvs::batch::Batch<Key>, Error> {
		// MDBX does not support versioned queries.
		if version.is_some() {
			return Err(Error::UnsupportedVersionedQueries);
		}
		let keys = self.keys(rng, batch, None).await?;
		Ok(crate::kvs::batch::Batch::new(None, keys))
	}

	async fn batch_keys_vals(
		&mut self,
		rng: std::ops::Range<Key>,
		batch: u32,
		version: Option<u64>,
	) -> Result<crate::kvs::batch::Batch<(Key, Val)>, Error> {
		// MDBX does not support versioned queries.
		if version.is_some() {
			return Err(Error::UnsupportedVersionedQueries);
		}
		let pairs = self.scan(rng, batch, None).await?;
		Ok(crate::kvs::batch::Batch::new(None, pairs))
	}

	async fn save_point_prepare(
		&mut self,
		_key: &Key,
		version: Option<u64>,
		_op: crate::kvs::savepoint::SaveOperation,
	) -> Result<Option<crate::kvs::savepoint::SavePrepare>, Error> {
		// MDBX does not support versioned queries.
		if version.is_some() {
			return Err(Error::UnsupportedVersionedQueries);
		}
		Ok(None)
	}
}

impl Datastore {
	/// Open a new database
	pub async fn new(path: &str) -> Result<Datastore, Error> {
		let db_path = Path::new(path);
		match Database::open(db_path) {
			Ok(db) => Ok(Datastore {
				db: Arc::new(db),
			}),
			Err(e) => Err(Error::Ds(e.to_string())),
		}
	}

	/// Shutdown the database
	pub(crate) async fn shutdown(&self) -> Result<(), Error> {
		Ok(())
	}

	/// Start a new transaction
	pub async fn transaction(
		&self,
		write: bool,
		check: Check,
	) -> Result<Box<dyn kvs_api::Transaction + Send + 'static>, Error> {
		let db = self.db.clone();
		let raw_txn = db.begin_rw_txn().map_err(|e| Error::Ds(e.to_string()))?;
		// SAFETY: We store the Arc<Database> in the Transaction struct, ensuring the database
		// outlives the transaction. This lifetime extension is safe.
		let static_txn: MdbxTxn<'static, RW, WriteMap> = unsafe { std::mem::transmute(raw_txn) };
		Ok(Box::new(Transaction::new(db, static_txn, write, check)))
	}
}

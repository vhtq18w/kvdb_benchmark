extern crate db_key;
extern crate leveldb;
extern crate tempdir;

use db_key::Key;
use leveldb::database::batch::{
    Batch,
    Writebatch,
};
use leveldb::database::Database;
use leveldb::kv::KV;
use leveldb::options::{
    Options,
    ReadOptions,
    WriteOptions,
};
use tempdir::TempDir;

fn main() {
    let dir = TempDir::new("example").unwrap();
    let mut options = Options::new();
    options.create_if_missing = true;
    let database: Database<i32> = Database::open(dir.path(), options).unwrap();

    let key1 = Key::from_u8(b"key1");
    let key2 = Key::from_u8(b"key2");
    let key3 = Key::from_u8(b"key3");

    let batch = &mut Writebatch::new();
    batch.put(key1, b"val1");
    batch.put(key2, b"val2");
    batch.put(key3, b"val3");
    database.write(WriteOptions::new(), batch).unwrap();

    assert_eq!(database.get(ReadOptions::new(), key1).unwrap().unwrap(), b"val1");
    assert_eq!(database.get(ReadOptions::new(), key2).unwrap().unwrap(), b"val2");
    assert_eq!(database.get(ReadOptions::new(), key3).unwrap().unwrap(), b"val3");

    database.delete(WriteOptions::new(), key1).unwrap();
    assert_eq!(database.get(ReadOptions::new(), key1).unwrap(), None);
}

#[macro_use]
extern crate criterion;

#[macro_use]
extern crate lazy_static;

extern crate db_key;
extern crate leveldb;
extern crate rand;
extern crate tempdir;
extern crate walkdir;

use criterion::Criterion;

use self::rand::{
    random,
    thread_rng,
    Rng,
};

use self::walkdir::WalkDir;

use leveldb::database::batch::{
    Batch,
    Writebatch,
};

use leveldb::database::Database;
use leveldb::iterator::Iterable;
use leveldb::kv::KV;

use leveldb::options::{
    Options,
    ReadOptions,
    WriteOptions,
};

use std::{
    mem,
    thread,
    time,
};

use tempdir::TempDir;

// We parameterize benchmarks across both the number of KV pairs we write to
// (or read from) a datastore and the sizes of the values we write (or read).
//
// It might make sense to also parameterize across the sizes of the keys,
// for which we'd need to define a struct that implements the db_key::Key trait,
// since the leveldb crate delegates to the db_key crate to define key types,
// and the only implementation of Key in the db_key crate itself is for u32,
// which always has the same size: four bytes.
//
// (The lmdb crate defines key types as any type that implements AsRef<[u8]>,
// so we don't need to do anything special to parameterize across key sizes
// for that storage engine.)
//
const PAIR_COUNTS: [u32; 3] = [1, 100, 1000];
const VALUE_SIZES: [usize; 3] = [1, 100, 1000];

#[derive(Debug)]
struct Param {
    num_pairs: u32,
    size_values: usize,
}

lazy_static! {
    // A collection of tuples (num_pairs, size_values) representing every
    // combination of numbers of pairs and sizes of values, which we use
    // to benchmark storage engine performance across various shapes of data.
    static ref PARAMS: Vec<Param> =
        PAIR_COUNTS.iter().flat_map(|&m| VALUE_SIZES.iter().map(move |&n| Param { num_pairs: m, size_values: n })).collect();
}

fn get_key(n: u32) -> i32 {
    n as i32
}

fn get_value(size_values: usize) -> Vec<u8> {
    (0..size_values).map(|_| random()).collect()
}

fn get_pair(num_pairs: u32, size_values: usize) -> (i32, Vec<u8>) {
    (get_key(num_pairs), get_value(size_values))
}

fn setup_bench_db(num_pairs: u32, size_values: usize) -> TempDir {
    let dir = TempDir::new("demo").unwrap();

    let mut options = Options::new();
    options.create_if_missing = true;
    let database = Database::open(dir.path(), options).unwrap();

    let batch = &mut Writebatch::new();
    for i in 0..num_pairs {
        batch.put(i as i32, &get_value(size_values));
    }
    let mut write_opts = WriteOptions::new();
    write_opts.sync = true;
    database.write(write_opts, batch).unwrap();

    dir
}

fn bench_open_db(c: &mut Criterion) {
    let dir = TempDir::new("bench_open_db").unwrap();

    // Create the database first so we only measure the time to open
    // an existing database.
    {
        let mut options = Options::new();
        options.create_if_missing = true;
        let _db: Database<i32> = Database::open(dir.path(), options).unwrap();
    }

    c.bench_function("leveldb_open_db", move |b| {
        b.iter(|| {
            let db: Database<i32> = Database::open(dir.path(), Options::new()).unwrap();
            db
        })
    });
}

fn leveldb_put(db: &Database<i32>, pairs: &Vec<(i32, Vec<u8>)>, sync: bool) {
    let mut write_opts = WriteOptions::new();
    // LevelDB writes are async by default.  Set WriteOptions::sync
    // to true to make them sync.
    if sync {
        write_opts.sync = true;
    }
    if pairs.len() == 1 {
        // Optimize the case where we're writing only one value
        // by writing it directly rather than creating a batch.
        db.put(write_opts, *&pairs[0].0, &pairs[0].1).unwrap();
    } else {
        let batch = &mut Writebatch::new();
        for (key, value) in pairs {
            batch.put(*key, value);
        }
        db.write(write_opts, batch).unwrap();
    }
}

fn bench_put_seq_sync(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "leveldb_put_seq_sync",
        |b, ref t| {
            let Param {
                num_pairs,
                size_values,
            } = t;
            let dir = TempDir::new("bench_put_seq").unwrap();
            let path = dir.path();
            let mut options = Options::new();
            options.create_if_missing = true;
            let db: Database<i32> = Database::open(path, options).unwrap();
            let pairs: Vec<(i32, Vec<u8>)> = (0..*num_pairs).map(|n| get_pair(n, *size_values)).collect();

            b.iter(|| leveldb_put(&db, &pairs, true))
        },
        PARAMS.iter(),
    );
}

fn bench_put_seq_async(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "leveldb_put_seq_async",
        |b, ref t| {
            let Param {
                num_pairs,
                size_values,
            } = t;
            let dir = TempDir::new("bench_put_seq").unwrap();
            let path = dir.path();
            let mut options = Options::new();
            options.create_if_missing = true;
            let db: Database<i32> = Database::open(path, options).unwrap();
            let pairs: Vec<(i32, Vec<u8>)> = (0..*num_pairs).map(|n| get_pair(n, *size_values)).collect();

            b.iter(|| leveldb_put(&db, &pairs, false))
        },
        PARAMS.iter(),
    );
}

fn bench_put_rand_sync(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "leveldb_put_rand_sync",
        |b, ref t| {
            let Param {
                num_pairs,
                size_values,
            } = t;
            let dir = TempDir::new("bench_put_rand_sync").unwrap();
            let path = dir.path();
            let mut options = Options::new();
            options.create_if_missing = true;
            let db: Database<i32> = Database::open(path, options).unwrap();
            let mut pairs: Vec<(i32, Vec<u8>)> = (0..*num_pairs).map(|n| get_pair(n, *size_values)).collect();
            thread_rng().shuffle(&mut pairs[..]);

            b.iter(|| leveldb_put(&db, &pairs, true))
        },
        PARAMS.iter(),
    );
}

fn bench_put_rand_async(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "leveldb_put_rand_async",
        |b, ref t| {
            let Param {
                num_pairs,
                size_values,
            } = t;
            let dir = TempDir::new("bench_put_rand_async").unwrap();
            let path = dir.path();
            let mut options = Options::new();
            options.create_if_missing = true;
            let db: Database<i32> = Database::open(path, options).unwrap();
            let mut pairs: Vec<(i32, Vec<u8>)> = (0..*num_pairs).map(|n| get_pair(n, *size_values)).collect();
            thread_rng().shuffle(&mut pairs[..]);

            b.iter(|| leveldb_put(&db, &pairs, false))
        },
        PARAMS.iter(),
    );
}

fn bench_get_seq(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "leveldb_get_seq",
        |b, ref t| {
            let Param {
                num_pairs,
                size_values,
            } = t;
            let dir = setup_bench_db(*num_pairs, *size_values);
            let path = dir.path();
            let options = Options::new();
            let database: Database<i32> = Database::open(path, options).unwrap();
            let keys: Vec<i32> = (0..*num_pairs as i32).collect();

            b.iter(|| {
                let mut i = 0usize;
                for key in &keys {
                    let read_opts = ReadOptions::new();
                    i += database.get(read_opts, key).unwrap().unwrap().len();
                }
                i
            })
        },
        PARAMS.iter(),
    );
}

fn bench_get_rand(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "leveldb_get_rand",
        |b, ref t| {
            let Param {
                num_pairs,
                size_values,
            } = t;
            let dir = setup_bench_db(*num_pairs, *size_values);
            let path = dir.path();
            let options = Options::new();
            let database: Database<i32> = Database::open(path, options).unwrap();
            let mut keys: Vec<i32> = (0..*num_pairs as i32).collect();
            thread_rng().shuffle(&mut keys[..]);

            b.iter(|| {
                let mut i = 0usize;
                for key in &keys {
                    let read_opts = ReadOptions::new();
                    i += database.get(read_opts, key).unwrap().unwrap().len();
                }
                i
            })
        },
        PARAMS.iter(),
    );
}

fn bench_get_seq_iter(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "leveldb_get_seq_iter",
        |b, ref t| {
            let Param {
                num_pairs,
                size_values,
            } = t;
            let dir = setup_bench_db(*num_pairs, *size_values);
            let path = dir.path();
            let options = Options::new();
            let database: Database<i32> = Database::open(path, options).unwrap();
            let mut keys: Vec<i32> = (0..*num_pairs as i32).collect();
            thread_rng().shuffle(&mut keys[..]);

            b.iter(|| {
                let mut i = 0usize;
                let mut count = 0u32;
                let read_opts = ReadOptions::new();
                for (key, data) in database.iter(read_opts) {
                    i += mem::size_of_val(&key) + data.len();
                    count += 1;
                }
                assert_eq!(count, *num_pairs);
                i
            })
        },
        PARAMS.iter(),
    );
}

// This measures space on disk, not time, reflecting the space taken
// by a database on disk into the time it takes the benchmark to complete.
fn bench_db_size(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "leveldb_db_size",
        |b, ref t| {
            let Param {
                num_pairs,
                size_values,
            } = t;
            let dir = setup_bench_db(*num_pairs, *size_values);
            let mut total_size = 0;

            for entry in WalkDir::new(dir.path()) {
                let metadata = entry.unwrap().metadata().unwrap();
                if metadata.is_file() {
                    total_size += metadata.len();
                }
            }

            b.iter(|| {
                // Convert size on disk to benchmark time by sleeping
                // for the total_size number of nanoseconds.
                thread::sleep(time::Duration::from_nanos(total_size));
            })
        },
        PARAMS.iter(),
    );
}

criterion_group!(
    benches,
    bench_open_db,
    bench_put_seq_sync,
    bench_put_seq_async,
    bench_put_rand_sync,
    bench_put_rand_async,
    bench_get_seq,
    bench_get_rand,
    bench_get_seq_iter,
    bench_db_size,
);
criterion_main!(benches);

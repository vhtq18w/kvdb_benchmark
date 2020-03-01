#[macro_use]
extern crate criterion;
extern crate leveldb;
extern crate lmdb;
extern crate tempdir;

use criterion::{
    Criterion,
    Fun,
};

use leveldb::database::Database as LeveldbDatabase;
use leveldb::options::Options;
use lmdb::Environment;
use tempdir::TempDir;

fn cmp_open_db(c: &mut Criterion) {
    let leveldb_dir = TempDir::new("leveldb_bench_open_db").unwrap();

    // Create the database first so we only measure the time to open
    // an existing database.
    {
        let mut options = Options::new();
        options.create_if_missing = true;
        let _db: LeveldbDatabase<i32> = LeveldbDatabase::open(leveldb_dir.path(), options).unwrap();
    }

    let leveldb_fun = Fun::new("leveldb", move |b, _i| {
        b.iter(|| {
            let db: LeveldbDatabase<i32> = LeveldbDatabase::open(leveldb_dir.path(), Options::new()).unwrap();
            db
        })
    });

    let lmdb_dir = TempDir::new("bench_open_db").unwrap();

    // Create the database first so we only measure the time to open
    // an existing database.
    {
        let env = Environment::new().open(lmdb_dir.path()).unwrap();
        let _db = env.open_db(None).unwrap();
    }

    let lmdb_fun = Fun::new("lmdb", move |b, _i| {
        b.iter(|| {
            let env = Environment::new().open(lmdb_dir.path()).unwrap();
            let db = env.open_db(None).unwrap();
            db
        })
    });

    let bench_functions = vec![leveldb_fun, lmdb_fun];
    c.bench_functions("cmp_open_db", bench_functions, ());
}

criterion_group!(benches, cmp_open_db);
criterion_main!(benches);

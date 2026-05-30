# GuiXu Rust

Rust rewrite of the original Kotlin GuiXu embedded database prototype.

## Current Scope

- `GuiXu::new(path)` database initialization.
- `TypedBox<T>` with `serde` + `bincode` serialization.
- `ByteArrayBox` for raw bytes.
- `KVBox` for primitive key/value data.
- Append-only store files with 12-byte index items.
- `clear`, `remove`, `compact`, `get_info`, and `close` APIs.

The Rust `TypedBox<T>` storage is not byte-compatible with Kotlin `kotlinx.serialization.protobuf` yet. It is intentionally implemented with `serde` + `bincode` for the first Rust-native version.

## Example

```rust
use guixu::{impl_store_data, GuiXu, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestClass {
    id: u64,
    name: String,
    age: i32,
}

impl_store_data!(TestClass, id);

fn main() -> Result<()> {
    let db = GuiXu::new("./data")?;
    let mut box_ = db.box_for::<TestClass>()?;

    let mut data = TestClass {
        id: 0,
        name: "Aa".to_string(),
        age: 18,
    };

    let id = box_.put(&mut data)?;
    let loaded = box_.get(id)?;

    println!("{loaded:?}");
    Ok(())
}
```

## Test

Run :

```bash
cd "对应目录"
cargo test
```

Run the basic example:

```bash
cargo run --example basic
```

## Benchmark

Run the Criterion benchmark suite:

```bash
cargo bench --bench guixu_bench
```

You can control the sample size:

```bash
GUIXU_BENCH_SAMPLE_SIZE=10 cargo bench --bench guixu_bench
```

For a quick throughput-style smoke test, the older example benchmark still exists:

```bash
cargo run --release --example benchmark -- 1000000 /tmp/guixu-bench
```

For more stable numbers, prefer a Linux filesystem path such as `/tmp/...` inside WSL instead of `/mnt/c/...`, because Windows-mounted paths are usually slower for file I/O.

### Current 10M Operation Results

Environment used for the latest comparison:

- OS: Windows
- Branch: `rust-rewrite`
- Command:

```bash
cargo run --release --example benchmark -- 10000000 D:\code\AHUTong\GuiXu-Rust\target\benchmark-db-10m-local5
```

Rust results:

| Box | Operation | Total time | Time/op | Throughput |
| --- | --- | ---: | ---: | ---: |
| `KVBox<String>` | create | 3922.741 ms | 0.392 us/op | 2,549,238 ops/s |
| `KVBox<String>` | update | 3410.147 ms | 0.341 us/op | 2,932,425 ops/s |
| `KVBox<String>` | read | 2575.363 ms | 0.258 us/op | 3,882,948 ops/s |
| `KVBox<String>` | remove | 2848.125 ms | 0.285 us/op | 3,511,082 ops/s |
| `ByteArrayBox` | create | 539.633 ms | 0.054 us/op | 18,531,123 ops/s |
| `ByteArrayBox` | update | 446.572 ms | 0.045 us/op | 22,392,826 ops/s |
| `ByteArrayBox` | read | 337.557 ms | 0.034 us/op | 29,624,591 ops/s |
| `ByteArrayBox` | remove | 36.245 ms | 0.004 us/op | 275,898,602 ops/s |
| `TypedBox<TestClass>` | create | 1283.498 ms | 0.128 us/op | 7,791,206 ops/s |
| `TypedBox<TestClass>` | update | 1118.851 ms | 0.112 us/op | 8,937,738 ops/s |
| `TypedBox<TestClass>` | read | 807.734 ms | 0.081 us/op | 12,380,315 ops/s |
| `TypedBox<TestClass>` | remove | 34.832 ms | 0.003 us/op | 287,089,856 ops/s |

Kotlin `main` branch comparison for the currently enabled `KVBox` test with `count = 10_000_000`:

| Operation | Kotlin main | Rust rewrite |
| --- | ---: | ---: |
| create | 2229 ms | 3922.741 ms |
| update | 1374 ms | 3410.147 ms |
| read | 593 ms | 2575.363 ms |
| remove | 906 ms | 2848.125 ms |

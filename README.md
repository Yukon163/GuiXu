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
    let box_ = db.box_for::<TestClass>()?;

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

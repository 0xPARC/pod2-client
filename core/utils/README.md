# Pod Utils

Shared utilities for working with pod2 across the ParcNet ecosystem.

## ValueExt Trait

The `ValueExt` trait provides convenient methods for extracting typed values from `pod2::middleware::Value` objects:

```rust
use pod_utils::ValueExt;

// Extract basic types
let string_val = pod.get("username").and_then(|v| v.as_str());
let int_val = pod.get("timestamp").and_then(|v| v.as_i64());
let bool_val = pod.get("verified").and_then(|v| v.as_bool());

// Extract complex types
let public_key = pod.get("_signer").and_then(|v| v.as_public_key());
let array = pod.get("data").and_then(|v| v.as_array());
let dict = pod.get("metadata").and_then(|v| v.as_dictionary());
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
pod-utils = { path = "../pod-utils" }
```

## Supported Types

- `as_i64()` - Extract `i64` values
- `as_str()` - Extract string values  
- `as_bool()` - Extract boolean values
- `as_public_key()` - Extract `PublicKey` values
- `as_set()` - Extract `Set` containers
- `as_dictionary()` - Extract `Dictionary` containers
- `as_array()` - Extract `Array` containers
- `as_raw()` - Extract `RawValue` types
- `as_pod_id()` - Extract `PodId` values
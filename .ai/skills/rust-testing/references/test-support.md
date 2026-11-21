# Rust test support

## A fake and a stub at one port

Use the port the application already needs. In this library example the fake
stores stock by SKU, while the stub forces a service error through the same seam.

```rust
use std::sync::Arc;

pub trait Stock: Send + Sync {
    fn count(&self, sku: &str) -> Result<u32, Unavailable>;
}

#[derive(Debug, PartialEq, Eq)]
pub struct Unavailable;

pub fn in_stock(stock: Arc<dyn Stock>, sku: &str) -> Result<bool, Unavailable> {
    stock.count(sku).map(|count| count > 0)
}

#[cfg(any(test, feature = "test-util"))]
pub mod test_support {
    use super::{Stock, Unavailable};
    use std::collections::HashMap;

    #[derive(Default)]
    pub struct FakeStock(HashMap<String, u32>);

    impl FakeStock {
        pub fn set(&mut self, sku: &str, count: u32) {
            self.0.insert(sku.into(), count);
        }
    }

    impl Stock for FakeStock {
        fn count(&self, sku: &str) -> Result<u32, Unavailable> {
            Ok(self.0.get(sku).copied().unwrap_or_default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UnavailableStock;

    impl Stock for UnavailableStock {
        fn count(&self, _: &str) -> Result<u32, Unavailable> {
            Err(Unavailable)
        }
    }

    #[test]
    fn distinguishes_available_missing_and_unavailable_stock() {
        let mut fake = test_support::FakeStock::default();
        fake.set("A", 2);
        let stock: Arc<dyn Stock> = Arc::new(fake);
        assert_eq!(in_stock(Arc::clone(&stock), "A"), Ok(true));
        assert_eq!(in_stock(stock, "B"), Ok(false));
        assert_eq!(in_stock(Arc::new(UnavailableStock), "A"), Err(Unavailable));
    }
}
```

If sending a replenishment event is part of the contract, inject a publisher
that records events and assert their payloads and count after the action.

## Producer and consumer setup

- For a crate-internal fake, use `#[cfg(test)]`. To import that fake from another
  crate, including the package's own integration test crate, expose it through
  the public feature-enabled module. The linked library is compiled without
  `cfg(test)`.
  Define `test-util = []` under the producer's `[features]` and keep it out of
  `default`. Consumers can add `stock-client = { version = "1", features =
  ["test-util"] }` under `[dev-dependencies]`, using the same version as their
  normal dependency. If public helpers need extra crates, declare optional
  normal dependencies and activate them through the feature. Dev-dependencies
  are for the package's own test targets.
- Run common contract cases against fresh real and fake instances, including
  missing keys and supported state changes. Keep setup specific to each backend.
- For HTTP, publish versioned fixtures with method, path, status, relevant
  headers, and body. Check representative requests against the real producer
  handler and validate the responses against the fixtures or published schema.
  Include representative errors. In the consumer, match the outgoing request
  and return the corresponding fixture through a fake HTTP server or transport.

## Release and lint checks

- Enable only runtime features in normal dependencies. For example, add
  `test-util` to the Tokio dev-dependency with the same version requirement as
  the normal dependency. Use `#[tokio::test(start_paused = true)]` on its
  current-thread runtime for timeouts and retries, and advance virtual time
  when the test needs an explicit clock step.
- With Cargo resolver 2 or 3, dev-dependency features can unify during test
  builds. Other normal dependency paths can also enable those features in a
  production build. Inspect the production graph with
  `cargo tree -e features,no-dev -i tokio`, or the equivalent inverse query for
  the producer crate. Use the release target and feature flags in that query.
  Confirm with a fresh `cargo build --release -vv` using the actual shipping
  invocation and inspect the relevant rustc `--cfg` feature flags. Use a
  separate target directory if cached builds hide the commands. `--release`
  selects a profile, so explicit test features can still enter a release build.
- Gate test-only items and their imports together. A private helper called only
  by tests can be dead in the normal target. `cfg_attr` with `expect(dead_code)`
  reports an unfulfilled expectation in targets where the helper is used.
  Prefer matching `cfg` gates. If the item must remain, scope
  `allow(dead_code)` to that item and configuration, with a short reason.
- Lint the default build and each supported feature configuration. For a single
  opt-in feature, run both commands below and exercise both with
  `cargo test --all-targets` using the corresponding feature flags. Use
  `--all-features` when that combination is supported and check other shipped
  combinations separately.

```sh
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features test-util -- -D warnings
```

Sources: [Cargo targets](https://doc.rust-lang.org/cargo/reference/cargo-targets.html),
[Cargo feature resolution](https://doc.rust-lang.org/cargo/reference/features.html#feature-resolver-version-2),
[Cargo dependency graphs](https://doc.rust-lang.org/cargo/commands/cargo-tree.html), and
[rustc lint expectations](https://doc.rust-lang.org/rustc/lints/levels.html#expect).

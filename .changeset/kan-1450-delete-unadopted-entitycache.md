---
bump: minor
---

service: remove the unadopted `EntityCache`, deleting the `pub mod cache` module and the `pub use cache::EntityCache` re-export. The module was built with a full test suite but was never wired to anything, and the topology it belonged to was retired when the Model became the single store. Because it removes a public item from a published crate, this is a breaking change for out-of-tree callers and takes a `minor` bump under the pre-1.0 policy, even though nothing in this workspace imported it.

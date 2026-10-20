// Suggestion: use `cargo doc --open` to view types
#![allow(unused, warnings)]
#![doc = "Vendored SolarXR FlatBuffers bindings (upstream SlimeVR/SolarXR-Protocol, commit 00c38a6)."]

mod generated;

pub use generated::solarxr_protocol::*;

pub use ::flatbuffers;

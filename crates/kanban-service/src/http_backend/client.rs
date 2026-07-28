//! HTTP request helpers for the HTTP backend.
//! This module provides utilities for building requests, sending them,
//! deserializing responses, and mapping errors.
//!
//! This is currently a minimal stub; real request/response handling
//! will be added in later cards as actual HTTP operations are implemented.

#[allow(dead_code)]
pub fn build_url(base_url: &str, path: &str) -> String {
    format!("{}{}", base_url, path)
}


//! library to `gerrit-rust` tool

#[macro_use] extern crate quick_error;
extern crate curl;
extern crate git2;
extern crate rustc_serialize;
extern crate url;

pub mod call;
pub mod entities;
pub mod error;
pub mod gerrit;


// Copyright 2019 PingCAP, Inc.

//! Utility functions for generating Rust code from protobuf specifications.
//!
//! These functions panic liberally, they are designed to be used from build
//! scripts, not in production.

#[cfg(feature = "prost-codec")]
mod wrapper;

#[cfg(feature = "protobuf-codec")]
mod protobuf_impl;

#[cfg(feature = "prost-codec")]
mod prost_impl;

use bitflags::bitflags;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Builder {
    files: Vec<String>,
    includes: Vec<String>,
    black_list: Vec<String>,
    out_dir: String,
    #[allow(dead_code)]
    wrapper_opts: GenOpt,
}

impl Builder {
    pub fn new() -> Builder {
        Builder {
            files: Vec::new(),
            includes: vec!["include".to_owned(), "proto".to_owned()],
            black_list: vec![
                "protobuf".to_owned(),
                "google".to_owned(),
                "gogoproto".to_owned(),
            ],
            out_dir: format!(
                "{}/protos",
                std::env::var("OUT_DIR").expect("No OUT_DIR defined")
            ),
            wrapper_opts: GenOpt::all(),
        }
    }

    pub fn generate(&self) {
        assert!(!self.files.is_empty(), "No files specified for generation");
        self.prep_out_dir();
        self.generate_files();
        self.generate_mod_file();
    }

    #[cfg(feature = "prost-codec")]
    pub fn wrapper_options(&mut self, wrapper_opts: GenOpt) -> &mut Self {
        self.wrapper_opts = wrapper_opts;
        self
    }

    /// Finds proto files to operate on in the `proto_dir` directory.
    pub fn search_dir_for_protos(&mut self, proto_dir: &str) -> &mut Self {
        self.files = fs::read_dir(proto_dir)
            .expect("Couldn't read proto directory")
            .filter_map(|e| {
                let e = e.expect("Couldn't list file");
                if e.file_type().expect("File broken").is_dir() {
                    None
                } else {
                    Some(format!("{}/{}", proto_dir, e.file_name().to_string_lossy()))
                }
            })
            .collect();
        self
    }

    pub fn files<T: ToString>(&mut self, files: &[T]) -> &mut Self {
        self.files = files.iter().map(|t| t.to_string()).collect();
        self
    }

    pub fn includes<T: ToString>(&mut self, includes: &[T]) -> &mut Self {
        self.includes = includes.iter().map(|t| t.to_string()).collect();
        self
    }

    pub fn append_include(&mut self, include: impl Into<String>) -> &mut Self {
        self.includes.push(include.into());
        self
    }

    pub fn black_list<T: ToString>(&mut self, black_list: &[T]) -> &mut Self {
        self.black_list = black_list.iter().map(|t| t.to_string()).collect();
        self
    }

    /// Add the name of an include file to the builder's black list.
    ///
    /// Files named on the black list are not made modules of the generated
    /// program.
    pub fn append_to_black_list(&mut self, include: impl Into<String>) -> &mut Self {
        self.black_list.push(include.into());
        self
    }

    pub fn out_dir(&mut self, out_dir: impl Into<String>) -> &mut Self {
        self.out_dir = out_dir.into();
        self
    }

    fn generate_mod_file(&self) {
        let mut f = File::create(format!("{}/mod.rs", self.out_dir)).unwrap();

        let modules = self.list_rs_files().filter_map(|path| {
            let name = path.file_stem().unwrap().to_str().unwrap();
            if name.starts_with("wrapper_")
                || name == "mod"
                || self.black_list.iter().any(|i| name.contains(i))
            {
                return None;
            }
            Some((name.replace('-', "_"), name.to_owned()))
        });

        for (module, file_name) in modules {
            if cfg!(feature = "protobuf-codec") {
                writeln!(f, "pub mod {};", module).unwrap();
                continue;
            }

            let mut level = 0;
            for part in module.split('.') {
                writeln!(f, "pub mod {} {{", part).unwrap();
                level += 1;
            }
            writeln!(f, "include!(\"{}.rs\");", file_name,).unwrap();
            if Path::new(&format!("{}/wrapper_{}.rs", self.out_dir, file_name)).exists() {
                writeln!(f, "include!(\"wrapper_{}.rs\");", file_name,).unwrap();
            }
            writeln!(f, "{}", "}\n".repeat(level)).unwrap();
        }
    }

    fn prep_out_dir(&self) {
        if Path::new(&self.out_dir).exists() {
            fs::remove_dir_all(&self.out_dir).unwrap();
        }
        fs::create_dir_all(&self.out_dir).unwrap();
    }

    // List all `.rs` files in `self.out_dir`.
    fn list_rs_files(&self) -> impl Iterator<Item = PathBuf> {
        fs::read_dir(&self.out_dir)
            .expect("Couldn't read directory")
            .filter_map(|e| {
                let path = e.expect("Couldn't list file").path();
                if path.extension() == Some(std::ffi::OsStr::new("rs")) {
                    Some(path)
                } else {
                    None
                }
            })
    }
}

impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}

bitflags! {
    pub struct GenOpt: u32 {
        /// Generate implementation for trait `::protobuf::Message`.
        const MESSAGE = 0b0000_0001;
        /// Generate getters.
        const TRIVIAL_GET = 0b0000_0010;
        /// Generate setters.
        const TRIVIAL_SET = 0b0000_0100;
        /// Generate the `new_` constructors.
        const NEW = 0b0000_1000;
        /// Generate `clear_*` functions.
        const CLEAR = 0b0001_0000;
        /// Generate `has_*` functions.
        const HAS = 0b0010_0000;
        /// Generate mutable getters.
        const MUT = 0b0100_0000;
        /// Generate `take_*` functions.
        const TAKE = 0b1000_0000;
        /// Except `impl protobuf::Message`.
        const NO_MSG = Self::TRIVIAL_GET.bits
         | Self::TRIVIAL_SET.bits
         | Self::CLEAR.bits
         | Self::HAS.bits
         | Self::MUT.bits
         | Self::TAKE.bits;
        /// Except `new_` and `impl protobuf::Message`.
        const ACCESSOR = Self::TRIVIAL_GET.bits
         | Self::TRIVIAL_SET.bits
         | Self::MUT.bits
         | Self::TAKE.bits;
    }
}

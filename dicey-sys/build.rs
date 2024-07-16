// Copyright (c) 2014-2024 Zuru Tech HK Limited, All rights reserved.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let dicey_path = env::var("DICEY_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let curdir = env::var("CARGO_MANIFEST_DIR").unwrap();

            Path::new(&curdir).join("libdicey")
        });

    let libdir = dicey_path.join("lib");
    let incdir = dicey_path.join("include");

    println!("cargo:rerun-if-env-changed=DICEY_PATH");

    println!("cargo:rustc-link-search={}", libdir.display());
    println!("cargo:rustc-link-lib=dicey");
    println!("cargo:rustc-link-lib=uv");

    let hpath = incdir.join("dicey").join("dicey.h");

    let bindings = bindgen::Builder::default()
        .clang_arg(format!("-I{}", incdir.display()))
        .header(hpath.to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

pub fn build_zlib_ng(target: &str, compat: bool) {
    let mut cmake = cmake::Config::new("src/zlib-ng");
    
    cmake
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("ZLIB_COMPAT", if compat { "ON" } else { "OFF" })
        .define("ZLIB_ENABLE_TESTS", "OFF")
        .define("WITH_GZFILEOP", "ON");

    let install_dir = cmake.build();

    let includedir = install_dir.join("include");
    let libdir = install_dir.join("lib");
    let libdir64 = install_dir.join("lib64");

    println!(
        "cargo:rustc-link-search=native={}",
        libdir.to_str().unwrap()
    );

    println!(
        "cargo:rustc-link-search=native={}",
        libdir64.to_str().unwrap()
    );

    let mut debug_suffix = "";

    let libname = if target.contains("windows") && target.contains("msvc") {
        if env::var("OPT_LEVEL").unwrap() == "0" {
            debug_suffix = "d";
        }
        "zlibstatic"
    } else {
        "z"
    };

    println!(
        "cargo:rustc-link-lib=static={}{}{}",
        libname,
        if compat { "" } else { "-ng" },
        debug_suffix,
    );

    println!("cargo:root={}", install_dir.to_str().unwrap());
    println!("cargo:include={}", includedir.to_str().unwrap());

    if !compat {
        println!("cargo:rustc-cfg=zng");
    }
}

/*
 * Copyright (c) 2014-2024 Zuru Tech HK Limited, All rights reserved.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::{
    env, fmt,
    ops::Deref,
    path::{Path, PathBuf},
};

#[derive(Debug)]
struct IncDir(PathBuf);

impl fmt::Display for IncDir {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.display().fmt(f)
    }
}

impl Deref for IncDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn discover_explicit() -> Option<IncDir> {
    env::var("DICEY_PATH")
        .map(PathBuf::from)
        .ok()
        .map(|dicey_path| {
            let libdir = dicey_path.join("lib");

            assert!(
                libdir.exists(),
                "DICEY_PATH does not contain a lib directory"
            );

            let incdir = dicey_path.join("include");

            assert!(
                incdir.exists(),
                "DICEY_PATH does not contain an include directory"
            );

            println!("cargo:rerun-if-env-changed=DICEY_PATH");

            println!("cargo:rustc-link-search={}", libdir.display());
            println!("cargo:rustc-link-lib=dicey");
            println!("cargo:rustc-link-lib=uv");

            IncDir(incdir)
        })
}

fn discover_pkgconfig() -> Option<IncDir> {
    pkg_config::Config::new()
        .atleast_version("0.3.9")
        .statik(cfg!(feature = "static"))
        .probe("dicey")
        .ok()
        .map(|mut lib| {
            println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
            println!("cargo:rerun-if-changed=dicey.pc");

            assert!(lib.include_paths.len() == 1);

            IncDir(lib.include_paths.remove(0))
        })
}

fn is_release() -> bool {
    env::var("PROFILE").unwrap() == "release"
}

fn build_dicey() -> Option<IncDir> {
    let mut cmake = cmake::Config::new("src/libdicey");

    cmake
        .define(
            "CMAKE_BUILD_TYPE",
            if is_release() { "Release" } else { "Debug" },
        )
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("BUILD_SAMPLES", "OFF");

    let install_dir = cmake.build();

    let includedir = install_dir.join("include");
    let libdir = install_dir.join("lib");

    println!("cargo:rustc-link-search=native={}", libdir.display());

    println!("cargo:rustc-link-lib=static=dicey");

    println!("cargo:root={}", install_dir.display());
    println!("cargo:include={}", includedir.display());

    Some(IncDir(includedir))
}

fn main() {
    let incdir = discover_explicit()
        .or_else(discover_pkgconfig)
        .or_else(build_dicey)
        .unwrap();

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

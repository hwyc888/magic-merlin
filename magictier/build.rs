use std::path::PathBuf;

#[cfg(target_os = "windows")]
use std::{env, fs};

#[cfg(target_os = "windows")]
struct WindowsBuild {}

#[cfg(target_os = "windows")]
impl WindowsBuild {
    fn copy_dir_all(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if ty.is_dir() {
                Self::copy_dir_all(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }
        Ok(())
    }

    fn check_protoc_exist() -> Option<PathBuf> {
        let path = env::var_os("PROTOC").map(PathBuf::from);
        if path.is_some() && path.as_ref().is_some_and(|p| p.exists()) {
            return path;
        }

        let path = env::var_os("PATH").unwrap_or_default();
        for p in env::split_paths(&path) {
            let p = p.join("protoc.exe");
            if p.exists() && p.is_file() {
                return Some(p);
            }
        }

        None
    }

    pub fn check_for_win() {
        // add third_party dir to link search path
        let target = std::env::var("TARGET").unwrap_or_default();

        let arch_dir = if target.contains("x86_64") {
            Some("x86_64")
        } else if target.contains("i686") {
            Some("i686")
        } else if target.contains("aarch64") {
            Some("arm64")
        } else {
            None
        };
        if let Some(arch_dir) = arch_dir {
            // Cargo runs this script in the package directory, not its parent.
            // Use an absolute path so both --manifest-path and cd builds work.
            let native_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
                .join("third_party")
                .join(arch_dir);
            assert!(
                native_dir.join("Packet.lib").is_file(),
                "Packet.lib missing in {}",
                native_dir.display()
            );
            println!("cargo:rustc-link-search=native={}", native_dir.display());
            println!("cargo:rerun-if-changed={}", native_dir.display());
        }

        let protoc_path = if let Some(o) = Self::check_protoc_exist() {
            println!("cargo:info=use os exist protoc: {:?}", o);
            o
        } else if let Ok(v) = protoc_bin_vendored::protoc_bin_path() {
            println!("cargo:info=use vendored protoc: {:?}", v);
            v
        } else {
            panic!(
                "protoc.exe not found. Set PROTOC or install protoc-bin-vendored. \
                 Network download fallback has been disabled."
            )
        };
        std::env::set_var("PROTOC", protoc_path);

        // When using an unpacked protoc binary, also expose its bundled include dir
        // so imports like google/protobuf/timestamp.proto can be resolved.
        if let Some(include_dir) = std::env::var_os("PROTOC")
            .map(PathBuf::from)
            .and_then(|p| p.parent().map(PathBuf::from))
            .and_then(|p| p.parent().map(PathBuf::from))
            .map(|p| p.join("include"))
        {
            if include_dir.exists() {
                // protoc may fail to resolve imports when include path contains non-ASCII chars.
                // Copy include files to a temp ASCII path and use it preferentially.
                let temp_include_dir = std::env::temp_dir().join("magictier_protobuf_include");
                if !temp_include_dir.exists() {
                    if let Err(e) = Self::copy_dir_all(&include_dir, &temp_include_dir) {
                        println!(
                            "cargo:warning=Failed to copy protoc include dir to temp path: {}",
                            e
                        );
                    }
                }

                if temp_include_dir.exists() {
                    std::env::set_var("PROTOC_INCLUDE", temp_include_dir);
                } else {
                    std::env::set_var("PROTOC_INCLUDE", include_dir);
                }
            }
        }
    }
}

fn workdir() -> Option<String> {
    if let Ok(cargo_manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        return Some(cargo_manifest_dir);
    }

    let dest = std::env::var("OUT_DIR");
    if dest.is_err() {
        return None;
    }
    let dest = dest.unwrap();

    let seperator = regex::Regex::new(r"(/target/(.+?)/build/)|(\\target\\(.+?)\\build\\)")
        .expect("Invalid regex");
    let parts = seperator.split(dest.as_str()).collect::<Vec<_>>();

    if parts.len() >= 2 {
        return Some(parts[0].to_string());
    }

    None
}

fn check_locale() {
    let workdir = workdir().unwrap_or("./".to_string());

    let locale_path = format!("{workdir}/**/locales/**/*");
    if let Ok(globs) = globwalk::glob(locale_path) {
        for entry in globs {
            if let Err(e) = entry {
                println!("cargo:i18n-error={e}");
                continue;
            }

            let entry = entry.unwrap().into_path();
            println!("cargo:rerun-if-changed={}", entry.display());
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    WindowsBuild::check_for_win();

    let proto_files_reflect = ["src/proto/peer_rpc.proto", "src/proto/common.proto"];

    let proto_files = [
        "src/proto/error.proto",
        "src/proto/tests.proto",
        "src/proto/api_instance.proto",
        "src/proto/api_logger.proto",
        "src/proto/api_config.proto",
        "src/proto/api_manage.proto",
        "src/proto/web.proto",
        "src/proto/magic_dns.proto",
        "src/proto/acl.proto",
    ];

    for proto_file in proto_files.iter().chain(proto_files_reflect.iter()) {
        println!("cargo:rerun-if-changed={proto_file}");
    }

    let mut config = prost_build::Config::new();
    config
        .protoc_arg("--experimental_allow_proto3_optional")
        .type_attribute(".acl", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(".common", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(".error", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(".api", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(".web", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(".config", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(
            "peer_rpc.GetIpListResponse",
            "#[derive(serde::Serialize, serde::Deserialize)]",
        )
        .type_attribute("peer_rpc.DirectConnectedPeerInfo", "#[derive(Hash)]")
        .type_attribute("peer_rpc.PeerInfoForGlobalMap", "#[derive(Hash)]")
        .type_attribute("peer_rpc.ForeignNetworkRouteInfoKey", "#[derive(Hash, Eq)]")
        .type_attribute(
            "peer_rpc.RouteForeignNetworkSummary.Info",
            "#[derive(Hash, Eq, serde::Serialize, serde::Deserialize)]",
        )
        .type_attribute(
            "peer_rpc.RouteForeignNetworkSummary",
            "#[derive(Hash, Eq, serde::Serialize, serde::Deserialize)]",
        )
        .type_attribute("common.RpcDescriptor", "#[derive(Hash, Eq)]")
        .field_attribute(".api.manage.NetworkConfig", "#[serde(default)]")
        .service_generator(Box::new(rpc_build::ServiceGenerator::new()))
        .btree_map(["."])
        .skip_debug([".common.Ipv4Addr", ".common.Ipv6Addr", ".common.UUID"]);

    // First pass: generate normal prost types.
    config.compile_protos(&proto_files, &["src/proto/"])?;

    // Second pass: generate ReflectMessage with descriptor set written to an ASCII temp path,
    // then copy it back to OUT_DIR for include_bytes!(concat!(env!("OUT_DIR"), ...)).
    let descriptor_tmp = std::env::temp_dir().join("magictier_file_descriptor_set.bin");
    let mut reflect_builder = prost_reflect_build::Builder::new();
    reflect_builder
        .file_descriptor_set_path(descriptor_tmp.clone())
        .file_descriptor_set_bytes("crate::proto::DESCRIPTOR_POOL_BYTES")
        .compile_protos_with_config(config, &proto_files_reflect, &["src/proto/"])?;

    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    std::fs::create_dir_all(&out_dir)?;
    std::fs::copy(descriptor_tmp, out_dir.join("file_descriptor_set.bin"))?;

    check_locale();
    Ok(())
}

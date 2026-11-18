use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

fn main() {
    glib_build_tools::compile_resources(
        &["data/resources"],
        "data/resources/resources.gresource.xml",
        "resources.gresource",
    );

    // Meson installs the schema; plain `cargo run` needs its own compiled copy
    println!("cargo:rerun-if-env-changed=MIRAGE_APP_ID");
    if env::var_os("MIRAGE_APP_ID").is_none() {
        compile_devel_schema();
    }
}

fn compile_devel_schema() {
    let template = "data/tech.vitainside.mirage.gschema.xml.in";
    println!("cargo:rerun-if-changed={template}");

    let app_id = "tech.vitainside.mirage.Devel";
    let schema = fs::read_to_string(template)
        .unwrap()
        .replace("@app-id@", app_id)
        .replace("@gschema-path@", &format!("/{}/", app_id.replace('.', "/")))
        .replace("@gettext-package@", "mirage");

    let dir = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("schemas");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{app_id}.gschema.xml")), schema).unwrap();

    match Command::new("glib-compile-schemas")
        .arg("--strict")
        .arg(&dir)
        .status()
    {
        Ok(status) if status.success() => {
            println!("cargo:rustc-env=MIRAGE_SCHEMA_DIR={}", dir.display());
        }
        Ok(status) => panic!("glib-compile-schemas failed: {status}"),
        Err(error) => println!("cargo:warning=Could not run glib-compile-schemas: {error}"),
    }
}

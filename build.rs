fn main() {
    embuild::espidf::sysenv::output();
    cxx_build::bridge("src/main.rs")
        .file("src/meshcore/MeshCore.h")
        .compile("cxx-demo");

    println!("cargo:rerun-if-changed=src/main.rs");
    println!("cargo:rerun-if-changed=src/meshcore");
}

fn main() {
    println!("cargo:rerun-if-changed=helper.rc");
    println!("cargo:rerun-if-changed=helper.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("helper.rc", embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}

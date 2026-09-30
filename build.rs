fn main() {
    println!("cargo:rerun-if-changed=assets/prntscrape.rc");
    println!("cargo:rerun-if-changed=assets/prntscrape.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("assets/prntscrape.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("embed Windows application icon");
    }
}

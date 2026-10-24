//! The web app is embedded from `web/dist`. If the frontend hasn't been built, write a placeholder
//! page so the Rust build still works and the server explains what to do instead of showing nothing.

fn main() {
    let dist = std::path::Path::new("../../web/dist");
    println!("cargo:rerun-if-changed=../../web/dist");
    if !dist.join("index.html").exists() {
        std::fs::create_dir_all(dist).expect("create web/dist");
        std::fs::write(
            dist.join("index.html"),
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>Learning tool</title></head>\
             <body><p>Frontend not built: run <code>pnpm --dir web install && pnpm --dir web build</code>, \
             then rebuild learning-tool.</p></body></html>\n",
        )
        .expect("write placeholder index.html");
    }
}

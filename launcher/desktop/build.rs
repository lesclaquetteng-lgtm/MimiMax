use std::path::PathBuf;

fn main() {
	// The core is embedded in the shell, so that a portable build stays a single file. Building
	// the shell before the core would embed a stale binary, and that defect is silent: the two
	// parts would simply disagree about the features they support.
	let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("the manifest directory"));
	let core = manifest.join("..").join("target").join("release").join("aether-launcher.exe");

	if !core.is_file() {
		panic!(
			"the launcher core is missing: build it first with `cargo build --release` in {}",
			manifest.join("..").display()
		);
	}

	// The installer carries the version of the core it was built with.
	println!("cargo:rerun-if-changed={}", core.display());
	println!("cargo:rerun-if-changed=tauri.conf.json");
	for asset in [
		"../gui/src/ui/index.html",
		"../gui/src/ui/app.js",
		"../gui/src/ui/bridge.js",
		"../gui/src/ui/styles.css",
		"../gui/src/ui/tokens.css",
	] {
		println!("cargo:rerun-if-changed={asset}");
	}

	tauri_build::build();
}

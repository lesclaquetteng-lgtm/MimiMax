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

	// The mod is embedded too: an installed launcher has to be able to repair its own installation
	// on a machine that never saw the sources of the project.
	let client = manifest
		.join("..")
		.join("..")
		.join("aether-client")
		.join("build")
		.join("libs")
		.join("aether-client-1.0.0.jar");

	if !client.is_file() {
		panic!(
			"the client jar is missing: build it first with `gradle build` in {}",
			manifest.join("..").join("..").join("aether-client").display()
		);
	}

	// The installer carries the versions of the core and of the mod it was built with.
	println!("cargo:rerun-if-changed={}", core.display());
	println!("cargo:rerun-if-changed={}", client.display());
	for asset in [
		"tauri.conf.json",
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

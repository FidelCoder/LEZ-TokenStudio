fn main() {
    if std::env::var_os("RUSTC_WRAPPER").is_some()
        || std::env::var_os("RUSTC_WORKSPACE_WRAPPER").is_some()
    {
        // SAFETY: Cargo build scripts execute single-threaded before spawning guest Cargo.
        unsafe {
            std::env::remove_var("RUSTC_WRAPPER");
            std::env::remove_var("RUSTC_WORKSPACE_WRAPPER");
        }
    }

    risc0_build::embed_methods();
}

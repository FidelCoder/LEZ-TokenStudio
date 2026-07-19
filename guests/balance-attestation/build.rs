fn main() {
    if std::env::var_os("RUSTC_WRAPPER").is_some()
        || std::env::var_os("RUSTC_WORKSPACE_WRAPPER").is_some()
    {
        // SAFETY: Cargo build scripts run this function single-threaded. The
        // mutation affects only this process and the nested guest Cargo child.
        unsafe {
            std::env::remove_var("RUSTC_WRAPPER");
            std::env::remove_var("RUSTC_WORKSPACE_WRAPPER");
        }
    }

    risc0_build::embed_methods();
}

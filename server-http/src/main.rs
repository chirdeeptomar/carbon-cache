// Standalone HTTP server binary — legacy, kept for dhat heap profiling only.
// Production deployments use carbon-server which includes Raft.
fn main() {
    eprintln!("Use carbon-server for production. This standalone binary is not maintained.");
    std::process::exit(1);
}

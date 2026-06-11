// Standalone TCP server binary — for local development only.
// Production deployments use carbon-server which includes Raft.
// This binary is not maintained for cluster use.
fn main() {
    eprintln!("Use carbon-server for production. This standalone binary is not supported.");
    std::process::exit(1);
}

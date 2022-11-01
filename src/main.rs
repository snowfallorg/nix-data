#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    println!("{:#?}", nix_data::cache::flakes::unavailablepkgs(&["/etc/nixos/configuration.nix"]).await);
}
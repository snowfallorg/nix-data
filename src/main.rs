use nix_data::cache;

#[tokio::main]
async fn main() {
    println!("{:?}", cache::nixos::nixospkgs().await);
}

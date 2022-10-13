use nix_data::cache;

#[tokio::main]
async fn main() {
    println!("{:?}", cache::profile::getprofilepkgs_versioned().await);
}

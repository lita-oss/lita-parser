mod main_impl;

use main_impl::{Error, main_impl};

#[tokio::main]
async fn main() -> Result<(), Error> {
    use tokio::signal::ctrl_c;

    let signal = ctrl_c();

    tokio::select! {
        res = main_impl() => res?,
        res = signal => res?,
    }

    Ok(())
}

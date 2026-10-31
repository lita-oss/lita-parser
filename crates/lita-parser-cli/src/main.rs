mod main_impl;
use main_impl::Error;
use main_impl::main_impl;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let signal = tokio::signal::ctrl_c();

    tokio::select! {
        res = main_impl() => res?,
        res = signal => res?,
    }

    Ok(())
}

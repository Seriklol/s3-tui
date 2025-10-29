mod app;
mod app_state;
mod s3;
mod widgets;
pub mod utils;

use crate::app::App;

async fn main_async() -> color_eyre::Result<()> {
    App::default().run().await
}

fn main() -> color_eyre::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_stack_size(8 * 1024 * 1024)
        .enable_all()
        .build()?;

    let res = runtime.block_on(main_async());
    ratatui::restore();
    res
}

use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::{Builder, Handle, Runtime};

static TOKIO_RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn build_runtime() -> Runtime {
    Builder::new_multi_thread()
        .enable_all()
        .thread_name("vertex-tokio")
        .build()
        .expect("failed to build tokio runtime")
}

fn runtime() -> &'static Runtime {
    TOKIO_RUNTIME.get_or_init(build_runtime)
}

pub fn init() {
    let _ = runtime();
}

pub fn handle() -> &'static Handle {
    runtime().handle()
}

pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    runtime().spawn(future)
}

pub fn spawn_blocking<F, R>(task: F) -> tokio::task::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    runtime().spawn_blocking(task)
}

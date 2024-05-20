use futures_util;
use logger::Logger;
use parser::*;
mod article;
mod logger;
mod parser;
use clap::Parser;
use std::sync::atomic::AtomicI32;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The number of files to use. Will count down from this to zero.
    #[arg(short, long, default_value_t = 1219)]
    filecount: usize,

    /// The number of download processes.
    #[arg(short, long, default_value_t = 10)]
    processes: usize,
}

fn main() {
    let args = Args::parse();
    let multi_threaded_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(args.processes)
        .worker_threads(args.processes)
        .build()
        .unwrap();
    let _ = multi_threaded_runtime.block_on(run(args.processes, args.filecount));
}

async fn run(n_procs: usize, n_files: usize) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(100) // Optimize the connection pool
        .build()?;
    let mut logger = Logger::new(n_procs, n_files);
    let task_counter = Arc::new(AtomicI32::new(n_files.clone() as i32));
    let logger_sender = logger.get_sender();
    let mut tasks = vec![];

    let logger_thread = std::thread::spawn(move || logger.run());
    for n in 0..n_procs {
        let client = client.clone();
        let c = logger_sender.clone();
        let mut parser =
            crate::parser::Parser::initialize(task_counter.clone(), &c.clone(), n as u32);
        let handle = tokio::spawn(async move {
            parser.try_restart(&client).await;
        });
        tasks.push(handle);
    }
    let _ = futures_util::future::join_all(tasks).await;
    let _ = logger_sender.send(ParserMessage {
        id: 0,
        new_state: ParserState::Terminate,
    });
    let _ = logger_thread.join();
    Ok(())
}

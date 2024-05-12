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

#[tokio::main]
async fn main() {
    let args = Args::parse();
    run(args.processes, args.filecount).await;
}

async fn run(n_procs: usize, n_files: usize) {
    let mut logger = Logger::new(n_procs, n_files);
    let task_counter = Arc::new(AtomicI32::new(n_files.clone() as i32));
    let logger_sender = logger.get_sender();
    let mut tasks = vec![];
    for n in 0..n_procs {
        let c = &logger.get_sender();
        let mut parser =
            crate::parser::Parser::initialize(task_counter.clone(), &c.clone(), n as u32);
        let handle = tokio::spawn(async move {
            parser.try_restart().await;
        });
        tasks.push(handle);
    }
    let logger_thread = std::thread::spawn(move || logger.run());
    let _ = futures_util::future::join_all(tasks).await;
    let _ = logger_sender.send(ParserMessage {
        id: 0,
        new_state: ParserState::Terminate,
    });
    let _ = logger_thread.join();
}

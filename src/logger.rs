use crate::parser::{ParserMessage, ParserState};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::io::{self, Write};
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct Logger {
    n_progs: usize,
    sender: Sender<ParserMessage>,
    receiver: Receiver<ParserMessage>,
    last_parser_states: Vec<ParserState>,
    bars: Vec<ProgressBar>,
    multi_progress: MultiProgress,
    progress_bar_style: ProgressStyle,
    spinner_style: ProgressStyle,
    finished_files: usize,
    found_articles: usize,
    overall_progress_bar: ProgressBar,
}

/// This class handles the log output from all the worker processes.
/// The object should be constructed using new, then the senders should be created by calling
/// get_sender as many times as required and the, once computation and reporting begins, run()
/// initializes a loop that waits for status updates and reprints the console output.
impl Logger {
    pub fn new(number_of_processes: usize, number_of_files: usize) -> Self {
        let (sender, receiver) = channel();
        let mut last_parser_states = vec![];
        for _i in 0..number_of_processes {
            last_parser_states.push(ParserState::Waiting)
        }
        let mut bars = vec![];

        let spinner_style = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
        let bar_style = ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>5}/{len:5} {msg} ({eta})",
        )
        .unwrap()
        .progress_chars("##-");
        let m = MultiProgress::new();
        let overall = m.add(ProgressBar::new(number_of_files as u64));
        overall.set_style(bar_style.clone());
        for _i in 0..number_of_processes {
            let pb = m.add(ProgressBar::new(100));
            pb.set_style(spinner_style.clone());
            bars.push(pb);
        }

        Self {
            n_progs: number_of_processes,
            sender,
            receiver,
            last_parser_states,
            multi_progress: m,
            bars,
            progress_bar_style: bar_style.clone(),
            spinner_style: spinner_style.clone(),
            finished_files: 0,
            found_articles: 0,
            overall_progress_bar: overall,
        }
    }

    pub fn get_sender(&self) -> Sender<ParserMessage> {
        self.sender.clone()
    }

    pub fn run(&mut self) {
        let mut clear_counter = 0;
        loop {
            clear_counter += 1;
            if clear_counter > 9 {
                let _ = Logger::clear_console();
                self.update_overall_progress_bar();
                for i in 0..self.n_progs {
                    self.update_view(i);
                }
                clear_counter = 0;
            }
            let msg = self.receiver.recv();
            if msg.is_ok() {
                let m = msg.unwrap();

                if matches!(m.new_state, ParserState::Terminate) {
                    println!("Shutting down.");
                    break;
                }
                let index = m.id as usize;
                if index < self.n_progs {
                    self.last_parser_states[index] = m.new_state;
                }
                self.update_view(index);
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    fn clear_console() -> io::Result<()> {
        let mut stdout = io::stdout();
        stdout.write_all(b"\x1B[2J\x1B[1;1H")?;
        stdout.flush()?;
        Ok(())
    }

    fn set_message(&self, message: &String, index: usize) {
        self.bars[index].set_message(message.clone());
        self.bars[index].set_style(self.spinner_style.clone());
    }

    fn update_view(&mut self, index: usize) {
        match self.last_parser_states[index] {
            ParserState::Restarting => self.bars[index].reset_elapsed(),
            ParserState::Waiting => self.set_message(&"Waiting".to_string(), index),
            ParserState::FinishedInputFile(n_articles) => {
                self.update_article_and_file_counts(n_articles)
            }
            ParserState::WritingFile => {
                self.set_message(&"Writing output file... ".to_string(), index)
            }
            ParserState::Done => self.finish_parser_progress(index),
            ParserState::CheckMd5 => self.set_message(&"Check Md5 Checksum".to_string(), index),
            ParserState::Downloading(progress) => {
                self.print_progress_bar("Downloading".to_string(), index, &progress)
            }
            ParserState::Processing(progress) => {
                self.print_progress_bar("Processing".to_string(), index, &progress)
            }
            ParserState::Extracting(progress) => {
                self.print_progress_bar("Extracting".to_string(), index, &progress)
            }
            ParserState::ErrorChecksumWrong => {
                Logger::print_error_message("Checksum is wrong!", index)
            }
            ParserState::ErrorWritingFailed => {
                Logger::print_error_message("Writing file failed!", index)
            }
            ParserState::ErrorDownloadFailed => {
                Logger::print_error_message("Downloading data failed!", index)
            }
            ParserState::ErrorParsingFailed => {
                Logger::print_error_message("Parsing failed!", index)
            }
            ParserState::ErrorExtractionFailed => {
                Logger::print_error_message("Extracting archive failed!", index)
            }
            ParserState::Terminate => {
                let _ = self.multi_progress.clear();
                println!("All processes have terminated.");
            }
        }
    }

    fn update_article_and_file_counts(&mut self, articles: usize) {
        self.finished_files += 1;
        self.found_articles += articles;
    }

    fn update_overall_progress_bar(&self) {
        self.overall_progress_bar
            .set_position(self.finished_files.clone() as u64);
        self.overall_progress_bar
            .set_message(format!("Found {} articles.", self.found_articles.clone()));
    }

    fn print_progress_bar(&self, message: String, index: usize, progress: &u8) {
        self.bars[index].set_message(format!("Process {}:{}", index + 1, message));
        self.bars[index].set_position(*progress as u64);
        self.bars[index].set_style(self.progress_bar_style.clone());
    }

    fn print_error_message(message: &str, index: usize) {
        println!("{}Process: {}", index, message,);
    }

    fn finish_parser_progress(&self, index: usize) {
        self.bars[index].finish_with_message("Done");
    }
}

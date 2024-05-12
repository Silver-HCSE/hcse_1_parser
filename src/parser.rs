use crate::article::*;
use async_compression::tokio::bufread::GzipDecoder;
use core::fmt;
use file_integrity::hash_file;
use reqwest::Client;
use roxmltree::{Node, ParsingOptions};
use std::sync::atomic::*;
use std::sync::Arc;
use std::{path::Path, sync::mpsc::Sender};
use tempdir::TempDir;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;

pub enum ParserState {
    Restarting,
    Waiting,
    Downloading(u8),
    CheckMd5,
    Extracting(u8),
    Processing(u8),
    WritingFile,
    FinishedInputFile(usize),
    Done,
    ErrorDownloadFailed,
    ErrorChecksumWrong,
    ErrorExtractionFailed,
    ErrorParsingFailed,
    ErrorWritingFailed,
    Terminate,
}

pub struct ParserMessage {
    pub id: u32,
    pub new_state: ParserState,
}

pub struct Parser {
    id: u32,
    download_url: String,
    local_download_filename: String,
    md5_file_name: String,
    extracted_filename: String,
    article_data: Vec<Article>,
    output_filename: String,
    sender: Sender<ParserMessage>,
    counter_arc: Arc<AtomicI32>,
    temp_dir: String,
}

impl Parser {
    pub fn initialize(
        arc: Arc<AtomicI32>,
        reporting_channel: &Sender<ParserMessage>,
        id: u32,
    ) -> Self {
        let id_string: &String = &format!("dir{}", id);
        let dir = TempDir::new(id_string).unwrap();
        let temp_dir = dir.path().to_string_lossy().replace(".", "");
        Parser {
            download_url: String::new(),
            local_download_filename: String::new(),
            md5_file_name: String::new(),
            extracted_filename: String::new(),
            article_data: vec![],
            output_filename: String::new(),
            counter_arc: arc,
            temp_dir,
            sender: reporting_channel.clone(),
            id,
        }
    }

    pub async fn try_restart(&mut self) {
        loop {
            self.counter_arc
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            let counter_value = self.counter_arc.load(std::sync::atomic::Ordering::SeqCst);
            if counter_value < 0 {
                self.report_state(ParserState::Done);
                return;
            } else {
                self.reinit_for_index(counter_value as u32).await;
            }
        }
    }

    async fn reinit_for_index(&mut self, index: u32) {
        let _ = tokio::fs::create_dir(&self.temp_dir.clone()).await;
        let fname = format!("pubmed24n{:0>4}.xml", index);
        self.report_state(ParserState::Restarting);
        self.download_url = format!("https://ftp.ncbi.nlm.nih.gov/pubmed/baseline/{}.gz", fname);
        self.local_download_filename = format!("{}/{}.gz", &self.temp_dir, fname).to_string();
        self.md5_file_name = format!("{}/{}.gz.md5", &self.temp_dir, fname).to_string();
        self.extracted_filename = format!("{}/{}", &self.temp_dir, fname).to_string();
        self.article_data = vec![];
        self.output_filename = format!("results_{}.json", fname).to_string();
        self.run().await;
    }

    pub async fn run(&mut self) {
        let is_already_parsed_locally = self.check_if_file_is_present();
        if is_already_parsed_locally {
            return;
        }
        let download_worked = self.download().await;
        if download_worked.is_err() {
            self.report_state(ParserState::ErrorDownloadFailed);
            return;
        }
        let is_checksum_correct = self.check_md5().await;
        if is_checksum_correct.is_err() {
            self.report_state(ParserState::ErrorChecksumWrong);
            return;
        }
        let extracting_status = self.extract().await;
        if extracting_status.is_err() {
            self.report_state(ParserState::ErrorExtractionFailed);
            return;
        }
        let processing_state = self.process().await;
        if processing_state.is_err() {
            self.report_state(ParserState::ErrorParsingFailed);
        }
        self.filter_articles();
        let write_putput_worked = self.write_output().await;
        if !write_putput_worked {
            self.report_state(ParserState::ErrorWritingFailed);
        }
    }

    fn check_if_file_is_present(&self) -> bool {
        Path::new(&self.output_filename).exists()
    }

    fn report_state(&self, state: ParserState) {
        let _ = self.sender.send(ParserMessage {
            id: self.id,
            new_state: state,
        });
    }

    async fn download(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::new();
        let mut response = client.get(&self.download_url).send().await?;
        let mut dest_file = File::create(&self.local_download_filename).await?;
        let total_download_size = response.content_length().unwrap_or(0);

        let mut processed_data = 0;
        let mut last_reported_percentage: u8 = 0;
        let _ = self.sender.send(ParserMessage {
            id: self.id,
            new_state: ParserState::Downloading(0),
        });
        while let Some(chunk) = response.chunk().await? {
            dest_file.write_all(&chunk).await?;
            processed_data = processed_data + chunk.len();
            let new_percentage: f32 =
                100 as f32 * processed_data as f32 / total_download_size as f32;
            if new_percentage.floor() > last_reported_percentage as f32 {
                last_reported_percentage = new_percentage.floor() as u8;
                self.report_state(ParserState::Downloading(last_reported_percentage));
            }
        }
        self.report_state(ParserState::Downloading(100));
        Ok(())
    }

    async fn check_md5(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.report_state(ParserState::CheckMd5);
        let client = Client::new();
        let mut response = client
            .get(format!("{}.md5", self.download_url))
            .send()
            .await?;
        let mut dest_file = File::create(&self.md5_file_name).await?;
        while let Some(chunk) = response.chunk().await? {
            dest_file.write_all(&chunk).await?;
        }
        let checksum_from_control = std::fs::read_to_string(&self.md5_file_name)?;
        let checksum_from_file = hash_file(self.local_download_filename.clone());
        Ok(checksum_from_control.trim() == checksum_from_file.md5_hash.trim())
    }

    async fn extract(&self) -> Result<(), std::io::Error> {
        self.report_state(ParserState::Extracting(0));
        let gz_file = tokio::fs::File::open(&self.local_download_filename).await?;
        let br = BufReader::new(gz_file);
        self.report_state(ParserState::Extracting(10));
        let mut gz = GzipDecoder::new(br);
        let mut xml_data = String::new();
        let _ = gz.read_to_string(&mut xml_data).await;
        self.report_state(ParserState::Extracting(90));
        tokio::fs::write(&self.extracted_filename, &xml_data).await?;
        self.report_state(ParserState::Extracting(100));
        Ok(())
    }

    async fn process(&mut self) -> Result<usize, fmt::Error> {
        self.report_state(ParserState::Processing(0));
        let xml_data = tokio::fs::read_to_string(&self.extracted_filename)
            .await
            .unwrap();
        let opts = ParsingOptions {
            allow_dtd: true,
            nodes_limit: u32::MAX,
        };
        let doc = roxmltree::Document::parse_with_options(&xml_data, opts).unwrap();
        let mut last_reported_percentage: u8 = 0;
        let mut processed_articles = 0;
        let itter = doc
            .root()
            .descendants()
            .filter(|n| n.tag_name().name() == "PubmedArticle");

        let total_n_articles = itter.clone().count();
        for pubmed_article in itter {
            let article = self.process_one_pubmed_article(pubmed_article);
            if article.is_valid() {
                self.article_data.push(article);
            }
            processed_articles += 1;
            let new_percentage =
                (100.0 * processed_articles as f32 / total_n_articles as f32).floor() as u8;
            if new_percentage > last_reported_percentage {
                last_reported_percentage = new_percentage;
                self.report_state(ParserState::Processing(last_reported_percentage));
            }
        }
        Ok(self.article_data.len())
    }

    pub fn process_one_pubmed_article(&self, pubmed_article: Node) -> Article {
        let mut article = Article::new();
        for child in pubmed_article.descendants() {
            match child.tag_name().name() {
                "Article" => {
                    article.set_from_article_data(child);
                }
                "Keyword" => article.tags.push(child.text().unwrap_or("").to_string()),
                "PubmedData" => article.set_from_pubmed_data(child),
                "DateCompleted" => article.set_date_from_date_node(child),
                _ => {}
            }
        }
        article
    }

    fn filter_articles(&mut self) {
        self.article_data.retain(|a| a.is_article_relevant());
    }

    async fn write_output(&self) -> bool {
        self.report_state(ParserState::WritingFile);
        let articles_json = serde_json::to_string_pretty(&self.article_data).unwrap();
        let mut file = File::create(&self.output_filename).await.unwrap();
        file.write_all(articles_json.as_bytes()).await.unwrap();
        let _ = tokio::fs::remove_dir(self.temp_dir.clone()).await;
        self.report_state(ParserState::FinishedInputFile(self.article_data.len()));
        true
    }
}

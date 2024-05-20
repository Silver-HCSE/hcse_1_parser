use roxmltree::Node;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Article {
    pub title: String,
    pub pmid: String,
    pub doi: String,
    pub pmc: String,
    pub pii: String,
    pub paper_abstract: String,
}

impl Article {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            doi: String::new(),
            pmid: String::new(),
            pii: String::new(),
            pmc: String::new(),
            paper_abstract: String::new(),
        }
    }

    pub fn set_from_article_data(&mut self, node: Node) {
        for child in node.children() {
            match child.tag_name().name() {
                "ArticleTitle" => {
                    if self.title != "".to_string() {
                        println!("multiple article titles found.");
                    }
                    self.title = child.text().unwrap_or("").to_string()
                }
                "Abstract" => {
                    for abstract_node in child.children() {
                        if abstract_node.tag_name().name() == "AbstractText" {
                            self.paper_abstract = abstract_node.text().unwrap_or("").to_string();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub fn set_from_pubmed_data(&mut self, node: Node) {
        for child in node.children() {
            match child.tag_name().name() {
                "ArticleIdList" => self.set_doi_for_id_list(child),
                _ => {}
            }
        }
    }

    pub fn set_doi_for_id_list(&mut self, article_id_list: Node) {
        for child in article_id_list.children() {
            match child.tag_name().name() {
                "ArticleId" => {
                    if child.has_attribute("IdType") {
                        if child.attribute("IdType").unwrap_or_default() == "doi".to_string() {
                            self.doi = child.text().unwrap_or("").to_string();
                        }
                        if child.attribute("IdType").unwrap_or_default() == "pubmed".to_string() {
                            self.pmid = child.text().unwrap_or("").to_string();
                        }
                        if child.attribute("IdType").unwrap_or_default() == "pmc".to_string() {
                            self.pmc = child.text().unwrap_or("").to_string();
                        }
                        if child.attribute("IdType").unwrap_or_default() == "pii".to_string() {
                            self.pii = child.text().unwrap_or("").to_string();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub fn is_valid(&self) -> bool {
        self.title != "".to_string() && self.doi != "".to_string()
    }

    /// We will suppress the dead code warning for this function because it is useful for
    /// debugging.
    #[allow(dead_code)]
    pub fn print(&self) {
        println!("{}", self.title);
        println!("{}", self.pmc);
        println!("{}", self.doi);
        println!("{}", self.paper_abstract);
        println!("----------------");
    }

    pub fn is_article_relevant(&self) -> bool {
        Article::is_string_relevant(&self.title)
            && Article::is_string_relevant(&self.paper_abstract)
    }

    fn is_string_relevant(some_text: &String) -> bool {
        if some_text.contains("cancer") {
            return true;
        }
        if some_text.contains("oncology") {
            return true;
        }
        if some_text.contains("tumor") {
            return true;
        }
        return false;
    }
}

use roxmltree::Node;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Article {
    pub title: String,
    pub id: String,
    pub paper_abstract: String,
    pub authors: Vec<Author>,
    pub tags: Vec<String>,
    pub date: String,
    pub language: String,
}

impl Article {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            id: String::new(),
            paper_abstract: String::new(),
            authors: vec![],
            tags: vec![],
            date: String::new(),
            language: String::new(),
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
                "Language" => self.language = child.text().unwrap_or("").to_string(),
                "AuthorList" => self.set_authors_from_author_list(child),
                _ => {}
            }
        }
    }

    fn set_authors_from_author_list(&mut self, node: Node) {
        for child in node.children() {
            if child.tag_name().name() == "Author" {
                let mut author = Author::new();
                for author_child in child.children() {
                    match author_child.tag_name().name() {
                        "LastName" => {
                            author.last_name = author_child.text().unwrap_or("").to_string()
                        }
                        "ForeName" => {
                            author.first_name = author_child.text().unwrap_or("").to_string()
                        }
                        _ => {}
                    }
                }
                self.authors.push(author);
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
                            self.id = child.text().unwrap_or("").to_string();
                            if self.id != "".to_string() {
                                return;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub fn set_date_from_date_node(&mut self, node: Node) {
        let mut year: u32 = 0;
        let mut month: u32 = 0;
        let mut day: u32 = 0;
        for child in node.children() {
            match child.tag_name().name() {
                "Year" => year = child.text().unwrap_or("0").parse::<u32>().unwrap_or(0),
                "Month" => month = child.text().unwrap_or("0").parse::<u32>().unwrap_or(0),
                "Day" => day = child.text().unwrap_or("0").parse::<u32>().unwrap_or(0),
                _ => {}
            }
        }
        if year > 0 && month > 0 && day > 0 {
            self.date = format!("{}-{}-{}", year, month, day);
        }
    }

    pub fn is_valid(&self) -> bool {
        self.title != "".to_string()
            && self.date != "".to_string()
            && self.authors.len() > 0
            && self.id != "".to_string()
    }

    /// We will suppress the dead code warning for this function because it is useful for
    /// debugging.
    #[allow(dead_code)]
    pub fn print(&self) {
        println!("{}", self.title);
        println!("{}", self.date);
        println!("{}", self.id);
        println!("{}", self.paper_abstract);
        let mut auth = String::new();
        for a in self.authors.iter() {
            auth.push_str(&a.last_name);
            auth.push_str(",");
        }
        println!("{}", auth);
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Author {
    pub first_name: String,
    pub last_name: String,
}

impl Author {
    pub fn new() -> Self {
        Self {
            first_name: String::new(),
            last_name: String::new(),
        }
    }
}

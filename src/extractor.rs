use super::{dom, error::Error};
use html5ever::tendril::stream::TendrilSink;
use html5ever::{parse_document, serialize};
use markup5ever_rcdom::{RcDom, SerializableHandle};

use super::scorer;
use reqwest_wasm;
use scorer::Candidate;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::default::Default;
use std::io::Read;
use std::path::Path;

use url::Url;

#[derive(Debug)]
pub struct Product {
    pub title: String,
    pub content: String,
    pub text: String,
}

pub async fn scrape(url: &str) -> Result<Product, Error> {
    let res = reqwest_wasm::get(url).await.map_err(Error::NetworkError)?;
    let success = res.status().is_success();
    let data = res.bytes().await?;
    let data = data.to_vec();

    if success {
        let url = Url::parse(url)?;
        extract(data.as_slice(), &url)
    } else {
        Err(Error::Unexpected)
    }
}

pub fn extract<R>(mut input: R, url: &Url) -> Result<Product, Error>
where
    R: Read,
{
    let mut dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut input)?;
    let mut title = String::new();
    let mut candidates = BTreeMap::new();
    let mut nodes = BTreeMap::new();
    let handle = dom.document.clone();
    scorer::preprocess(&mut dom, handle.clone(), &mut title);
    scorer::find_candidates(Path::new("/"), handle.clone(), &mut candidates, &mut nodes);
    let mut id: &str = "/";
    let mut top_candidate: &Candidate = &Candidate {
        node: handle.clone(),
        score: Cell::new(0.0),
    };
    for (i, c) in candidates.iter() {
        let score = c.score.get() * (1.0 - scorer::get_link_density(c.node.clone()));
        c.score.set(score);
        if score <= top_candidate.score.get() {
            continue;
        }
        id = i;
        top_candidate = c;
    }
    let mut bytes = vec![];

    let node = top_candidate.node.clone();
    scorer::clean(&mut dom, Path::new(id), node.clone(), url, &candidates);

    serialize(
        &mut bytes,
        &SerializableHandle::from(node.clone()),
        Default::default(),
    )
    .ok();
    let content = String::from_utf8(bytes).unwrap_or_default();

    let mut text: String = String::new();
    dom::extract_text(node.clone(), &mut text, true);
    Ok(Product {
        title,
        content,
        text,
    })
}

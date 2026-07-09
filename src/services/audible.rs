use crate::{api::api_error::ApiError, models::match_meta::MatchCandidate};
use serde::Deserialize;
use std::time::Duration;

// Free catalog API, no key required; the same source Audiobookshelf uses.
// Alternatives for later: Audnexus (api.audnex.us, ASIN enrich), Audimeta.de,
// Open Library (print-oriented, spotty series data).
const CATALOG_URL: &str = "https://api.audible.com/1.0/catalog/products";

#[derive(Deserialize)]
struct CatalogResponse {
    #[serde(default)]
    products: Vec<Product>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Product {
    title: Option<String>,
    asin: Option<String>,
    authors: Vec<Contributor>,
    narrators: Vec<Contributor>,
    series: Vec<SeriesEntry>,
    release_date: Option<String>,
    product_images: Option<serde_json::Value>,
    // product_desc response group; publisher_summary is the fuller description when
    // present, merchandising_summary is the shorter teaser Audible always returns.
    publisher_summary: Option<String>,
    merchandising_summary: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Contributor {
    name: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SeriesEntry {
    title: Option<String>,
    sequence: Option<String>,
    asin: Option<String>,
}

/// Search the Audible catalog by title (+ optional author). Empty results are Ok(vec![]).
pub async fn search_products(
    title: &str,
    author: Option<&str>,
) -> Result<Vec<MatchCandidate>, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let mut query: Vec<(&str, String)> = vec![
        ("num_results", "10".to_string()),
        ("title", title.to_string()),
        (
            "response_groups",
            "contributors,series,media,product_desc".to_string(),
        ),
    ];
    if let Some(author) = author {
        query.push(("author", author.to_string()));
    }

    let response = client.get(CATALOG_URL).query(&query).send().await?;
    let body: CatalogResponse = response.json().await?;

    Ok(body.products.into_iter().filter_map(to_candidate).collect())
}

fn to_candidate(p: Product) -> Option<MatchCandidate> {
    let title = p.title?;
    let series = p.series.into_iter().next();

    Some(MatchCandidate {
        title,
        author: p.authors.into_iter().next().and_then(|c| c.name),
        narrator: p.narrators.into_iter().next().and_then(|c| c.name),
        series_name: series.as_ref().and_then(|s| s.title.clone()),
        series_sequence: series.as_ref().and_then(|s| s.sequence.clone()),
        series_asin: series.and_then(|s| s.asin),
        year: p
            .release_date
            .as_deref()
            .and_then(|d| d.get(..4))
            .and_then(|y| y.parse().ok()),
        asin: p.asin,
        cover_url: largest_image(p.product_images),
        description: p.publisher_summary.or(p.merchandising_summary),
        confidence: 0.0,
    })
}

fn largest_image(images: Option<serde_json::Value>) -> Option<String> {
    let map = images?;
    let obj = map.as_object()?;
    obj.keys()
        .filter_map(|k| k.parse::<u32>().ok())
        .max()
        .and_then(|k| obj.get(&k.to_string()))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_catalog_response() {
        // Trimmed real-shape catalog payload.
        let json = r#"{
            "products": [{
                "asin": "B002V0QK4C",
                "title": "The Silmarillion",
                "authors": [{"name": "J. R. R. Tolkien"}],
                "narrators": [{"name": "Martin Shaw"}],
                "series": [{"title": "The Lord of the Rings", "sequence": "0", "asin": "B005NF6MIQ"}],
                "release_date": "2008-08-12",
                "product_images": {"500": "https://img/500.jpg", "1024": "https://img/1024.jpg"},
                "merchandising_summary": "The forerunner to The Lord of the Rings."
            }, {
                "asin": "B0XXNOTITLE",
                "authors": [],
                "narrators": [],
                "series": []
            }]
        }"#;

        let parsed: CatalogResponse = serde_json::from_str(json).unwrap();
        let candidates: Vec<MatchCandidate> =
            parsed.products.into_iter().filter_map(to_candidate).collect();

        // Product without a title is dropped.
        assert_eq!(candidates.len(), 1);
        let c = &candidates[0];
        assert_eq!(c.title, "The Silmarillion");
        assert_eq!(c.author.as_deref(), Some("J. R. R. Tolkien"));
        assert_eq!(c.narrator.as_deref(), Some("Martin Shaw"));
        assert_eq!(c.series_name.as_deref(), Some("The Lord of the Rings"));
        assert_eq!(c.series_sequence.as_deref(), Some("0"));
        assert_eq!(c.series_asin.as_deref(), Some("B005NF6MIQ"));
        assert_eq!(c.year, Some(2008));
        assert_eq!(c.asin.as_deref(), Some("B002V0QK4C"));
        assert_eq!(c.cover_url.as_deref(), Some("https://img/1024.jpg"));
        assert_eq!(c.description.as_deref(), Some("The forerunner to The Lord of the Rings."));
    }
}

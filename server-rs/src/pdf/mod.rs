// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Server-side PDFs (meal plan, grocery list, planner list, invoice), rendered
//! with Typst. Replaces the browser's jsPDF / html2canvas output.
//!
//! Each document is a Typst template (`templates/*.typ`, compiled into the
//! binary) that reads its data from a virtual `data.json`; the handlers build
//! that JSON with every string already formatted. Values are never spliced
//! into markup, so names and notes render as plain text. Fonts are embedded
//! (Liberation Sans, metric-compatible with the Helvetica the jsPDF output
//! used), so rendering needs no system fonts and no network.

pub mod docs;

use std::collections::HashMap;
use std::sync::LazyLock;

use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use crate::error::AppError;
use crate::rate_limit::AccountLimits;
use crate::state::AppState;

/// The product name, used when a chef hasn't set a business name yet
/// (`APP_NAME` in `client/src/constants.ts`).
pub const APP_NAME: &str = "Platebook";

const THEME: &str = include_str!("templates/theme.typ");

/// A document template.
#[derive(Debug, Clone, Copy)]
pub enum Template {
    MealPlan,
    Grocery,
    Planner,
    Invoice,
}

impl Template {
    fn source(self) -> &'static str {
        match self {
            Template::MealPlan => include_str!("templates/meal_plan.typ"),
            Template::Grocery => include_str!("templates/grocery.typ"),
            Template::Planner => include_str!("templates/planner.typ"),
            Template::Invoice => include_str!("templates/invoice.typ"),
        }
    }
}

/// Library and fonts are built once and shared by every render.
struct Shared {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

static SHARED: LazyLock<Shared> = LazyLock::new(|| {
    let fonts: Vec<Font> = [
        include_bytes!("../../assets/fonts/LiberationSans-Regular.ttf").as_slice(),
        include_bytes!("../../assets/fonts/LiberationSans-Bold.ttf").as_slice(),
        include_bytes!("../../assets/fonts/LiberationSans-Italic.ttf").as_slice(),
        include_bytes!("../../assets/fonts/LiberationSans-BoldItalic.ttf").as_slice(),
    ]
    .into_iter()
    .flat_map(|data| Font::iter(Bytes::new(data)))
    .collect();
    Shared {
        library: LazyHash::new(Library::default()),
        book: LazyHash::new(FontBook::from_fonts(&fonts)),
        fonts,
    }
});

fn file_id(path: &str) -> FileId {
    RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new(path).expect("valid virtual path"),
    )
    .intern()
}

/// One compilation: the template plus its in-memory files.
struct DocWorld {
    main: Source,
    files: HashMap<FileId, Bytes>,
}

impl World for DocWorld {
    fn library(&self) -> &LazyHash<Library> {
        &SHARED.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &SHARED.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            return Ok(self.main.clone());
        }
        let bytes = self.file(id)?;
        let text = std::str::from_utf8(bytes.as_slice()).map_err(|_| FileError::InvalidUtf8)?;
        Ok(Source::new(id, text.to_string()))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main.id() {
            return Ok(Bytes::from_string(self.main.clone()));
        }
        self.files
            .get(&id)
            .cloned()
            .ok_or_else(|| FileError::NotFound(id.vpath().get_without_slash().into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        SHARED.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        // Templates get every date pre-formatted in data.json.
        None
    }
}

/// An image file handed to a template (e.g. the chef's logo).
pub struct Asset {
    /// Virtual path, with an extension Typst recognizes ("logo.png").
    pub path: &'static str,
    pub bytes: Vec<u8>,
}

/// Renders `template` with `data` (read in the template as
/// `json("data.json")`) to PDF bytes. CPU-bound: call it off the async
/// runtime ([`render_blocking`]).
pub fn render(
    template: Template,
    data: &serde_json::Value,
    assets: Vec<Asset>,
) -> Result<Vec<u8>, String> {
    let mut files = HashMap::new();
    files.insert(file_id("/theme.typ"), Bytes::from_string(THEME.to_string()));
    files.insert(
        file_id("/data.json"),
        Bytes::new(serde_json::to_vec(data).map_err(|e| e.to_string())?),
    );
    for a in assets {
        files.insert(file_id(&format!("/{}", a.path)), Bytes::new(a.bytes));
    }
    let world = DocWorld {
        main: Source::new(file_id("/main.typ"), template.source().to_string()),
        files,
    };
    let doc = typst::compile::<typst_layout::PagedDocument>(&world)
        .output
        .map_err(|errs| describe(&errs))?;
    typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default()).map_err(|errs| describe(&errs))
}

fn describe(errs: &[typst::diag::SourceDiagnostic]) -> String {
    errs.iter()
        .map(|e| e.message.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

/// [`render`] on the blocking pool, as a 500 on failure.
pub async fn render_blocking(
    template: Template,
    data: serde_json::Value,
    assets: Vec<Asset>,
) -> Result<Vec<u8>, AppError> {
    tokio::task::spawn_blocking(move || {
        if assets.is_empty() {
            return render(template, &data, assets);
        }
        render(template, &data, assets).or_else(|e| {
            // An image Typst can't decode (a corrupt or odd logo upload)
            // shouldn't block the download: render again without it.
            tracing::warn!(error = %e, ?template, "[pdf] render failed with assets, retrying without");
            let mut data = data.clone();
            if let Some(obj) = data.as_object_mut() {
                obj.insert("logo".into(), serde_json::Value::Null);
            }
            render(template, &data, vec![])
        })
    })
    .await
        .map_err(AppError::internal)?
        .map_err(|e| {
            tracing::error!(error = %e, ?template, "[pdf] render");
            AppError::internal(e)
        })
}

/// Per-account PDF limit (`acct:{id}`): rendering is the most CPU-heavy
/// thing the API does.
pub async fn check_limit(state: &AppState, account_id: i64) -> Result<(), AppError> {
    state
        .account_limits
        .pdf
        .check(&state.limits_pool, &AccountLimits::key(account_id))
        .await
        .map_err(|e| e.with_code())
}

/// `application/pdf` download response.
pub fn response(bytes: Vec<u8>, filename: &str) -> Response {
    let mut res = bytes.into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    if let Ok(v) = HeaderValue::from_str(&format!(
        "attachment; filename=\"{}\"",
        safe_filename(filename)
    )) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

/// ASCII-only, quote-free filename for Content-Disposition.
fn safe_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Decodes a `data:image/...;base64,` logo into a Typst image file.
pub fn logo_asset(data_uri: Option<&str>) -> Option<Asset> {
    use base64::Engine;
    let uri = data_uri?.trim();
    let (meta, b64) = uri.strip_prefix("data:")?.split_once(',')?;
    let mime = meta.strip_suffix(";base64")?;
    let path = match mime.to_ascii_lowercase().as_str() {
        "image/png" => "logo.png",
        "image/jpeg" | "image/jpg" => "logo.jpg",
        "image/gif" => "logo.gif",
        "image/webp" => "logo.webp",
        "image/svg+xml" => "logo.svg",
        _ => return None,
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()?;
    Some(Asset { path, bytes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A valid 1×1 PNG.
    const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    fn text(pdf: &[u8]) -> String {
        assert!(pdf.starts_with(b"%PDF-"), "not a PDF");
        pdf_extract::extract_text_from_mem(pdf).expect("extract text")
    }

    #[test]
    fn decodes_logo_data_uris() {
        let a = logo_asset(Some(PNG)).expect("png");
        assert_eq!(a.path, "logo.png");
        assert!(a.bytes.starts_with(b"\x89PNG"));
        assert!(logo_asset(Some("data:text/html;base64,PGI+")).is_none());
        assert!(logo_asset(Some("not a data uri")).is_none());
        assert!(logo_asset(None).is_none());
    }

    #[test]
    fn filenames_are_header_safe() {
        assert_eq!(
            safe_filename("invoice-BK \"0001\".pdf"),
            "invoice-BK--0001-.pdf"
        );
    }

    #[test]
    fn markup_in_data_renders_as_plain_text() {
        let data = json!({
            "business": "#strong[X] = *Y*", "client": "$x$ <b>", "date": "", "serves": 4,
            "use_by": "", "dishes": [], "allergens": [], "website": "",
        });
        let t = text(&render(Template::MealPlan, &data, vec![]).unwrap());
        assert!(t.contains("#strong[X] = *Y*"), "{t}");
        assert!(t.contains("$x$ <b>"), "{t}");
    }

    #[test]
    fn long_documents_flow_onto_more_pages() {
        const CATS: [&str; 3] = ["stovetop", "oven", "cold"];
        let dishes: Vec<_> = (1..=14)
            .map(|i| {
                json!({ "name": format!("Dish number {i} with a fairly long descriptive name"),
                             "reheat": "Stovetop, medium-low heat, stir occasionally, 5–8 min",
                             "category": CATS[i % 3], "freeze": i % 2 == 0 })
            })
            .collect();
        let meal = json!({
            "business": "LONG KITCHEN", "client": "Pat", "date": "Monday, May 4, 2026", "serves": 4,
            "use_by": "May 8", "dishes": dishes, "allergens": ["Sesame"], "website": "long.example",
        });
        let items: Vec<_> = (1..=30)
            .map(|i| json!({ "name": format!("Item {i}"), "quantity": "2 cups", "sources": "Soup · Stew" }))
            .collect();
        let grocery = json!({
            "business": "LONG KITCHEN", "client": "Pat", "date": "Monday, May 4, 2026", "serves": 4,
            "notes": "", "website": "",
            "sections": [{ "name": "Produce", "count": "30 items", "items": items.clone() },
                         { "name": "Other", "count": "30 items", "items": items }],
        });
        for (name, template, data) in [
            ("long-meal-plan.pdf", Template::MealPlan, meal),
            ("long-grocery.pdf", Template::Grocery, grocery),
        ] {
            let pdf = render(template, &data, vec![]).unwrap();
            let t = text(&pdf);
            assert!(t.contains("Dish number 14") || t.contains("Item 30"), "{t}");
            if let Ok(dir) = std::env::var("PDF_OUT_DIR") {
                std::fs::write(std::path::Path::new(&dir).join(name), pdf).unwrap();
            }
        }
    }

    #[tokio::test]
    async fn a_logo_typst_cannot_decode_falls_back_to_no_logo() {
        let data = json!({
            "logo": "logo.png", "business": "Bad Logo Kitchen", "chef_name": "", "chef_email": "",
            "chef_phone": "", "number": "INV-1", "issued": "", "due": "", "client_name": "",
            "client_email": "", "client_phone": "", "client_address": "", "items": [],
            "subtotal": "$0.00", "tax": null, "total": "$0.00", "message": "",
        });
        let junk = Asset {
            path: "logo.png",
            bytes: b"not really a png".to_vec(),
        };
        let pdf = render_blocking(Template::Invoice, data, vec![junk])
            .await
            .unwrap();
        assert!(text(&pdf).contains("Bad Logo Kitchen"));
    }
}

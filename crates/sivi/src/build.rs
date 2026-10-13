//! The build of one deck: parse, bind, emit, compile, measure, report, export, publish; each stage stops the build with its diagnostics.

use crate::assets::{self, Dirs};
use crate::digest::sha256_hex;
use crate::fonts;
use crate::io;
use crate::publish::{self, Disk};
use crate::themes::{self, LoadedTheme};
use sivi_core::bind::{self, BoundDeck};
use sivi_core::diagnostic::{Code, Diagnostic, SlideRef, has_error};
use sivi_core::emit::{self, Emitted};
use sivi_core::ir::{BlockBody, Deck};
use sivi_core::lints;
use sivi_core::measure::Measurement;
use sivi_core::report::{self, Measured};
use sivi_typst::raw;
use sivi_typst::{
    CompileError, Compiled, PdfSettings, SiviWorld, UtcTimestamp, compile, export, measure,
};
use std::path::{Path, PathBuf};

/// The most characters of generated source a compile error quotes.
const EXCERPT_CHARS: usize = 80;

/// Where the PDF goes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Sink {
    File(PathBuf),
    Stdout,
    /// Nothing is written, as `check` asks.
    Nothing,
}

/// One build to run.
#[derive(Clone, Debug)]
pub struct Request<'a> {
    pub deck: &'a Path,
    /// The theme named on the command line, which overrides the deck's.
    pub theme: Option<&'a str>,
    pub sink: Sink,
    /// The creation date of the PDF, from `SOURCE_DATE_EPOCH`, or none.
    pub timestamp: Option<UtcTimestamp>,
}

/// What a successful build produced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Product {
    pub pages: u32,
    /// The digest of the PDF bytes, when a PDF was exported.
    pub sha256: Option<String>,
    /// The PDF bytes, when they go to standard output.
    pub bytes: Option<Vec<u8>>,
    pub path: Option<PathBuf>,
}

/// Everything a build leaves: the texts diagnostics quote, the diagnostics, the product, or why nothing could start.
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub deck_text: String,
    pub theme_text: String,
    pub theme_label: String,
    pub diagnostics: Vec<Diagnostic>,
    pub product: Option<Product>,
    /// Why the deck could not even be read, which is a usage error and never a diagnostic.
    pub failure: Option<String>,
}

/// The deck and theme, bound and emitted.
struct Composed {
    deck: Deck,
    theme: LoadedTheme,
    bound: BoundDeck,
    emitted: Emitted,
}

fn typst_diagnostic(found: &sivi_typst::Diagnostic, source: &str) -> Diagnostic {
    let mut diagnostic = Diagnostic::new(
        Code::TypstCompile,
        format!(
            "the generated Typst source failed to compile: {}",
            found.message
        ),
    );
    if let Some(range) = &found.range {
        let excerpt: String = source
            .get(range.start..)
            .unwrap_or("")
            .chars()
            .take(EXCERPT_CHARS)
            .collect();
        diagnostic = diagnostic.note(format!(
            "at byte {} of the generated source: `{}`",
            range.start,
            excerpt.replace('\n', " ")
        ));
    }
    for hint in &found.hints {
        diagnostic = diagnostic.note(format!("Typst hints: {hint}"));
    }
    diagnostic.help(String::from(
        "this is a defect of sivi or of the theme; report it with the deck and the theme",
    ))
}

/// The warning each code block whose fence names a language the highlighter does not know earns.
fn language_warnings(deck: &Deck) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    for slide in &deck.slides {
        for block in &slide.blocks {
            let BlockBody::Code(code) = &block.body else {
                continue;
            };
            let Some(language) = &code.language else {
                continue;
            };
            if raw::knows_language(language) {
                continue;
            }
            found.push(
                Diagnostic::new(
                    Code::UnknownLanguage,
                    format!(
                        "the language `{language}` is not one the highlighter knows, so the code shows without highlighting"
                    ),
                )
                .at(block.span, String::from("this fence names the language"))
                .help(String::from(
                    "name a language Typst highlights, or leave the fence bare",
                ))
                .on_slide(SlideRef {
                    number: slide.number,
                    id: slide.id.clone(),
                    master: slide.master.clone(),
                }),
            );
        }
    }
    found
}

struct Build<'a> {
    request: &'a Request<'a>,
    outcome: Outcome,
}

impl Build<'_> {
    fn take<T>(&mut self, result: Result<T, Vec<Diagnostic>>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(diagnostics) => {
                self.outcome.diagnostics.extend(diagnostics);
                None
            }
        }
    }

    fn deck_dir(&self) -> &Path {
        self.request.deck.parent().unwrap_or_else(|| Path::new("."))
    }

    fn read(&mut self) -> Option<Vec<u8>> {
        match io::read(self.request.deck) {
            Ok(bytes) => Some(bytes),
            Err(error) => {
                self.outcome.failure = Some(error.to_string());
                None
            }
        }
    }

    fn parse(&mut self, bytes: &[u8]) -> Option<Deck> {
        self.outcome.deck_text = String::from_utf8_lossy(bytes).into_owned();
        let parsed = sivi_markdown::parse(bytes);
        self.outcome.diagnostics.extend(parsed.diagnostics);
        if has_error(&self.outcome.diagnostics) {
            return None;
        }
        let deck = parsed.deck?;
        self.outcome.diagnostics.extend(lints::deck_warnings(&deck));
        self.outcome.diagnostics.extend(language_warnings(&deck));
        Some(deck)
    }

    fn theme(&mut self, deck: &Deck) -> Option<LoadedTheme> {
        let choice = themes::choose(
            self.request.theme,
            deck.head.theme.as_ref(),
            self.deck_dir(),
        );
        let (loaded, warnings) = self.take(themes::load(&choice))?;
        self.outcome.diagnostics.extend(warnings);
        self.outcome.theme_text.clone_from(&loaded.text);
        self.outcome.theme_label.clone_from(&loaded.label);
        Some(loaded)
    }

    fn compose(&mut self, deck: &Deck, theme: &LoadedTheme) -> Option<(BoundDeck, Emitted)> {
        let bound = self.take(bind::bind(deck, &theme.theme))?;
        let boxes = fonts::line_boxes(&theme.faces);
        let emitted = self.take(emit::typst_source(&theme.theme, deck, &bound, &boxes))?;
        Some((bound, emitted))
    }

    /// Compiles the source with its assets, returning the pages and the identity of every input.
    fn compile(&mut self, composed: &Composed) -> Option<(Compiled, String)> {
        let dirs = Dirs {
            deck: self.deck_dir(),
            theme: composed.theme.dir.as_deref(),
        };
        let loaded = assets::load(&composed.emitted.assets, &composed.deck, dirs);
        self.outcome.diagnostics.extend(loaded.diagnostics);
        if has_error(&self.outcome.diagnostics) {
            return None;
        }
        let assets = loaded.assets;
        let mut parts: Vec<&[u8]> = vec![
            self.outcome.deck_text.as_bytes(),
            composed.theme.text.as_bytes(),
        ];
        parts.extend(assets.iter().map(|asset| asset.bytes.as_slice()));
        parts.extend(composed.theme.fonts.iter().map(Vec::as_slice));
        let ident = sha256_hex(&parts);
        let source = composed.emitted.source.as_str();
        let world = match SiviWorld::new(source, assets, composed.theme.fonts.clone()) {
            Ok(world) => world,
            Err(error) => {
                self.outcome.diagnostics.push(Diagnostic::new(
                    Code::Internal,
                    format!("the compiler's world cannot be built: {error}"),
                ));
                return None;
            }
        };
        match compile(&world) {
            Ok(compiled) => {
                for warning in &compiled.warnings {
                    self.outcome
                        .diagnostics
                        .push(typst_diagnostic(warning, source));
                }
                if has_error(&self.outcome.diagnostics) {
                    return None;
                }
                Some((compiled, ident))
            }
            Err(CompileError::Typst(errors)) => {
                for error in &errors {
                    self.outcome
                        .diagnostics
                        .push(typst_diagnostic(error, source));
                }
                None
            }
        }
    }

    /// Reads the layout back and judges it.
    fn verdict(&mut self, composed: &Composed, compiled: &Compiled) -> Option<Measurement> {
        let measurement = match measure(&compiled.document, &composed.emitted.expected) {
            Ok(measurement) => measurement,
            Err(error) => {
                self.outcome.diagnostics.push(
                    Diagnostic::new(
                        Code::MarkerMissing,
                        format!("the layout could not be read back: {error}"),
                    )
                    .help(String::from(
                        "this is a defect of sivi; report it with the deck",
                    )),
                );
                return None;
            }
        };
        self.outcome.diagnostics.extend(report::diagnose(
            &composed.theme.theme,
            &composed.deck,
            &composed.bound,
            Measured {
                expected: &composed.emitted.expected,
                measurement: &measurement,
            },
        ));
        if has_error(&self.outcome.diagnostics) {
            None
        } else {
            Some(measurement)
        }
    }

    fn produce(&mut self, compiled: &Compiled, ident: String, pages: u32) {
        if self.request.sink == Sink::Nothing {
            self.outcome.product = Some(Product {
                pages,
                sha256: None,
                bytes: None,
                path: None,
            });
            return;
        }
        let settings = PdfSettings {
            ident,
            timestamp: self.request.timestamp,
        };
        let pdf = match export(&compiled.document, &settings) {
            Ok(pdf) => pdf,
            Err(error) => {
                self.outcome.diagnostics.push(Diagnostic::new(
                    Code::PdfExport,
                    format!("the PDF could not be written: {error}"),
                ));
                return;
            }
        };
        let sha256 = Some(sha256_hex(&[&pdf]));
        match &self.request.sink {
            Sink::File(path) => {
                if let Err(error) = publish::publish(&mut Disk::default(), path, &pdf) {
                    self.outcome.diagnostics.push(
                        Diagnostic::new(
                            Code::Publish,
                            format!("the output could not be published: {error}"),
                        )
                        .help(String::from(
                            "check that the directory of the output exists and is writable",
                        )),
                    );
                    return;
                }
                self.outcome.product = Some(Product {
                    pages,
                    sha256,
                    bytes: None,
                    path: Some(path.clone()),
                });
            }
            Sink::Stdout => {
                self.outcome.product = Some(Product {
                    pages,
                    sha256,
                    bytes: Some(pdf),
                    path: None,
                });
            }
            Sink::Nothing => {}
        }
    }
}

/// Runs one build to its end or to its first failing stage.
#[must_use]
pub fn run(request: &Request<'_>) -> Outcome {
    let mut build = Build {
        request,
        outcome: Outcome::default(),
    };
    let Some(bytes) = build.read() else {
        return build.outcome;
    };
    let Some(deck) = build.parse(&bytes) else {
        return build.outcome;
    };
    let Some(theme) = build.theme(&deck) else {
        return build.outcome;
    };
    let Some((bound, emitted)) = build.compose(&deck, &theme) else {
        return build.outcome;
    };
    let composed = Composed {
        deck,
        theme,
        bound,
        emitted,
    };
    let Some((compiled, ident)) = build.compile(&composed) else {
        return build.outcome;
    };
    let Some(measurement) = build.verdict(&composed, &compiled) else {
        return build.outcome;
    };
    build.produce(&compiled, ident, measurement.pages.0);
    build.outcome
}

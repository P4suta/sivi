# Diagnostics

Every message of sivi carries a code from this catalog, which `cargo xtask gen` writes from the enum in `crates/sivi-core/src/diagnostic.rs`.
A code never changes meaning and is never reused.
An `E` code is an error, which stops the build; a `W` code is a warning, which `--deny-warnings` turns into a failure of the exit status.
The [command line](spec/cli.md) explains the exit statuses and the JSON document that carries these codes.

## Codes

| Code | Severity | Name | Meaning |
| --- | --- | --- | --- |
| [E0001](#e0001) | error | `Internal` | an internal failure |
| [E0101](#e0101) | error | `NotUtf8` | the deck is not UTF-8 |
| [E0102](#e0102) | error | `UnclosedFrontMatter` | a front matter block has no closing fence |
| [E0103](#e0103) | error | `MalformedKeyValue` | a front matter line is not `key: value` |
| [E0104](#e0104) | error | `UnknownKey` | a front matter key is unknown or belongs elsewhere |
| [E0105](#e0105) | error | `InvalidValue` | a front matter value leaves its grammar |
| [E0106](#e0106) | error | `DuplicateKey` | a front matter key repeats |
| [E0107](#e0107) | error | `DuplicateSlideId` | two slides share an id |
| [E0108](#e0108) | error | `NoSlides` | the deck holds no slide |
| [E0110](#e0110) | error | `LimitExceeded` | an input exceeds its size limit |
| [E0201](#e0201) | error | `HtmlBlock` | a raw HTML block |
| [E0202](#e0202) | error | `HtmlInline` | raw inline HTML |
| [E0203](#e0203) | error | `HtmlComment` | an HTML comment, which carries no directive |
| [E0204](#e0204) | error | `SetextHeading` | a setext heading, whose underline reads as a separator |
| [E0205](#e0205) | error | `HeadingTooDeep` | a heading below level two |
| [E0206](#e0206) | error | `Footnote` | a footnote |
| [E0207](#e0207) | error | `TaskList` | a task list item |
| [E0208](#e0208) | error | `Math` | a math span |
| [E0209](#e0209) | error | `IndentedCode` | an indented code block |
| [E0210](#e0210) | error | `LinkScheme` | a link whose destination is not a web, mail, or slide address |
| [E0212](#e0212) | error | `ImagePlacement` | an image that is not alone in its paragraph or whose path leaves the deck |
| [E0213](#e0213) | error | `QuoteContent` | a block quote holding more than one paragraph |
| [E0214](#e0214) | error | `HardBreak` | a hard line break |
| [E0215](#e0215) | error | `NotesCount` | a second notes block on one slide |
| [E0216](#e0216) | error | `ThematicBreak` | a thematic break that is not a separator |
| [E0217](#e0217) | error | `OutsideSubset` | a construct outside the Markdown subset |
| [E0218](#e0218) | error | `ListItemContent` | a list item holding more than a paragraph and a nested list |
| [E0219](#e0219) | error | `TableCells` | a table row whose cell count differs from the header's |
| [E0220](#e0220) | error | `EmptyBlock` | an empty heading or list item |
| [E0221](#e0221) | error | `UnresolvedReference` | a reference link without a definition |
| [E0301](#e0301) | error | `MasterMissing` | a slide without a master and a deck without a default |
| [E0302](#e0302) | error | `UnknownMaster` | a master the theme does not define |
| [E0303](#e0303) | error | `NoSlotForBlock` | a block no slot of the master accepts |
| [E0304](#e0304) | error | `RequiredSlotEmpty` | a required slot left empty |
| [E0305](#e0305) | error | `SectionCount` | more sections than the master holds |
| [E0306](#e0306) | error | `CapacityExceeded` | content beyond a slot's capacity |
| [E0307](#e0307) | error | `TitlePosition` | a title heading away from the start of its slide |
| [E0308](#e0308) | error | `UnknownSlideId` | a link to a slide id no slide carries |
| [E0401](#e0401) | error | `VerticalOverflow` | content taller than its region |
| [E0402](#e0402) | error | `HorizontalOverflow` | a line wider than its region |
| [E0403](#e0403) | error | `MultiplePages` | a slide that spilled onto a second page |
| [E0404](#e0404) | error | `OutsidePage` | an item laid out beyond the page |
| [E0405](#e0405) | error | `TooManyLines` | more lines than the slot allows |
| [E0501](#e0501) | error | `ThemeToml` | a theme file that is not TOML |
| [E0502](#e0502) | error | `ThemeSchema` | a theme with an unknown schema or key |
| [E0503](#e0503) | error | `ThemeValue` | a theme value outside its grammar |
| [E0504](#e0504) | error | `RegionOutsidePage` | a region outside the page |
| [E0505](#e0505) | error | `RegionOverlap` | two slot regions that overlap |
| [E0506](#e0506) | error | `BelowMinimumSize` | a text size below the theme's minimum |
| [E0507](#e0507) | error | `UndefinedReference` | a reference to a color, text style, or font the theme does not define |
| [E0508](#e0508) | error | `BindingConflict` | slot bindings that contradict each other |
| [E0509](#e0509) | error | `NoMasterOrSlot` | a theme without a master or a master without a slot |
| [E0510](#e0510) | error | `MissingFace` | a font face the theme needs but no font provides |
| [E0511](#e0511) | error | `LineHeightBelowFace` | a line height below the face's natural line box |
| [E0512](#e0512) | error | `CodeThemeUnknown` | a code theme the carried schemes lack |
| [E0601](#e0601) | error | `FontMissing` | a font file the theme names is missing |
| [E0602](#e0602) | error | `FontDigest` | a font file whose digest differs from the theme's |
| [E0603](#e0603) | error | `FontUnreadable` | a font file that cannot be read as a font |
| [E0604](#e0604) | error | `MissingGlyph` | a character no font of the theme provides |
| [E0605](#e0605) | error | `ImageMissing` | an image file is missing |
| [E0606](#e0606) | error | `ImageUnreadable` | an image file that cannot be decoded |
| [E0607](#e0607) | error | `ImageKind` | an image whose header does not match its extension |
| [E0608](#e0608) | error | `FamilyMismatch` | a font whose family name differs from the theme's |
| [E0609](#e0609) | error | `FamilyUnknown` | a font family that is neither bundled, declared by the theme, nor in the catalog |
| [E0610](#e0610) | error | `FontNotInstalled` | a catalog family the theme sets text in that is not installed |
| [E0901](#e0901) | error | `TypstCompile` | the generated Typst source failed to compile |
| [E0902](#e0902) | error | `MarkerMissing` | a measurement marker missing or malformed |
| [E0903](#e0903) | error | `OracleDisagreement` | the measure and the frame walk disagree |
| [E0904](#e0904) | error | `PdfExport` | the PDF export failed |
| [E0905](#e0905) | error | `Publish` | the output could not be published |
| [W0101](#w0101) | warning | `EmptySlide` | a slide with no content |
| [W0102](#w0102) | warning | `NearlyFull` | a region filled beyond ninety-five percent |
| [W0103](#w0103) | warning | `LowResolution` | an image below one hundred forty-four pixels per inch |
| [W0104](#w0104) | warning | `UnknownLanguage` | a code language the highlighter does not know |
| [W0105](#w0105) | warning | `MissingAlt` | an image without alternative text |
| [W0106](#w0106) | warning | `NoTitle` | a deck without a title |
| [W0107](#w0107) | warning | `CjkWithoutLang` | East Asian text in a deck whose language is not East Asian |
| [W0108](#w0108) | warning | `NoLang` | a deck without a language |
| [W0501](#w0501) | warning | `UnusedFont` | a font the theme loads and never uses |
| [W0502](#w0502) | warning | `FallbackWeight` | a fallback family without the exact weight a style sets |

## E0001

`Internal`, an error.
An internal failure.

## E0101

`NotUtf8`, an error.
The deck is not UTF-8.

## E0102

`UnclosedFrontMatter`, an error.
A front matter block has no closing fence.

## E0103

`MalformedKeyValue`, an error.
A front matter line is not `key: value`.

## E0104

`UnknownKey`, an error.
A front matter key is unknown or belongs elsewhere.

## E0105

`InvalidValue`, an error.
A front matter value leaves its grammar.

## E0106

`DuplicateKey`, an error.
A front matter key repeats.

## E0107

`DuplicateSlideId`, an error.
Two slides share an id.

## E0108

`NoSlides`, an error.
The deck holds no slide.

## E0110

`LimitExceeded`, an error.
An input exceeds its size limit.

## E0201

`HtmlBlock`, an error.
A raw HTML block.

## E0202

`HtmlInline`, an error.
Raw inline HTML.

## E0203

`HtmlComment`, an error.
An HTML comment, which carries no directive.

## E0204

`SetextHeading`, an error.
A setext heading, whose underline reads as a separator.

## E0205

`HeadingTooDeep`, an error.
A heading below level two.

## E0206

`Footnote`, an error.
A footnote.

## E0207

`TaskList`, an error.
A task list item.

## E0208

`Math`, an error.
A math span.

## E0209

`IndentedCode`, an error.
An indented code block.

## E0210

`LinkScheme`, an error.
A link whose destination is not a web, mail, or slide address.

## E0212

`ImagePlacement`, an error.
An image that is not alone in its paragraph or whose path leaves the deck.

## E0213

`QuoteContent`, an error.
A block quote holding more than one paragraph.

## E0214

`HardBreak`, an error.
A hard line break.

## E0215

`NotesCount`, an error.
A second notes block on one slide.

## E0216

`ThematicBreak`, an error.
A thematic break that is not a separator.

## E0217

`OutsideSubset`, an error.
A construct outside the Markdown subset.

## E0218

`ListItemContent`, an error.
A list item holding more than a paragraph and a nested list.

## E0219

`TableCells`, an error.
A table row whose cell count differs from the header's.

## E0220

`EmptyBlock`, an error.
An empty heading or list item.

## E0221

`UnresolvedReference`, an error.
A reference link without a definition.

## E0301

`MasterMissing`, an error.
A slide without a master and a deck without a default.

## E0302

`UnknownMaster`, an error.
A master the theme does not define.

## E0303

`NoSlotForBlock`, an error.
A block no slot of the master accepts.

## E0304

`RequiredSlotEmpty`, an error.
A required slot left empty.

## E0305

`SectionCount`, an error.
More sections than the master holds.

## E0306

`CapacityExceeded`, an error.
Content beyond a slot's capacity.

## E0307

`TitlePosition`, an error.
A title heading away from the start of its slide.

## E0308

`UnknownSlideId`, an error.
A link to a slide id no slide carries.

## E0401

`VerticalOverflow`, an error.
Content taller than its region.

## E0402

`HorizontalOverflow`, an error.
A line wider than its region.

## E0403

`MultiplePages`, an error.
A slide that spilled onto a second page.

## E0404

`OutsidePage`, an error.
An item laid out beyond the page.

## E0405

`TooManyLines`, an error.
More lines than the slot allows.

## E0501

`ThemeToml`, an error.
A theme file that is not TOML.

## E0502

`ThemeSchema`, an error.
A theme with an unknown schema or key.

## E0503

`ThemeValue`, an error.
A theme value outside its grammar.

## E0504

`RegionOutsidePage`, an error.
A region outside the page.

## E0505

`RegionOverlap`, an error.
Two slot regions that overlap.

## E0506

`BelowMinimumSize`, an error.
A text size below the theme's minimum.

## E0507

`UndefinedReference`, an error.
A reference to a color, text style, or font the theme does not define.

## E0508

`BindingConflict`, an error.
Slot bindings that contradict each other.

## E0509

`NoMasterOrSlot`, an error.
A theme without a master or a master without a slot.

## E0510

`MissingFace`, an error.
A font face the theme needs but no font provides.

## E0511

`LineHeightBelowFace`, an error.
A line height below the face's natural line box.

## E0512

`CodeThemeUnknown`, an error.
A code theme the carried schemes lack.

## E0601

`FontMissing`, an error.
A font file the theme names is missing.

## E0602

`FontDigest`, an error.
A font file whose digest differs from the theme's.

## E0603

`FontUnreadable`, an error.
A font file that cannot be read as a font.

## E0604

`MissingGlyph`, an error.
A character no font of the theme provides.

## E0605

`ImageMissing`, an error.
An image file is missing.

## E0606

`ImageUnreadable`, an error.
An image file that cannot be decoded.

## E0607

`ImageKind`, an error.
An image whose header does not match its extension.

## E0608

`FamilyMismatch`, an error.
A font whose family name differs from the theme's.

## E0609

`FamilyUnknown`, an error.
A font family that is neither bundled, declared by the theme, nor in the catalog.

## E0610

`FontNotInstalled`, an error.
A catalog family the theme sets text in that is not installed.

## E0901

`TypstCompile`, an error.
The generated Typst source failed to compile.

## E0902

`MarkerMissing`, an error.
A measurement marker missing or malformed.

## E0903

`OracleDisagreement`, an error.
The measure and the frame walk disagree.

## E0904

`PdfExport`, an error.
The PDF export failed.

## E0905

`Publish`, an error.
The output could not be published.

## W0101

`EmptySlide`, a warning.
A slide with no content.

## W0102

`NearlyFull`, a warning.
A region filled beyond ninety-five percent.

## W0103

`LowResolution`, a warning.
An image below one hundred forty-four pixels per inch.

## W0104

`UnknownLanguage`, a warning.
A code language the highlighter does not know.

## W0105

`MissingAlt`, a warning.
An image without alternative text.

## W0106

`NoTitle`, a warning.
A deck without a title.

## W0107

`CjkWithoutLang`, a warning.
East Asian text in a deck whose language is not East Asian.

## W0108

`NoLang`, a warning.
A deck without a language.

## W0501

`UnusedFont`, a warning.
A font the theme loads and never uses.

## W0502

`FallbackWeight`, a warning.
A fallback family without the exact weight a style sets.

# The command line

`sivi` compiles one deck to one PDF, or says why it cannot.
Every decision it reports is a diagnostic of the [catalog](../diagnostics.md).

## Commands

```text
sivi build <DECK> [-o <OUT.pdf> | -o -] [--theme <NAME|DIR>] [--format human|json] [--deny-warnings] [--color auto|always|never]
sivi check <DECK> [--theme <NAME|DIR>] [--format human|json] [--deny-warnings] [--color auto|always|never]
sivi theme check <NAME|DIR> [--format human|json] [--deny-warnings] [--color auto|always|never]
sivi fonts list [--script <SCRIPT>] [--installed] [--format human|json]
sivi fonts install <FAMILY>... [--format human|json] [--color auto|always|never]
sivi fonts dir
```

`build` reads the deck, binds every slide to a master of its theme, lays the deck out through Typst, reads the layout back, and writes the PDF when no error was reported.
Without `-o` the PDF goes beside the deck as `<DECK>.pdf` with the `.md` replaced; `-o -` writes it to standard output.
`check` runs the same pipeline through the layout verdicts and writes nothing.
`theme check` validates a theme: the built-in `plain` by name, or a directory holding `theme.toml` and the files it names.
`--theme` replaces the deck's `theme:`; a built-in name or a directory.
`fonts list` shows every family of the font catalog with its state on this host: bundled in the binary, installed in the library, or available.
`fonts install` fetches the files of each named family from google/fonts at the commit the catalog pins, refuses any file whose digest differs, and places them in the library.
`fonts dir` prints the library, the directory installed fonts live in.

## Streams

Standard output carries only the PDF bytes under `-o -` or the JSON document under `--format json`; the two cannot be combined.
Diagnostics for a person go to standard error, in the style of rustc, with the deck's lines under each span.
`--color` decides styling: `auto` styles when standard error is a terminal and `NO_COLOR` is unset.

## Exit status

| Status | Meaning |
| --- | --- |
| 0 | No error was reported; warnings may have been |
| 1 | An error was reported, or a warning under `--deny-warnings` |
| 2 | A usage error, or the deck could not be read |
| 3 | A diagnostic names a defect of sivi or of its adapters: `E0001` or any `E09xx` |

## Environment

| Variable | Effect |
| --- | --- |
| `SOURCE_DATE_EPOCH` | Seconds since the Unix epoch that become the PDF creation date; without it the PDF carries no date |
| `NO_COLOR` | Disables styling when set to anything but the empty string |
| `SIVI_HOME` | The data directory, under which `fonts/` is the library of installed fonts |
| `XDG_DATA_HOME`, `HOME` | Without `SIVI_HOME`, the library lives under `/sivi` or `/.local/share/sivi` |
| `LOCALAPPDATA` | On Windows, without `SIVI_HOME`, the library lives under `%LOCALAPPDATA%\sivi` |

sivi reads no other variable, no clock, and no font of the host, and only `fonts install` reaches the network.
The PDF bytes depend on the deck, the theme and its files, the images, the bundled and installed fonts, `SOURCE_DATE_EPOCH`, and the sivi version alone.

## The JSON document

`--format json` writes one document, described by [`docs/schema/diagnostics-1.json`](../schema/diagnostics-1.json).

```json
{
  "format": "sivi.diagnostics",
  "version": 1,
  "deck": "deck.md",
  "diagnostics": [
    {
      "code": "E0401",
      "severity": "error",
      "message": "content bound to slot `body` is 82.800pt taller than its region",
      "slide": { "number": 7, "id": "revenue", "master": "bullets" },
      "spans": [
        { "file": "deck.md", "start": { "line": 27, "column": 1 }, "end": { "line": 38, "column": 29 }, "text": "content bound to `body` starts here", "primary": true }
      ],
      "notes": ["region 840.000pt × 324.000pt at (60.000pt, 156.000pt); content needs 406.800pt"],
      "helps": ["remove 3 lines"],
      "doc": "docs/diagnostics.md#e0401"
    }
  ],
  "output": null
}
```

`output` holds the published path, the page count, and the SHA-256 of the PDF after a successful build; a check reports the page count alone.
Every length is in points with three decimals, as the human rendering prints it.
Columns count bytes from one, as the human rendering does.

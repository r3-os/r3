// This [Deno] program scans the compiled API documentation to check for errors.
//
// [Deno]: https://deno.land/
//
// Usage: deno run --allow-read scripts/check-workspace.ts
import { parse as parseFlags } from "https://deno.land/std@0.143.0/flags/mod.ts";
import { walk } from "https://deno.land/std@0.143.0/fs/mod.ts";
import * as path from "https://deno.land/std@0.143.0/path/mod.ts";
import * as log from "https://deno.land/std@0.143.0/log/mod.ts";

const parsedArgs = parseFlags(Deno.args, {
    "alias": {
        h: "help",
        d: "rustdoc-output",
    },
    "string": [
        "rustdoc-output",
    ],
});

if (parsedArgs["help"]) {
    console.log("Arguments:");
    console.log("  -h --help  Displays this message");
    console.log("  -d DIRECTORY --rustdoc-output=DIRECTORY");
    console.log("             Specifies the rustdoc output directory to scan. " +
        "Defaults to `./target/doc` when unspecified.");
}

await log.setup({
    handlers: {
        console: new log.handlers.ConsoleHandler("DEBUG"),
    },

    loggers: {
        default: {
            level: "INFO",
            handlers: ["console"],
        },
    },
});

const logger = log.getLogger();
let hasError = false;
let expectedRepository: string | null = null;

// A code fragment indicating the presence of `common.md`
const COMMON_CSS_FRAGMENT = /\.toc-header \+ ul::before {/g;
// Code fragments that require the presence of `common.md`
const COMMON_CSS_USES = [
    /class="toc-header"/,
    // The negative lookbehind is intended to avoid matching
    // the example code in `common.md`
    /(?<!\* *<div )class="admonition-follows"/,
    /class="disabled-feature-warning"/,
    // The negative lookbehind is intended to avoid matching
    // the example code in `common.md`
    /(?<!\* *<span )class="class"/,
    '<cneter>',
];

await validateRustdocOutput(parsedArgs.d || "./target/doc");

if (hasError) {
    Deno.exit(1);
}

async function validateRustdocOutput(docPath: string): Promise<void> {
    let numFilesScanned = 0;

    logger.info(`Scanning ${docPath}`);
    for await (const { path } of walk(docPath, { includeDirs: false })) {
        if (!path.endsWith(".html") && !path.endsWith(".htm")) {
            continue;
        }

        logger.debug(`# ${path}`);

        const html = await Deno.readTextFile(path);
        numFilesScanned += 1;

        const numCommonCssInstances =
            Array.from(html.matchAll(COMMON_CSS_FRAGMENT)).length;
        if (numCommonCssInstances === 0) {
            // Maybe a redirect page?
            logger.debug(`${path}: Doesn't contain a fragment of 'common.md' - ignoring`);
            continue;
        } else if (numCommonCssInstances >= 2) {
            // `#[doc = ...]` (per-file) + `$RUSTDOCFLAGS` [ref:doc_global_styling]
            logger.debug(`${path}: There's a per-file inclusion of 'common.md' - ignoring`);

            if (numCommonCssInstances > 2) {
                logger.warning(`${path}: Includes too many instances of 'common.md'`);
            }

            continue;
        } else {
            // This file lacks a per-file incluson of `common.md`.
        }

        // To prevent the degradation of the appearance in the absence of the
        // appropriate `$RUSTDOCFLAGS`, this page must be devoid of constructs
        // that make use of the styling rules defined by `common.md`.
        for (const cssUsage of COMMON_CSS_USES) {
            if (html.match(cssUsage)) {
                logger.error(`${path}: This file lacks a per-file inclusion of 'common.md', ` +
                    `but includes a code fragment '${cssUsage}'`);
                hasError = true;
            }
        }
    }

    logger.info(`${numFilesScanned} file(s) have been checked`);
} // validateRustdocOutput

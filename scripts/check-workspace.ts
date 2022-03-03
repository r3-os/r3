// This [Deno] program checks the conformance of this workspace's Cargo
// metadata to the coding guidelines.
//
// This script also lists crates to be published.
//
// [Deno]: https://deno.land/
//
// Usage: deno run --allow-read scripts/check-workspace.ts
import { parse as parseFlags } from "https://deno.land/std@0.125.0/flags/mod.ts";
import { parse as parseToml } from "https://deno.land/std@0.125.0/encoding/toml.ts";
import * as path from "https://deno.land/std@0.125.0/path/mod.ts";
import * as log from "https://deno.land/std@0.125.0/log/mod.ts";
import { exists } from "https://deno.land/std@0.125.0/fs/exists.ts";

const parsedArgs = parseFlags(Deno.args, {
    "alias": {
        h: "help",
        w: "workspace",
    },
    "string": [
        "workspace",
    ],
});

if (parsedArgs["help"]) {
    console.log("Arguments:");
    console.log("  -h --help  Displays this message");
    console.log("  -w WORKSPACE --workspace=WORKSPACE");
    console.log("             Specifies the workspace directory. Defaults to `.` when unspecified.");
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

const EXPECTED_SOURCE_FRAGMENTS = [
    // We want published crates to have consistent logos, which should
    // appear conditionally [ref:doc_feature]
    `#![cfg_attr(
    feature = "doc",
    doc(html_logo_url = "https://r3-os.github.io/r3/logo-small.svg")
)]`,
];

const COMMON_HEADER_PATH = "src/common.md";
const EXPECTED_RUSTDOC_ARGS = `--html-in-header ${COMMON_HEADER_PATH}`;

const logger = log.getLogger();
let hasError = false;
let expectedRepository: string | null = null;

await validateWorkspace(parsedArgs.w || ".");

if (hasError) {
    Deno.exit(1);
}

async function validateWorkspace(workspacePath: string): Promise<void> {
    // Read the workspace's `Cargo.toml` file
    const workspaceMetaPath = path.join(workspacePath, "Cargo.toml");
    logger.debug(`Reading workspace metadata from '${workspaceMetaPath}`);
    const workspaceMeta: CargoMeta =
        parseToml(cleanToml(await Deno.readTextFile(workspaceMetaPath))) as any;

    if (!workspaceMeta.workspace || !workspaceMeta.workspace.members) {
        logger.error("'.workspace.members' is missing from the workspace metadata.");
        hasError = true;
        return;
    }

    for (const crateRelPath of workspaceMeta.workspace.members) {
        const cratePath = path.join(workspacePath, crateRelPath);
        const crateMetaPath = path.join(cratePath, "Cargo.toml");
        const crateRootSourcePath = path.join(cratePath, "src/lib.rs");
        logger.debug(`Reading crate metadata from '${crateMetaPath}'`);
        const crateMeta: CargoMeta =
            parseToml(cleanToml(await Deno.readTextFile(crateMetaPath))) as any;

        if (!crateMeta.package) {
            logger.error(`${crateRelPath}: '.package' is missing from the crate metadata.`);
            hasError = true;
            continue;
        }

        const {package: pkg, dependencies = {}, features = {}} = crateMeta;
        const {publish = true, version} = pkg;

        // CC-VER-UNPUBLISHED
        if (!publish && pkg.version !== '0.0.0') {
            logger.error(`${crateRelPath}: '.package.version' must be '0.0.0' for an unpublished crate.`);
            hasError = true;
        } else if (publish && pkg.version === '0.0.0') {
            logger.error(`${crateRelPath}: '.package.version' must not be '0.0.0' for a published crate.`);
            hasError = true;
        }

        // Log published crates
        if (publish) {
            logger.info(`${crateRelPath}: version ${pkg.version}`);
        }

        // Published crates must have versioned dependencies
        for (const [name, dep] of Object.entries(dependencies)) {
            const depEx = typeof dep === "string" ? {version: dep} : dep;
            if (publish && depEx.version == null) {
                logger.error(`${crateRelPath}: Dependency '${name}' must have a version ` +
                    `specification because ${pkg.name} is a published crate.`);
                hasError = true;
            }
        }

        // We want published crates to have sufficient metadata
        if (publish) {
            if (pkg.license == null) {
                logger.error(`${crateRelPath}: '.package.license' must be set for a published crate.`);
                hasError = true;
            }
            if (pkg.description == null) {
                logger.error(`${crateRelPath}: '.package.description' must be set for a published crate.`);
                hasError = true;
            }
            if (pkg.keywords == null) {
                logger.error(`${crateRelPath}: '.package.keywords' must be set for a published crate.`);
                hasError = true;
            }
            if (pkg.repository == null) {
                logger.error(`${crateRelPath}: '.package.repository' must be set for a published crate.`);
                hasError = true;
            } else if (pkg.repository !== (expectedRepository = expectedRepository ?? pkg.repository)) {
                logger.error(`${crateRelPath}: '.package.repository' must be consistent across the ` +
                    `workspace. The first found value is '${expectedRepository}'.`);
                hasError = true;
            }
        }

        if (publish) {
            logger.debug(`Reading a source file at '${crateRootSourcePath}'`);
            const rootSource = await Deno.readTextFile(crateRootSourcePath);

            for (const text of EXPECTED_SOURCE_FRAGMENTS) {
                if (rootSource.indexOf(text) < 0) {
                    logger.error(`${crateRelPath}: ${crateRootSourcePath} doesn't ` +
                        `include the text '${text}'.`);
                    hasError = true;
                }
            }
        }

        // We want `common.md` applied for the entire crate in order that the
        // upper-left logo is properly styled [tag:doc_global_styling]
        const docsMetadata = pkg?.metadata?.docs?.rs ?? {};
        if (publish) {
            const docsArgs = docsMetadata['rustdoc-args'] ?? [];

            // Poor man's `docsArgs.windows(2).any(|x| x == ...)`
            const docsArgsConcat = docsArgs.join(' ');
            if (docsArgsConcat.indexOf(EXPECTED_RUSTDOC_ARGS) < 0) {
                logger.error(`${crateRelPath}: package.metadata.docs.rs.rustdoc-args doesn't ` +
                    `include '${EXPECTED_RUSTDOC_ARGS}'.`);
                hasError = true;
            }

            // The file referenced by `EXPECTED_RUSTDOC_ARGS` should actually
            // exist
            const commonHeaderPath  = path.join(cratePath, COMMON_HEADER_PATH);
            if (!await exists(commonHeaderPath)) {
                logger.error(`${crateRelPath}: '${commonHeaderPath}' doesn't exist.`);
                hasError = true;
            }
        }

        // The custom logo needs a custom stylesheet [ref:doc_global_styling],
        // so it must be enabled conditionally. The `doc` Cargo feature is used
        // to toggle this. [tag:doc_feature]
        //
        // This means that the appearance will be degraded when `doc` is used
        // alone without the appropriate stylesheet, but this can only happen
        // in "unofficial" documentation builds.
        if (publish) {
            if (typeof features.doc == "undefined") {
                logger.error(`${crateRelPath}: features.doc is not present.`);
                hasError = true;
            }
        }

        // The enabled features must be consistent between docs.rs and
        // our API documentation website [ref:doc_all_features]
        if (publish && !docsMetadata["all-features"]) {
            logger.error(`${crateRelPath}: package.metadata.docs.rs.all-features is ` +
                `not set.`);
        }
    }
}

interface CargoMeta {
    workspace?: {
        members?: string[],
    },
    "package"?: {
        name: string,
        version: string,
        authors: string[],
        readme?: string,
        edition?: string,
        license?: string,
        description?: string,
        categories?: string[],
        keywords?: string[],
        repository?: string,
        publish?: boolean,
        metadata?: {
            docs?: {
                rs?: {
                    "rustdoc-args"?: string[],
                    "all-features"?: boolean,
                },
            },
        },
    },
    features?: { [name: string]: string[] },
    dependencies?: { [name: string]: Dep },
    "dev-dependencies"?: { [name: string]: Dep },
}

type Dep = DepEx | string;

interface DepEx {
    version?: string,
    path?: string,
    optional?: boolean,
    features?: string[],
}

/**
 * Sanitize the given TOML encoded data for `std/encoding/toml.ts`.
 */
function cleanToml(source: string): string {
    // Remove comments. The parser doen't like them in array literals.
    source = source.replace(/#.*/g, '');
    return source;
}

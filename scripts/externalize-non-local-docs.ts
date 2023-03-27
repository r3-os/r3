// This [Deno] program scans `target/doc` directory and replaces the non-local
// crate documentation with redirect pages to docs.rs.
//
// [Deno]: https://deno.land/
//
// Usage: deno run -A scripts/externalize-non-local-docs.ts
import { parse as parseFlags } from "https://deno.land/std@0.181.0/flags/mod.ts";
import { parse as parseToml } from "https://deno.land/std@0.181.0/encoding/toml.ts";
import * as path from "https://deno.land/std@0.181.0/path/mod.ts";
import * as log from "https://deno.land/std@0.181.0/log/mod.ts";
import { walk } from "https://deno.land/std@0.181.0/fs/walk.ts";
import * as semver from "https://deno.land/x/semver@v1.4.0/mod.ts";

const parsedArgs = parseFlags(Deno.args, {
    "alias": {
        h: "help",
        w: "workspace",
        y: "apply",
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
    console.log("  -y --apply Actually make modification");
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

// This script is quite destructive, so as a safety measure, we don't make
// changes unless we're expressly told to do so.
const shouldModify: boolean = parsedArgs.y;

if (!shouldModify) {
    logger.warning("Performing a dry run because the `--apply` flag was not given");
}

// Get the workspace metadata <https://doc.rust-lang.org/1.58.1/cargo/commands/cargo-metadata.html>
const workspaceMetaJson = await (async () => {
    if ((parsedArgs.w || ".") !== ".") {
        logger.error("Unsupported: Operating on a workspace outside the current "
            + "directory is not supported.");
        Deno.exit(1);
    }
    const process = Deno.run({
        cmd: [
            "cargo", "metadata", "--format-version=1", "--all-features",
        ],
        stdout: "piped",
    });
    const [stdoutBytes, status] = await Promise.all([process.output(), process.status()]);
    if (!status.success) {
        Deno.exit(status.code);
    }
    return new TextDecoder().decode(stdoutBytes);
})();
const workspaceMeta: CargoMetadataV1 = JSON.parse(workspaceMetaJson);

type CargoMetadataV1 = {
    packages: PackageMetadataV1[],
};
type PackageMetadataV1 = {
    name: string,
    version: string,
    source: string | null,
    targets: {
        kind: string[],
        crate_types: string[],
        name: string,
    }[],
};

// Calculate the mapping from crate names to docs.rs URLs
const crateNameToPackage = new Map<string, PackageMetadataV1[]>();
logger.debug("Constructing a mapping from crate names to packages");
for (const pkg of workspaceMeta.packages) {
    logger.debug(` - ${pkg.name} ${pkg.version}`);

    const libraryTarget = pkg.targets.find(t => t.kind.find(k => k == "lib" || k == "proc-macro"));
    if (libraryTarget == null) {
        logger.debug("It doesn't provide a library crate - ignoring");
        continue;
    }

    const crateName = libraryTarget.name.replace(/-/g, '_');
    logger.debug(`Crate name = ${crateName}`);
    if (!crateNameToPackage.has(crateName)) {
        crateNameToPackage.set(crateName, []);
    }
    crateNameToPackage.get(crateName)!.push(pkg);
}
if (crateNameToPackage.size === 0) {
    logger.error("The crate name mapping is empty - something is wrong.");
    Deno.exit(1);
}

// Scan the built documentation directory
const docPath = path.join(parsedArgs.w || ".", "target/doc");

for await (const entry of Deno.readDir(docPath)) {
    if (entry.isDirectory && entry.name !== "src" && entry.name !== "implementors") {
        await processCrateDocumentation(
            path.join(docPath, entry.name),
            `/${entry.name}`,
            entry.name,
        );
    }
}
for await (const entry of Deno.readDir(path.join(docPath, "src"))) {
    if (entry.isDirectory) {
        await processCrateDocumentation(
            path.join(docPath, "src", entry.name),
            `/src/${entry.name}`,
            entry.name,
        );
    }
}

async function processCrateDocumentation(docPath: string, relPath: string, crateName: string) {
    const packages = crateNameToPackage.get(crateName) ?? [];

    // Ignore non-crates.io packages
    if (packages.find(p => p.source !== "registry+https://github.com/rust-lang/crates.io-index")) {
        logger.debug(`${docPath}: It might be a non-crates.io package, ignoring`);
        return;
    }

    if (packages.length === 0) {
        logger.warning(`${docPath}: Unknown crate, ignoring`);
        return;
    }

    if (crateName.startsWith("r3_")) {
        logger.warning(`${docPath}: It starts with 'r3_' but is about to ` +
            `be processed - something might be wrong.`);
        return;
    }

    // If there are multiple candidates, choose the one with the highest version
    const pkg = packages.reduce((x, y) => semver.gt(x.version, y.version) ? x : y);
    if (packages.length > 1) {
        const candidates = JSON.stringify(packages.map(p => p.version));
        logger.info(`${docPath}: Choosing ${pkg.version} from the candidate(s) ${candidates}`);
    }

    const externalDocBaseUrl = `https://docs.rs/${pkg.name}/${pkg.version}${relPath}`;

    logger.info(`${docPath}: Externalizing the documentation to '${externalDocBaseUrl}'`);

    const files = [];
    for await (const entry of walk(docPath, { includeDirs: false })) {
        if (!entry.name.endsWith('.html')) {
            continue;
        }
        logger.debug(`${docPath}: ${entry.path}`);
        if (!entry.path.startsWith(docPath)) {
            throw new Error();
        }
        files.push(entry.path);
    }
    logger.info(`${docPath}: Replacing ${files.length} file(s) with redirect pages`);

    logger.debug(`${docPath}: Removing the directory`);
    if (shouldModify) {
        await Deno.remove(docPath, { recursive: true });
    }

    for (const filePath of files) {
        const url = externalDocBaseUrl + filePath.substring(docPath.length);
        logger.debug(`${docPath}: ${filePath} → ${url}`);
        if (shouldModify) {
            await Deno.mkdir(path.dirname(filePath), { recursive: true });
            await Deno.writeTextFile(filePath, redirectingHtmlCode(url));
        }
    }
}

function redirectingHtmlCode(url: string): string {
    return `<!DOCTYPE html><html><head><meta http-equiv="refresh" content="0; url=${url}">`;
}
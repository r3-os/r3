// This [Deno] uses `r3_test_runner` to create a size report of the test
// program.
//
// [Deno]: https://deno.land/
import { parse as parseFlags } from "https://deno.land/std@0.75.0/flags/mod.ts";
import * as path from "https://deno.land/std@0.75.0/path/mod.ts";
import * as log from "https://deno.land/std@0.75.0/log/mod.ts";
import { BufReader } from "https://deno.land/std@0.75.0/io/bufio.ts";
import AsciiTable from "https://deno.land/x/ascii_table@v0.1.0/mod.ts";

const ENV_SIZE = "REPORT_SIZE_SIZE";
const ENV_TEST_NAME = "R3_TEST"; // should be synched with `r3_test_runner`!

const SAMPLE_MARKER = "### ";

interface Sample {
    name: string,
    text: number,
    data: number,
    bss: number,
}

const parsedArgs = parseFlags(Deno.args, {
    "alias": {
        h: "help",
    },
    "string": [
        "size",
        "exe-handler",
    ],
    "boolean": [
        "help",
    ],
    "--": true,
});

if (parsedArgs["help"]) {
    console.log("Arguments:");
    console.log("  -h --help     Displays this message");
    console.log("  -s --size=... Path to the `size` utility from binutils");
    console.log("  -- ARGS...    Passed to `r3_test_runner`");
}

if (parsedArgs["exe-handler"]) {
    // This argument is set when this script is called back by `r3_test_runner`.
    // The argument value contains the executable's path.
    const exePath = parsedArgs["exe-handler"];

    const sizePath = Deno.env.get(ENV_SIZE);
    if (!sizePath) {
        throw new Error(`\$${ENV_SIZE} is not set`);
    }

    const testName = Deno.env.get(ENV_TEST_NAME);
    if (!testName) {
        throw new Error(`\$${ENV_TEST_NAME} is not set`);
    }

    const process = Deno.run({
        cmd: [sizePath, exePath],
        stdout: "piped",
    });
    const [stdoutBytes, status] = await Promise.all([process.output(), process.status()]);
    if (!status.success) {
        Deno.exit(status.code);
    }
    const stdout = new TextDecoder().decode(stdoutBytes);

    const lines = stdout.split('\n');
    if (lines.length < 2) {
        throw new Error(`\`${sizePath}\` did not produce an expected output: ${stdout}`);
    }
    const matches = lines[1].match(/^\s*([0-9]+)\s+([0-9]+)\s+([0-9]+)/);
    if (matches == null) {
        throw new Error(`\`${sizePath}\` did not produce an expected output line: ${lines[1]}`);
    }
    const [_, text, data, bss] = matches;

    console.log(SAMPLE_MARKER + JSON.stringify({
        name: testName,
        text: parseInt(text, 10),
        data: parseInt(data, 10),
        bss: parseInt(bss, 10),
    } as Sample));

    Deno.exit();
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

const denoPath = Deno.execPath();
const selfPath = path.join(Deno.cwd(), "scripts", "report-size.ts");

const process = Deno.run({
    cmd: [
        "cargo", "run", "-p", "r3_test_runner", "--",
        // Call back to this script
        "--exec", denoPath, "run", "-A", selfPath, "--exe-handler", "{}", ";",
        "--norun",
        "--small-rt",
        "-l", "off",
    ]
        .concat(parsedArgs['--']),
    env: {
        // `ENV_SIZE` is taken by this script (`report_size.ts`) when
        // it's called back. The default value uses
        // <https://crates.io/crates/cargo-binutils>.
        [ENV_SIZE]: parsedArgs['size'] ?? 'rust-size',
    },
    // Capture the output of the callback invocations
    stdout: "piped",
});
const [stdoutBytes, status] = await Promise.all([process.output(), process.status()]);
if (!status.success) {
    Deno.exit(status.code);
}
const stdout = new TextDecoder().decode(stdoutBytes);

// Extract the outputs of the callback invocations
const samples = [];
for (const line of stdout.split("\n")) {
    if (line.startsWith(SAMPLE_MARKER)) {
        const sampleJson = line.substring(SAMPLE_MARKER.length);
        logger.debug(`Found a sample: ${sampleJson}`);
        samples.push(JSON.parse(sampleJson) as Sample);
    }
}

// Markdown header
console.log("Test runner parameters: `" + parsedArgs['--'].join(' ') + "`");
console.log();

function emitTable(samples: ReadonlyArray<Sample>) {
    // Generate the table in GFM (https://github.github.com/gfm/#tables-extension-)
    const table = new AsciiTable()
        .removeBorder()
        .setBorder('|', '-', '|', '|')
        .setHeading("Name", "`.text`", "`.data`","`.bss`");

    for (const sample of samples) {
        const name = sample.name.startsWith('(') ? sample.name : "`" + sample.name + "`";
        table.addRow(name, sample.text, sample.data, sample.bss);
    }

    let tableStr = table.toString();
    tableStr = tableStr.substring(tableStr.indexOf("\n") + 1);
    tableStr = tableStr.substring(0, tableStr.lastIndexOf("\n"));

    console.log(tableStr);
}

// Summary table
function summarizeSamples(
    name: string,
    samples: ReadonlyArray<Sample>,
    func: (...values: number[]) => number,
): Sample {
    const out: Sample = { name, text: 0, data: 0, bss: 0 };
    for (const prop of ["text", "data", "bss"] as ("text" | "data" | "bss")[]) {
        out[prop] = func.apply(null, samples.map(x => x[prop]));
    }
    return out;
}

emitTable([
    summarizeSamples("(Min)", samples, Math.min),
    summarizeSamples("(Max)", samples, Math.max),
]);

// Full table
console.log();
console.log("<details>");
console.log("<summary>Full report</summary>");
console.log();
emitTable(samples);
console.log();
console.log("</details>");

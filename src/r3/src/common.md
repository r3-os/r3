<style type="text/css">
/* Cover image, something that is definitely relevant to the subject */
.distractor {
    margin: 0 auto;
    max-width: 600px;
}
.distractor > a {
    display: block;
    background-size: cover;
    /* background: ...; — specified by inline style */
    /* padding-bottom: ...; — specified by inline style */
}

/* Table of contents */
.toc-header + ul {
    background: rgba(128, 128, 128, 0.1);
    margin: 1em 0; padding: 1em;
    width: 40%;
    min-width: 280px;
    border: 1px solid rgba(128, 128, 128, 0.2);
}
.toc-header + ul::before {
    content: "Contents";
    font-weight: bold;
}
.toc-header + ul ul { margin-bottom: 0; }
.toc-header + ul li { list-style: none; }

/* Poor man's admonition
 *
 * # Usage
 *
 * ## Normal
 *
 *     <div class="admonition-follows"></div>
 *
 *     > **Title:** lorem ipsum lorem ipsum lorem ipsum lorem ipsum lorem ipsum
 *     > lorem ipsum
 *
 * ## Collapsible
 *
 *     <div class="admonition-follows"></div>
 *
 *     > <details>
 *     > <summary>Title</summary>
 *     >
 *     > lorem ipsum lorem ipsum lorem ipsum lorem ipsum lorem ipsum lorem ipsum
 *     >
 *     > </details>
 */
.admonition-follows + blockquote {
    background: rgba(128, 128, 128, 0.1) !important;
    margin: 1em !important; padding: 1em 1em 0 !important;
    color: inherit !important; overflow: hidden;
}
.admonition-follows + blockquote::after { /* collapsible padding */
    content: ""; display: block; margin-top: 1em;
}

.admonition-follows + blockquote summary {
    cursor: pointer;
    will-change: opacity;
    user-select: none;
    -webkit-user-select: none;
    font-weight: bold;
}
.admonition-follows + blockquote summary:not([open]):not(:hover) {
    opacity: 0.5;
}
.admonition-follows + blockquote summary + * {
    margin-top: 1em;
}

/* Display a warning if some Cargo features are disabled. */
.disabled-feature-warning > p > span:before { content: "Warning:"; font-weight: bold; }
.disabled-feature-warning > p > span:after { content: " This documentation was built without a "; }
.disabled-feature-warning > p > code:before { content: "--all-features"; }
.disabled-feature-warning > p:after { content: " build option. Some items might be missing."; }

/* Center an inline image
 *
 * # Usage
 *
 *    <span class="center">![kernel-traits]</span>
 *
 *    [kernel-traits]: data:image/svg+xml;base64,< super long base64 data >
 *
 * The image wouldn't render if `![kernel-traits]` were wrapped with `<center>`
 * because the Markdown processor doesn't do Markdown processing inside a block
 * element. On the other hand, it thinks `<span>` is an inline element (by
 * default), so markups inside `<span>` are processed.
 *
 * For diagrams processed by `::svgbobdoc`, just use `<center>`.
 */
span.center {
    display: block;
    text-align: center;
}

/* Add margins to SvgBob images */
span.center img, center img {
    border: 10px solid white;
}

/* Auto-invert SvgBob images in a dark theme */
body.theme-dark span.center img, body.theme-dark center img {
    filter: invert(88%);
}

body.theme-ayu span.center img, body.theme-ayu center img {
    filter: invert(89%) sepia(90%) hue-rotate(180deg);
}
</style>
<script type="application/javascript">
<!--
// Monitors the current rustdoc theme and adds `.theme-NAME` to `<body>`
function initThemeMonitor() {
    if (typeof switchTheme !== 'function' ||
        typeof themeStyle !== 'object' ||
        typeof document.body.classList === 'undefined')
    {
        // Something is wrong, don't do anything
        return;
    }

    var currentClassName = null;
    function onApplyTheme(name) {
        if (currentClassName != null) {
            document.body.classList.remove(currentClassName);
        }
        currentClassName = "theme-" + name;
        document.body.classList.add(currentClassName);
    }

    var match = themeStyle.href.match(/([a-z]+)\.css$/);
    var currentStyle = (match && match[1]) || "light";
    onApplyTheme(currentStyle);

    // Intercept calls to `switchTheme`
    var originalSwitchTheme = switchTheme;
    switchTheme = function (_0, _1, newTheme) {
        onApplyTheme(newTheme);
        originalSwitchTheme.apply(this, arguments);
    };
}

if (document.readyState === 'interactive' || document.readyState === 'complete') {
    initThemeMonitor();
} else {
    document.addEventListener('DOMContentLoaded', initThemeMonitor);
}
-->
</script>
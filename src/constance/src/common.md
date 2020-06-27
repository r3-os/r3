<style type="text/css">
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
 *     <div class="admonition-follows"></div>
 *
 *     > **Title:** lorem ipsum lorem ipsum lorem ipsum lorem ipsum lorem ipsum
 *     > lorem ipsum
 */
.admonition-follows + blockquote {
    background: rgba(128, 128, 128, 0.1) !important;
    margin: 1em !important; padding: 1em 1em 0 !important;
    color: inherit !important; overflow: hidden;
}
.admonition-follows + blockquote::after { /* collapsible padding */
    content: ""; display: block; margin-top: 1em;
}

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
</style>

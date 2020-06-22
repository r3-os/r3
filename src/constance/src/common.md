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
</style>

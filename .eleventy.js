const fs = require("node:fs/promises");
const path = require("node:path");
const MarkdownIt = require("markdown-it");

const guidePath = path.join(__dirname, "docs/guide.md");
const mogLanguagePath = path.join(__dirname, "site", "shiki-languages", "mog.tmLanguage.json");

function mapLanguage(rawLang) {
  const normalized = (rawLang || "").trim().toLowerCase().split(/\s+/)[0] || "text";
  if (normalized === "mog") {
    return "mog";
  }
  if (normalized === "sh") {
    return "bash";
  }
  if (normalized === "shell") {
    return "bash";
  }
  if (normalized === "md" || normalized === "markdown") {
    return "markdown";
  }
  return normalized;
}

module.exports = async function (eleventyConfig) {
  const mogLanguage = JSON.parse(await fs.readFile(mogLanguagePath, "utf8"));
  if (!mogLanguage.id) {
    mogLanguage.id = "mog";
  }
  const { createHighlighter } = await import("shiki");
  const highlighter = await createHighlighter({
    themes: ["github-dark", "github-light"],
    langs: [
      "text",
      "typescript",
      "javascript",
      "rust",
      "bash",
      "json",
      "toml",
      "markdown",
      "css",
      "html",
      "sql",
      "c",
      "cpp",
      "python",
      "yaml",
      "dockerfile",
      "diff",
      mogLanguage
    ]
  });

  const md = new MarkdownIt({
    html: true,
    linkify: true,
    typographer: true
  });

  md.renderer.rules.fence = function (tokens, idx, options, env) {
    const token = tokens[idx];
    const requestedLang = token.info ? token.info.trim() : "";
    const rawLang = requestedLang.split(/\s+/)[0] || "text";
    const highlighterLang = mapLanguage(rawLang);
    const theme = "github-dark";
    const code = token.content.replace(/\n$/, "");

    try {
      return highlighter.codeToHtml(code, {
        lang: highlighterLang,
        theme
      });
    } catch (_error) {
      return `<pre><code class="language-${rawLang || "text"}">${md.utils.escapeHtml(
        code
      )}</code></pre>`;
    }
  };

  eleventyConfig.setLibrary("md", md);

  eleventyConfig.addGlobalData("guideHtml", async () => {
    const guideMarkdown = await fs.readFile(guidePath, "utf8");
    return md.render(guideMarkdown);
  });

  eleventyConfig.addPassthroughCopy("site/styles.css");
  eleventyConfig.addPassthroughCopy("site/CNAME");
  eleventyConfig.addWatchTarget("docs/guide.md");

  return {
    dir: {
      input: "site",
      includes: "_includes",
      layouts: "_layouts",
      output: "_site"
    },
    templateFormats: ["njk", "md", "html"],
    markdownTemplateEngine: "njk",
    htmlTemplateEngine: "njk"
  };
};

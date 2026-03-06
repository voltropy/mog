const fs = require("node:fs/promises");
const path = require("node:path");
const MarkdownIt = require("markdown-it");

const guidePath = path.join(__dirname, "docs/guide.md");
const introductionPath = path.join(__dirname, "site", "introduction.md");
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

function slugifyHeading(text) {
  const normalized = String(text || "")
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, "")
    .trim()
    .replace(/\s+/g, "-");
  const collapsed = normalized.replace(/-+/g, "-");
  return collapsed.replace(/^-+|-+$/g, "") || "section";
}

function makeUniqueSlug(base, seen) {
  const baseSlug = slugifyHeading(base);
  const count = seen.get(baseSlug) || 0;
  seen.set(baseSlug, count + 1);
  return count === 0 ? baseSlug : `${baseSlug}-${count + 1}`;
}

function buildDocumentArtifacts(markdown, mdRenderer) {
  const tokens = mdRenderer.parse(markdown, {});
  const used = new Map();
  const guideToc = [];

  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    if (token.type !== "heading_open") {
      continue;
    }

    const next = tokens[i + 1];
    if (!next || next.type !== "inline") {
      continue;
    }

    const level = Number(token.tag.substring(1));
    const title = next.content.trim();
    if (!title) {
      continue;
    }

    const id = makeUniqueSlug(title, used);
    token.attrSet("id", id);

    if (level >= 1 && level <= 4) {
      guideToc.push({ id, level, title });
    }
  }

  const guideHtml = mdRenderer.renderer.render(tokens, mdRenderer.options, {});
  return { guideHtml, guideToc };
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
        includeExplanation: true,
        theme,
        transformers: [
          {
            tokens(tokenLines) {
              for (const line of tokenLines) {
                for (const token of line) {
                  const scopeNames = (token.explanation || [])
                    .flatMap((explanation) => explanation.scopes || [])
                    .map((scope) => scope.scopeName);

                  if (
                    scopeNames.some((scopeName) => scopeName.includes("variable.interpolation.mog"))
                  ) {
                    token.color = "#ffd86a";
                    token.htmlStyle = {
                      ...(token.htmlStyle || {}),
                      color: "#ffd86a"
                    };
                  }
                }
              }

              return tokenLines;
            }
          }
        ]
      });
    } catch (_error) {
      return `<pre><code class="language-${rawLang || "text"}">${md.utils.escapeHtml(
        code
      )}</code></pre>`;
    }
  };

  let guideArtifacts = null;
  let guideMtime = 0;
  let introductionArtifacts = null;
  let introductionMtime = 0;

  async function getGuideArtifacts() {
    const stats = await fs.stat(guidePath);
    if (guideArtifacts && guideMtime === stats.mtimeMs) {
      return guideArtifacts;
    }

    const guideMarkdown = await fs.readFile(guidePath, "utf8");
    guideArtifacts = buildDocumentArtifacts(guideMarkdown, md);
    guideMtime = stats.mtimeMs;
    return guideArtifacts;
  }

  async function getIntroductionArtifacts() {
    const stats = await fs.stat(introductionPath);
    if (introductionArtifacts && introductionMtime === stats.mtimeMs) {
      return introductionArtifacts;
    }

    const introductionMarkdown = await fs.readFile(introductionPath, "utf8");
    introductionArtifacts = buildDocumentArtifacts(introductionMarkdown, md);
    introductionMtime = stats.mtimeMs;
    return introductionArtifacts;
  }

  eleventyConfig.setLibrary("md", md);

  eleventyConfig.addGlobalData("guideHtml", async () => {
    const { guideHtml } = await getGuideArtifacts();
    return guideHtml;
  });

  eleventyConfig.addGlobalData("guideToc", async () => {
    const { guideToc } = await getGuideArtifacts();
    return guideToc;
  });

  eleventyConfig.addGlobalData("introductionHtml", async () => {
    const { guideHtml } = await getIntroductionArtifacts();
    return guideHtml;
  });

  eleventyConfig.addGlobalData("introductionToc", async () => {
    const { guideToc } = await getIntroductionArtifacts();
    return guideToc;
  });

  eleventyConfig.addPassthroughCopy("site/styles.css");
  eleventyConfig.addPassthroughCopy("site/CNAME");
  eleventyConfig.addPassthroughCopy("site/assets");
  eleventyConfig.addWatchTarget("docs/guide.md");
  eleventyConfig.addWatchTarget("site/introduction.md");

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

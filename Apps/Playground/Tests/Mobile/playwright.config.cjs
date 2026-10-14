const path = require("node:path");
const { defineConfig } = require("@playwright/test");
const outputDir = process.env.MOBILE_OUTPUT || path.join(__dirname, "mobile-results");

module.exports = defineConfig({
    testDir: __dirname,
    testMatch: "visual.spec.cjs",
    workers: 1,
    retries: 0,
    timeout: 90 * 60_000,
    globalTimeout: 95 * 60_000,
    updateSnapshots: "none",
    outputDir,
    use: { trace: "off", video: "off", screenshot: "off" },
    reporter: [["line"], ["json", { outputFile: path.join(outputDir, "playwright-results.json") }]]
});

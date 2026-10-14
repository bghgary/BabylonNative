const path = require("node:path");
const fs = require("node:fs");
const { spawn } = require("node:child_process");

if (!process.env.BABYLON_JS_ROOT) {
    throw new Error("Set BABYLON_JS_ROOT to the Babylon.js checkout containing the shared native Playwright suite");
}
const root = path.resolve(process.env.BABYLON_JS_ROOT);
const cli = require.resolve("@playwright/test/cli", { paths: [root] });
const args = process.argv.slice(2);
if (process.env.MOBILE_TEST_INDICES) {
    throw new Error("MOBILE_TEST_INDICES selected the removed native catalog; use Playwright --grep instead");
}
if (args.some(arg => /^--(?:update-snapshots|config|project)(?:=|$)/.test(arg) || ["-c", "-p", "-u"].includes(arg))) {
    throw new Error("The shared native project, configuration and unchanged snapshots are fixed by this runner");
}
if (process.env.MOBILE_TEST_GREP) {
    args.push("--grep", process.env.MOBILE_TEST_GREP);
}
if (process.env.MOBILE_SHARD) {
    args.push("--shard", process.env.MOBILE_SHARD);
}
async function main() {
    const env = { ...process.env };
    let uploadedApp;
    if (!args.some(arg => ["--list", "--help", "-h", "--version", "-V"].includes(arg))) {
        const { uploadApp } = require("./browserstack.cjs");
        uploadedApp = await uploadApp({ app: env.MOBILE_APP });
        env.MOBILE_UPLOADED_APP = JSON.stringify(uploadedApp);
    }
    const child = spawn(process.execPath, [cli, "test", "--config", path.join(__dirname, "playwright.config.cjs"), ...args], {
        cwd: root, stdio: "inherit", env
    });
    child.once("error", error => {
        console.error(error);
        process.exitCode = 1;
    });
    child.once("exit", (code, signal) => {
        if (uploadedApp) {
            const output = env.MOBILE_OUTPUT || path.join(__dirname, "mobile-results");
            fs.mkdirSync(output, { recursive: true });
            const { bytes, sha256, uploadMs } = uploadedApp;
            fs.writeFileSync(path.join(output, "app-upload.json"), JSON.stringify({ bytes, sha256, uploadMs }, null, 2));
        }
        if (signal) {
            console.error(`Native Playwright runner terminated by ${signal}`);
        }
        process.exitCode = code ?? 1;
    });
}
main().catch(error => {
    console.error(error);
    process.exitCode = 1;
});

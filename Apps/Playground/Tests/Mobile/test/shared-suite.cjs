const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { test } = require("node:test");
const assert = require("node:assert/strict");
const { PNG } = require("pngjs");

if (!process.env.BABYLON_JS_ROOT) {
    throw new Error("BABYLON_JS_ROOT is required for the shared Playwright integration controls");
}
const root = path.resolve(process.env.BABYLON_JS_ROOT);
const cli = require.resolve("@playwright/test/cli", { paths: [root] });

test("the actual shared suite owns scene assertions, filtering, retries and worker replacement", t => {
    const directory = fs.mkdtempSync(path.join(__dirname, "shared-suite-results-"));
    t.after(() => fs.rmSync(directory, { recursive: true }));
    const events = path.join(directory, "sessions.jsonl");
    const marker = path.join(directory, "first-failure");
    const transport = path.join(directory, "transport.cjs");
    const config = path.join(directory, "playwright.config.cjs");
    const catalog = path.join(directory, "catalog.json");
    fs.writeFileSync(transport, `
        const {createTransport} = require(${JSON.stringify(path.resolve(__dirname, "..", "transport.cjs"))});
        const {sessionFactory} = require(${JSON.stringify(path.join(__dirname, "fixtures", "session.cjs"))});
        exports.createTransport = (onConsole, signal) => createTransport(onConsole, signal, {
            sessionFactory: sessionFactory({events: ${JSON.stringify(events)}, retryMarker: ${JSON.stringify(marker)}}).factory,
            auth: {redact: text => text}
        });
    `);
    fs.writeFileSync(config, `
        const config = require(${JSON.stringify(path.resolve(__dirname, "..", "playwright.config.cjs"))});
        process.env.BABYLON_NATIVE_TRANSPORT = ${JSON.stringify(transport)};
        module.exports = {
            ...config,
            snapshotPathTemplate: ${JSON.stringify(path.join(directory, "{arg}{ext}"))},
            reporter: [["json", {outputFile: ${JSON.stringify(path.join(directory, "report.json"))}}]]
        };
    `);
    fs.writeFileSync(catalog, JSON.stringify({ root: "https://fixture.invalid/", tests: [
        { title: "Native transport passing scene", scriptToRun: "pass", functionToCall: "createScene", referenceImage: "fixture.png", renderCount: 3 },
        { title: "Native transport retry scene", scriptToRun: "retry", functionToCall: "createScene", referenceImage: "fixture.png", renderCount: 2 }
    ] }));
    const data = Buffer.alloc(600 * 400 * 4);
    for (let i = 0; i < data.length; i += 4) {
        data.set([1, 2, 3, 255], i);
    }
    fs.writeFileSync(path.join(directory, "fixture.png"), PNG.sync.write({ width: 600, height: 400, data }));
    const env = {
        ...process.env, MOBILE_PLATFORM: "android", MOBILE_OUTPUT: path.join(directory, "output"),
        CONFIG_PATH: catalog, CDN_BASE_URL: "https://fixture.invalid/", TIMEOUT: "10000"
    };
    function run(args) {
        const result = spawnSync(process.execPath, [cli, "test", "--config", config, ...args], {
            cwd: root, env, encoding: "utf8", timeout: 60_000
        });
        assert.equal(result.status, 0, result.error?.message || result.stdout + result.stderr);
        return JSON.parse(fs.readFileSync(path.join(directory, "report.json"), "utf8"));
    }
    const selected = run(["--grep", "passing scene", "--workers", "1"]);
    assert.equal(selected.stats.expected, 1);
    assert.equal(selected.stats.unexpected, 0);
    let lifecycle = fs.readFileSync(events, "utf8").trim().split("\n").map(line => JSON.parse(line));
    assert.deepEqual(lifecycle.map(item => item.phase), ["start", "stop"]);
    fs.writeFileSync(events, "");
    const all = run(["--workers", "2", "--retries", "1"]);
    assert.equal(all.stats.expected, 1);
    assert.equal(all.stats.flaky, 1);
    assert.equal(all.stats.unexpected, 0);
    const specs = all.suites.flatMap(suite => suite.specs);
    assert.equal(specs.length, 2);
    const retry = specs.find(spec => spec.title === "Native transport retry scene").tests[0];
    assert.deepEqual(retry.results.map(result => result.status), ["failed", "passed"]);
    assert.notEqual(retry.results[0].workerIndex, retry.results[1].workerIndex);
    assert.ok(retry.results[0].errors.some(error => /Screenshot comparison failed|pixels.*different/.test(error.message)));
    lifecycle = fs.readFileSync(events, "utf8").trim().split("\n").map(line => JSON.parse(line));
    const started = lifecycle.filter(item => item.phase === "start").map(item => item.id).sort();
    const stopped = lifecycle.filter(item => item.phase === "stop").map(item => item.id).sort();
    assert.equal(started.length, 3);
    assert.deepEqual(stopped, started);
});

test("the mobile entry point refuses replacement projects, configs and reference updates", () => {
    for (const args of [["--update-snapshots"], ["-u"], ["--project=webgl2"], ["--config", "other.cjs"], ["-c", "other.cjs"]]) {
        const result = spawnSync(process.execPath, [path.resolve(__dirname, "..", "run.cjs"), ...args], { encoding: "utf8" });
        assert.equal(result.status, 1);
        assert.match(result.stderr, /unchanged snapshots are fixed/);
    }
});

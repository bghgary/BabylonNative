const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");
const assert = require("node:assert/strict");
const { PNG } = require("pngjs");

async function runFixture(t, mode) {
    const directory = fs.mkdtempSync(path.join(__dirname, ".visual-fixture-"));
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
    const root = path.dirname(__dirname);
    const catalog = require(path.join(root, "..", "..", "Scripts", "config.json"));
    const version = require(path.join(root, "..", "..", "..", "package-lock.json"))
        .packages["node_modules/babylonjs"].version;
    const selected = mode === "recovery" ? [131, 149] : [131];
    const png = PNG.sync.write({ width: 600, height: 400, data: Buffer.alloc(600 * 400 * 4) });
    const image = { width: 600, height: 400, png: png.toString("base64"), pngBytes: png.length };
    if (mode === "oversized-frame" || mode === "misreported-frame") {
        png.writeUInt32BE(1024 * 1024 + 1, 16);
        image.png = png.toString("base64");
        if (mode === "oversized-frame") {
            image.width = 1024 * 1024 + 1;
        }
    }
    let execute;
    let stopped = 0;
    let closed = 0;
    class Controller {
        constructor(receive) { this.receive = receive; }
        async listen() { return this; }
        async close() { closed++; }
        async evaluate(source, indices) {
            if (typeof source === "string") {
                return;
            }
            const index = indices[0];
            const title = catalog.tests[index].title;
            await this.receive({ type: "test-start", index, title });
            if (mode === "timeout" || (mode === "recovery" && index === 131)) {
                throw Object.assign(new Error("fixture timeout"), { name: "MobileCommandTimeout", sceneIndex: index });
            }
            const frame = { type: "frame", index, filename: "mobile-results/Results/fixture.png", image };
            if (mode !== "missing-frame") {
                await this.receive(frame);
            }
            if (mode === "scene-failure") {
                await this.receive({ ...frame, filename: "mobile-results/Errors/fixture.png" });
            }
            if (mode === "duplicate-frame") {
                await this.receive(frame);
            }
            if (mode === "early-exit") {
                return { exitCode: 0 };
            }
            await this.receive({ type: "test-result", index, title, passed: mode !== "scene-failure" });
            return { exitCode: mode === "wrong-exit" || mode === "scene-failure" ? 1 : 0 };
        }
    }
    const expect = value => ({
        toBe: expected => assert.equal(value, expected),
        toEqual: expected => assert.equal(JSON.stringify(value), JSON.stringify(expected))
    });
    const cloud = {
        credentials: () => ({ redact: value => String(value) }),
        startSession: async () => ({
            measurements: { ready: { babylonVersion: version, graphicsApi: "OpenGL" } },
            stop: async () => {
                stopped++;
                if (mode === "cleanup-failure") {
                    throw new Error("fixture cloud cleanup failed");
                }
            },
            diagnostics: async () => {}
        })
    };
    vm.runInNewContext(fs.readFileSync(path.join(root, "visual.spec.cjs"), "utf8"), {
        require: Object.assign(name => {
            if (name === "@playwright/test") {
                return { test: (_, fn) => { execute = fn; }, expect };
            }
            if (name === "./controller.cjs") {
                return { MobileController: Controller };
            }
            if (name === "./browserstack.cjs") {
                return cloud;
            }
            return require(name);
        }, { resolve: require.resolve }),
        __dirname: root, performance, Buffer, console: { log() {} },
        process: { env: { MOBILE_TEST_INDICES: JSON.stringify(selected) } }
    }, { filename: "visual-fixture.cjs" });
    const testInfo = {
        outputPath: name => path.join(directory, name),
        attach: async () => {}
    };
    let error;
    try {
        await execute({}, testInfo);
    } catch (caught) {
        error = caught;
    }
    const result = JSON.parse(fs.readFileSync(path.join(directory, "validation", "results.json"), "utf8"));
    assert.equal(stopped, mode === "recovery" ? 2 : 1);
    assert.equal(closed, mode === "recovery" ? 2 : 1);
    return { result, error, files: fs.readdirSync(path.join(directory, "validation")) };
}

test("visual host requires a rendered frame and matching explicit completion for success", async t => {
    const { result, error } = await runFixture(t, "pass");
    assert.equal(error, undefined);
    assert.equal(result.complete, true);
    assert.equal(result.frames, 1);
    assert.equal(result.cases[0].status, "passed");
});

test("replacement-session success completes remaining scenes without erasing the earlier timeout", async t => {
    const { result, error } = await runFixture(t, "recovery");
    assert.ok(error);
    assert.equal(result.runError, undefined);
    assert.equal(result.complete, true);
    assert.deepEqual(result.cases.map(item => [item.index, item.status]), [[131, "timeout"], [149, "passed"]]);
});

test("cleanup failure is explicit in the persisted result even after every scene passes", async t => {
    const { result, error } = await runFixture(t, "cleanup-failure");
    assert.ok(error);
    assert.equal(result.complete, true);
    assert.equal(result.cases[0].status, "passed");
    assert.match(result.cleanupError, /fixture cloud cleanup failed/);
});

for (const mode of ["oversized-frame", "misreported-frame"]) {
    test(`visual host rejects ${mode} dimensions before PNG decompression`, async t => {
        const { result, error } = await runFixture(t, mode);
        assert.ok(error);
        assert.match(result.runError, /Mobile PNG dimensions exceed the limit or differ from metadata/);
        assert.equal(result.frames, 0);
    });
}

for (const mode of ["missing-frame", "duplicate-frame", "early-exit", "wrong-exit", "scene-failure", "timeout"]) {
    test(`visual host preserves ${mode} as failure and releases its session`, async t => {
        const { result, error, files } = await runFixture(t, mode);
        assert.ok(error);
        if (mode === "timeout") {
            assert.equal(result.cases[0].status, "timeout");
        } else if (mode === "scene-failure") {
            assert.equal(result.cases[0].status, "failed");
            assert.ok(files.includes("131-comparison.png"));
            assert.ok(files.includes("131-rendered.png"));
        } else {
            assert.ok(result.runError);
        }
    });
}

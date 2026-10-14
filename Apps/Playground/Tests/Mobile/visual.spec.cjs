const fs = require("node:fs");
const path = require("node:path");
const { createHash } = require("node:crypto");
const { test, expect } = require("@playwright/test");
const { PNG } = require("pngjs");
const { MobileController } = require("./controller.cjs");
const { credentials, startSession } = require("./browserstack.cjs");

const playground = path.resolve(__dirname, "..", "..");
const configPath = path.join(playground, "Scripts", "config.json");
const catalog = JSON.parse(fs.readFileSync(configPath, "utf8").replace(/^\uFEFF/, ""));
const version = require(path.join(playground, "..", "package-lock.json")).packages["node_modules/babylonjs"].version;

test("native mobile validation catalog", async ({}, testInfo) => {
    const started = performance.now();
    const auth = credentials();
    const platform = process.env.MOBILE_PLATFORM;
    const deviceName = process.env.MOBILE_DEVICE;
    const osVersion = process.env.MOBILE_OS_VERSION;
    const app = process.env.MOBILE_APP;
    const output = testInfo.outputPath("validation");
    fs.mkdirSync(output, { recursive: true });
    const result = {
        platform, deviceName, osVersion, babylonVersion: version,
        catalogCount: catalog.tests.length,
        catalogSha256: createHash("sha256").update(fs.readFileSync(configPath)).digest("hex"),
        exclusions: [], selected: [], cases: [], sessions: [], complete: false,
        frames: 0, pngBytes: 0
    };
    function persist() {
        fs.writeFileSync(path.join(output, "results.json"), auth.redact(JSON.stringify(result, null, 2)));
    }
    const requested = process.env.MOBILE_TEST_INDICES
        ? JSON.parse(process.env.MOBILE_TEST_INDICES) : undefined;
    if (requested && (!Array.isArray(requested) || !requested.length ||
            new Set(requested).size !== requested.length ||
            requested.some(n => !Number.isInteger(n) || n < 0 || n >= catalog.tests.length))) {
        throw new Error("MOBILE_TEST_INDICES must be a nonempty array of distinct catalog indices");
    }
    const [shard, shardCount] = (process.env.MOBILE_SHARD || "1/1").split("/").map(Number);
    if (!Number.isInteger(shard) || !Number.isInteger(shardCount) || shard < 1 || shard > shardCount || shardCount > 5) {
        throw new Error("MOBILE_SHARD must be n/total with 1 <= n <= total <= 5");
    }
    let controller;
    let mobile;
    let current;
    let currentStarted;
    let currentFrames;
    async function close() {
        const errors = [];
        try {
            await mobile?.stop();
            if (mobile) {
                try {
                    await mobile.diagnostics();
                } catch (error) {
                    mobile.measurements.diagnosticsError = auth.redact(error.message);
                }
            }
        } catch (error) {
            errors.push(error);
        }
        mobile = undefined;
        try {
            await controller?.close();
        } catch (error) {
            errors.push(error);
        }
        controller = undefined;
        if (errors.length) {
            result.cleanupError = auth.redact(errors.map(error => error.message).join("; "));
            throw new AggregateError(errors, "Mobile session cleanup failed");
        }
    }
    async function receive(message) {
        if (!result.selected.includes(message.index) || result.cases.some(item => item.index === message.index)) {
            throw new Error("Device reported an unselected or already completed scene");
        }
        if (message.type === "test-start") {
            if (current !== undefined || message.title !== catalog.tests[message.index].title) {
                throw new Error("Invalid mobile scene start");
            }
            current = message.index;
            currentStarted = performance.now();
            currentFrames = new Set();
        } else if (message.type === "frame") {
            const image = message.image;
            if (current !== message.index || !image || typeof image.png !== "string" ||
                    !Number.isSafeInteger(image.pngBytes) || image.pngBytes < 1) {
                throw new Error("Invalid mobile framebuffer event");
            }
            const bytes = Buffer.from(image.png, "base64");
            if (!Number.isSafeInteger(image.width) || !Number.isSafeInteger(image.height) ||
                    image.width < 1 || image.height < 1 || image.width * image.height > 1024 * 1024 ||
                    bytes.length < 33 || bytes.readUInt32BE(16) !== image.width ||
                    bytes.readUInt32BE(20) !== image.height) {
                throw new Error("Mobile PNG dimensions exceed the limit or differ from metadata");
            }
            const decoded = PNG.sync.read(bytes, { checkCRC: true });
            if (bytes.length !== image.pngBytes || decoded.width !== image.width || decoded.height !== image.height) {
                throw new Error("Mobile PNG dimensions or payload length differ from device metadata");
            }
            const bucket = message.filename?.startsWith("mobile-results/Errors/") ? "comparison" :
                message.filename?.startsWith("mobile-results/Results/") ? "rendered" : undefined;
            if (!bucket || currentFrames.has(bucket)) {
                throw new Error("Unrecognized or duplicate mobile output filename");
            }
            currentFrames.add(bucket);
            const filename = `${message.index}-${bucket}.png`;
            fs.writeFileSync(path.join(output, filename), bytes);
            result.frames++;
            result.pngBytes += bytes.length;
        } else if (message.type === "test-result") {
            if (current !== message.index || typeof message.passed !== "boolean" ||
                    message.title !== catalog.tests[message.index].title ||
                    (message.passed && !currentFrames.has("rendered"))) {
                throw new Error("Invalid mobile scene result");
            }
            result.cases.push({
                index: message.index, title: message.title, status: message.passed ? "passed" : "failed",
                elapsedMs: performance.now() - currentStarted, console: message.console
            });
            current = undefined;
            persist();
            console.log(`${result.cases.length}/${result.selected.length}: ${message.index} ${message.passed ? "passed" : "FAILED"} ${message.title}`);
        }
    }
    try {
        while (!result.complete) {
            controller = await new MobileController(receive).listen();
            mobile = await startSession({ controller, platform, deviceName, osVersion, app, output, auth });
            result.sessions.push(mobile.measurements);
            expect(mobile.measurements.ready.babylonVersion).toBe(version);
            if (result.selected.length === 0) {
                const api = mobile.measurements.ready.graphicsApi;
                for (const [index, scene] of catalog.tests.entries()) {
                    const reason = scene.excludeFromAutomaticTesting ? "excludeFromAutomaticTesting" :
                        scene.onlyVisual ? "onlyVisual" :
                        scene.excludedGraphicsApis?.includes(api) ? `excludedGraphicsApis:${api}` : undefined;
                    if (reason) {
                        result.exclusions.push({ index, title: scene.title, reason });
                    } else if ((!requested || requested.includes(index)) && index % shardCount === shard - 1) {
                        if (!scene.referenceImage || !fs.existsSync(path.join(playground, "ReferenceImages", scene.referenceImage))) {
                            throw new Error(`Missing committed native reference for scene ${index}`);
                        }
                        result.selected.push(index);
                    }
                }
                if (!result.selected.length || requested?.some(index => !result.selected.includes(index) &&
                        index % shardCount === shard - 1)) {
                    throw new Error("Requested mobile selection is empty or contains excluded scenes");
                }
            }
            persist();
            const fflateRoot = path.dirname(require.resolve("fflate/package.json"));
            await controller.evaluate(fs.readFileSync(path.join(fflateRoot, "umd", "index.js"), "utf8"));
            const completed = new Set(result.cases.map(item => item.index));
            const remaining = result.selected.filter(index => !completed.has(index));
            try {
                const completion = await controller.evaluate(
                    indices => globalThis.__mobileValidation.run({ testIndices: indices }), remaining,
                    { timeoutMs: 80 * 60_000, activityTimeoutMs: 120_000 }
                );
                if (!completion || !Number.isInteger(completion.exitCode)) {
                    throw new Error("Native validation did not return an explicit exit code");
                }
                if (result.cases.length !== result.selected.length) {
                    throw new Error(`Native validation exited before all selected scenes completed: ${completion.exitCode}`);
                }
                const failed = result.cases.some(item => remaining.includes(item.index) && item.status !== "passed");
                if ((completion.exitCode === 0) === failed) {
                    throw new Error("Native exit code disagrees with individual scene outcomes");
                }
                result.complete = true;
            } catch (error) {
                if (error.name !== "MobileCommandTimeout" || current === undefined || error.sceneIndex !== current) {
                    throw error;
                }
                result.cases.push({
                    index: current, title: catalog.tests[current].title, status: "timeout",
                    elapsedMs: performance.now() - currentStarted, error: error.message
                });
                current = undefined;
                // Timed-out code is never replayed in the contaminated app session.
                await close();
                result.complete = result.cases.length === result.selected.length;
                persist();
            }
        }
    } catch (error) {
        if (error.mobileMeasurements) {
            result.sessions.push(error.mobileMeasurements);
        }
        result.runError = auth.redact(error.message);
        throw error;
    } finally {
        try {
            await close();
        } finally {
            result.elapsedMs = performance.now() - started;
            persist();
            await testInfo.attach("native-mobile-results", {
                path: path.join(output, "results.json"), contentType: "application/json"
            });
        }
    }
    expect(result.complete).toBe(true);
    expect(result.cases.filter(item => item.status !== "passed"), "Native scene failures are recorded in results.json").toEqual([]);
});

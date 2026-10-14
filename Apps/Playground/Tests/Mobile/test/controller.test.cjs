const { test } = require("node:test");
const assert = require("node:assert/strict");
const { MobileController } = require("../controller.cjs");

async function fixture(t, onEvent) {
    const controller = await new MobileController(onEvent).listen();
    t.after(() => controller.close());
    const endpoint = `http://127.0.0.1:${controller.port}${controller.devicePath}`;
    const post = value => fetch(endpoint + "/message", {
        method: "POST", body: JSON.stringify(value)
    });
    await post({ type: "ready", protocol: 1, babylonVersion: "9.29.0", graphicsApi: "OpenGL" });
    await controller.ready(100);
    return { controller, endpoint, post };
}

test("controller correlates async commands and preserves device errors", async t => {
    const { controller, endpoint, post } = await fixture(t);
    const pending = controller.evaluate(async value => value + 1, 41);
    const command = await (await fetch(endpoint + "/next")).json();
    assert.equal(await eval(command.source), 42);
    await post({ type: "result", id: command.id, value: 42 });
    assert.equal(await pending, 42);
    const errorResult = controller.evaluate(() => { throw new Error("device failure"); });
    const check = assert.rejects(errorResult, error => {
        assert.equal(error.message, "Error: device failure");
        assert.equal(error.deviceStack, "device-stack");
        return true;
    });
    const next = await (await fetch(endpoint + "/next")).json();
    await post({ type: "result", id: next.id, error: "Error: device failure", stack: "device-stack" });
    await check;
});

test("wrong path cannot issue a command or poison the controller", async t => {
    const { controller, endpoint } = await fixture(t);
    assert.equal((await fetch(endpoint.replace(controller.token, "not-the-token") + "/next")).status, 404);
    assert.equal(controller.failure, undefined);
});

test("timeouts poison commands rather than replaying ambiguous work", async t => {
    const { controller } = await fixture(t);
    await assert.rejects(controller.evaluate(() => 1, undefined, { timeoutMs: 25 }), { name: "MobileCommandTimeout" });
    await assert.rejects(controller.evaluate(() => 2), { name: "MobileCommandTimeout" });
});

test("validation events apply backpressure and carry the current scene", async t => {
    const events = [];
    const { controller, endpoint, post } = await fixture(t, async message => events.push(message));
    const pending = controller.evaluate(() => 1);
    const command = await (await fetch(endpoint + "/next")).json();
    assert.equal((await post({ type: "test-start", commandId: command.id, index: 7, title: "scene" })).status, 200);
    assert.equal((await post({ type: "test-result", commandId: command.id, index: 7, passed: false })).status, 200);
    await post({ type: "result", id: command.id, value: { exitCode: -1 } });
    assert.deepEqual(await pending, { exitCode: -1 });
    assert.equal(events.length, 2);
});

test("out-of-order replies fail the active command explicitly", async t => {
    const { controller, post } = await fixture(t);
    const pending = controller.evaluate(() => 1);
    const check = assert.rejects(pending, /does not match/);
    assert.equal((await post({ type: "result", id: 999, value: 1 })).status, 400);
    await check;
});

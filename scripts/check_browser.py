#!/usr/bin/env python3
"""Serve the actual app and assert interactions in a real pinned Chromium engine.

Prepare tools/browser with npm ci and install its chromium-headless-shell first.
No browser, simulated DOM or missing screenshot is treated as a successful run.
"""
from __future__ import annotations

import functools
import http.server
import json
import os
from pathlib import Path
import subprocess
import threading
import time

from platform_support import ROOT, Run, digest, redact, source_hashes, tool_environment, write_json


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, _format, *_args):
        pass


def result_object(value):
    """CLI JSON wraps the returned run-code value; require our exact discriminator."""
    if isinstance(value, dict):
        if value.get("schema") == "suzume-browser-assertions/1":
            return value
        for child in value.values():
            result = result_object(child)
            if result is not None:
                return result
    elif isinstance(value, list):
        for child in value:
            result = result_object(child)
            if result is not None:
                return result
    elif isinstance(value, str):
        try:
            return result_object(json.loads(value))
        except ValueError:
            return None
    return None


def main() -> int:
    run = Run("browser-check")
    before = source_hashes()
    environment = tool_environment()
    environment.update({"PLAYWRIGHT_MCP_SANDBOX": "0", "NO_UPDATE_NOTIFIER": "1"})
    browser_root = Path(environment.get("PLAYWRIGHT_BROWSERS_PATH", ROOT / "dist/tools/browser-engines")).resolve()
    environment["PLAYWRIGHT_BROWSERS_PATH"] = str(browser_root)
    cli = ROOT / "tools/browser/node_modules/@playwright/cli/playwright-cli.js"
    node = __import__("shutil").which("node", path=environment["PATH"])
    session = "suzume-" + run.id
    server = None
    opened = False

    def command(name: str, arguments: list[str], *, plain: bool = False):
        start = time.monotonic()
        argv = [node or "node", str(cli), f"-s={session}", *([] if plain else ["--json"]), *arguments]
        try:
            completed = subprocess.run(argv, cwd=run.directory, env=environment, stdin=subprocess.DEVNULL,
                                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=55, check=False)
            code, output = completed.returncode, completed.stdout
        except (OSError, subprocess.TimeoutExpired) as error:
            code, output = 124 if isinstance(error, subprocess.TimeoutExpired) else 127, str(error)
        log = run.directory / f"{name}.log"
        log.write_text(redact(output), encoding="utf-8")
        payload = None
        if code == 0 and not plain:
            try:
                payload = json.loads(output)
            except ValueError:
                code = 1
            if isinstance(payload, dict) and payload.get("isError"):
                code = 1
        record = {"id": name, "command": [redact(part) for part in argv], "exit_code": code,
                  "success": code == 0, "timed_out": code == 124, "seconds": round(time.monotonic() - start, 3),
                  "log": log.name, "log_sha256": digest(log)}
        write_json(run.directory / f"{name}.json", record)
        run.results.append(record)
        print(f"{name}: {'success' if code == 0 else 'failure'} ({record['seconds']}s)", flush=True)
        if code != 0:
            raise RuntimeError(f"{name} failed; see {log.relative_to(ROOT)}")
        return output.strip() if plain else payload

    try:
        manifest = json.loads((ROOT / "tools/browser/package.json").read_text())
        version = command("cli-version", ["--version"], plain=True)
        if version != manifest["dependencies"]["@playwright/cli"]:
            raise RuntimeError("Playwright CLI version does not match its pinned manifest")
        browsers = json.loads((ROOT / "tools/browser/node_modules/playwright-core/browsers.json").read_text())["browsers"]
        expected = next(browser for browser in browsers if browser["name"] == "chromium-headless-shell")
        directory = browser_root / f"chromium_headless_shell-{expected['revision']}"
        engines = [path for path in directory.rglob("*") if path.name in {"chrome-headless-shell", "chrome-headless-shell.exe", "headless_shell"} and path.is_file()]
        if len(engines) != 1:
            raise RuntimeError("Pinned Chromium headless shell is missing; install the browser using tools/browser's locked Playwright")
        config = {"browser": {"browserName": "chromium", "isolated": True,
                  "launchOptions": {"executablePath": str(engines[0]), "headless": True},
                  "contextOptions": {"viewport": {"width": 1280, "height": 1000}}}}
        write_json(run.directory / "cli-config.json", config)
        handler = functools.partial(QuietHandler, directory=str(ROOT))
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        url = f"http://127.0.0.1:{server.server_port}/examples/browser/"
        command("open", ["open", url, "--config=" + str(run.directory / "cli-config.json")])
        opened = True
        initial_image, final_image = run.directory / "initial.png", run.directory / "after-actions.png"
        code = r'''async (page) => {
  const assertions = [];
  const errors = [];
  const check = (condition, name) => { if (!condition) throw new Error(name); assertions.push(name); };
  page.on('pageerror', error => errors.push(String(error)));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  await page.reload();
  await page.waitForFunction(() => window.__appReady === true, null, { timeout: 20000 });
  const initial = await page.evaluate(() => window.__suzumeView());
  check(initial.revision === 0 && initial.own_hand.length === 14, 'initial-deal');
  check(await page.locator('#hand button').count() === 14, 'rendered-fourteen-tiles');
  check(initial.players.every(player => !('hand' in player)) && !('seed' in initial), 'private-state-not-exposed');
  await page.screenshot({path: INITIAL_IMAGE, fullPage: true});
  await page.locator('#hand button:not([disabled])').first().click();
  await page.waitForFunction(() => window.__suzumeView().revision === 1);
  const discarded = await page.evaluate(() => window.__suzumeView());
  check(discarded.players[0].discards.length === 1, 'discard-from-visible-control');
  await page.getByRole('button', { name: 'CPUを1手進める', exact: true }).click();
  await page.waitForFunction(() => window.__suzumeView().revision === 2);
  check((await page.evaluate(() => window.__suzumeView())).revision === 2, 'one-cpu-operation');
  await page.getByRole('button', { name: 'あなたの手番まで進める', exact: true }).click();
  await page.waitForFunction(() => window.__suzumeView().actor === 0);
  const advanced = await page.evaluate(() => window.__suzumeView());
  check(advanced.revision > 2 && advanced.legal_actions.length > 0, 'advance-to-human');
  await page.getByLabel('対局の乱数種', { exact: true }).fill('invalid');
  await page.getByRole('button', { name: '新しい対局', exact: true }).click();
  await page.getByRole('alert').waitFor({state:'visible'});
  const unchanged = await page.evaluate(() => window.__suzumeView());
  check(unchanged.session_id === advanced.session_id && unchanged.revision === advanced.revision, 'invalid-seed-is-atomic');
  await page.getByLabel('対局の乱数種', { exact: true }).fill('18446744073709551615');
  await page.getByLabel('CPUの設定', { exact: true }).selectOption('strong');
  await page.getByRole('button', { name: '新しい対局', exact: true }).click();
  await page.waitForFunction(() => window.__suzumeView().session_id === 2);
  const restarted = await page.evaluate(() => window.__suzumeView());
  check(restarted.revision === 0 && restarted.own_hand.length === 14, 'restart-with-full-u64-seed');
  check(JSON.stringify(initial.own_hand) !== JSON.stringify(restarted.own_hand), 'different-seed-changes-deal');
  check(!(await page.getByRole('alert').isVisible()), 'error-cleared-after-recovery');
  check(errors.length === 0, 'no-console-or-page-errors');
  await page.screenshot({path: FINAL_IMAGE, fullPage: true});
  return {schema:'suzume-browser-assertions/1', browser_version:page.context().browser().version(), assertions, errors,
          initial:{session_id:initial.session_id,revision:initial.revision},
          after_actions:{session_id:advanced.session_id,revision:advanced.revision},
          restarted:{session_id:restarted.session_id,revision:restarted.revision}};
}'''.replace("INITIAL_IMAGE", json.dumps(str(initial_image))).replace("FINAL_IMAGE", json.dumps(str(final_image)))
        script = run.directory / "interaction.js"
        script.write_text(code, encoding="utf-8")
        payload = command("interactions", ["run-code", "--filename=" + str(script)])
        result = result_object(payload)
        if result is None or len(result["assertions"]) != 11 or result["errors"]:
            raise RuntimeError("Missing expected real-browser assertion results")
        if result["browser_version"] != expected["browserVersion"]:
            raise RuntimeError("Chromium version differs from the locked browser revision")
        screenshots = [{"file": path.name, "bytes": path.stat().st_size, "sha256": digest(path)} for path in (initial_image, final_image)]
        if len({item["sha256"] for item in screenshots}) != 2 or any(item["bytes"] == 0 for item in screenshots):
            raise RuntimeError("Two nonempty, different screenshots are required")
        write_json(run.directory / "assertions.json", result)
        command("close", ["close"])
        opened = False
        after = source_hashes()
        success = run.finish({"assertions": result, "screenshots": screenshots,
                              "cli_version": version, "browser_revision": expected["revision"],
                              "source_sha256_before": before, "source_sha256_after": after,
                              "sources_unchanged": before == after, "success": before == after})
        print(json.dumps({"browser_version": result["browser_version"], "assertions": result["assertions"], "screenshots": screenshots}, ensure_ascii=False))
        return 0 if success else 1
    except (OSError, ValueError, KeyError, RuntimeError, StopIteration) as error:
        if opened:
            try:
                command("close-after-error", ["close"])
            except RuntimeError:
                pass
        run.finish({"success": False, "error": redact(str(error)), "source_sha256_before": before})
        print(redact(str(error)))
        return 1
    finally:
        if server is not None:
            server.shutdown()
            server.server_close()


if __name__ == "__main__":
    raise SystemExit(main())

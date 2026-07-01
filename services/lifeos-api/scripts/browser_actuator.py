#!/usr/bin/env python3
"""Thin CLI over the vendored external/browser-use submodule (issue #54,
docs/SECURITY.md §4). Invoked by lifeos-api as a subprocess
(src/browser.rs::ProcessBrowserActuator), never imported directly - keeps
the Rust process free of a Python runtime dependency.

`scrape` runs with every state-changing action excluded from the agent's
tool set - it structurally cannot click, type, submit, or upload even if an
adversarial task string asked it to. `capture-session` is the one
interactive exception: it opens a real, headed browser for a human to log
in themselves, then dumps the resulting session for the caller to encrypt.

Action names below are read off external/browser-use/browser_use/tools/service.py
at the pinned submodule commit - re-verify against `Tools().registry.registry.actions.keys()`
after any submodule bump, since browser-use does not guarantee action-name stability.
"""

import asyncio
import json
import sys

STATE_CHANGING_ACTIONS = [
    "click",
    "input",
    "upload_file",
    "send_keys",
    "select_dropdown",
    "write_file",
    "replace_file",
    "evaluate",
]


def _llm():
    from browser_use.beta import ChatAnthropic

    return ChatAnthropic(model="claude-sonnet-5")


async def scrape(url: str, task: str) -> None:
    from urllib.parse import urlparse

    from browser_use.beta import Agent, BrowserProfile, Tools

    domain = urlparse(url).netloc
    agent = Agent(
        task=f"Navigate to {url}. {task}",
        llm=_llm(),
        browser_profile=BrowserProfile(headless=True, allowed_domains=[f"*.{domain}", domain]),
        tools=Tools(exclude_actions=STATE_CHANGING_ACTIONS),
    )
    history = await agent.run()
    print(json.dumps({"result": history.final_result()}))


async def capture_session(site: str) -> None:
    from browser_use.beta import Agent, BrowserProfile

    agent = Agent(
        task=(
            f"Navigate to https://{site}. Wait for the human operator to log in "
            "manually in this browser window, then call `done` once they confirm "
            "they are logged in. Never fill in credentials yourself."
        ),
        llm=_llm(),
        browser_profile=BrowserProfile(headless=False),
    )
    await agent.run()
    storage_state = await agent.browser_session.get_storage_state()
    print(json.dumps(storage_state))


def main() -> int:
    if len(sys.argv) < 2:
        print("usage: browser_actuator.py <scrape|capture-session> ...", file=sys.stderr)
        return 2

    command = sys.argv[1]
    if command == "scrape":
        if len(sys.argv) != 4:
            print("usage: browser_actuator.py scrape <url> <task>", file=sys.stderr)
            return 2
        asyncio.run(scrape(sys.argv[2], sys.argv[3]))
    elif command == "capture-session":
        if len(sys.argv) != 3:
            print("usage: browser_actuator.py capture-session <site>", file=sys.stderr)
            return 2
        asyncio.run(capture_session(sys.argv[2]))
    else:
        print(f"unknown command '{command}'", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())

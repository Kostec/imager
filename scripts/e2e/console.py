#!/usr/bin/env python3
"""Watches the serial console of the guest and logs in at the very first prompt.

Logging in at once is the point: whatever first boot still had to do would show
up here as a wizard, a refused password or a wrong host name.

Usage: E2E_PASSWORD=... console.py <socket> <log> <user> <hostname> <timeout seconds>
"""

import os
import re
import socket
import sys
import time

WIZARD_MARKS = (b"Create root password", b"Creating a new user", b"Please provide a username")
ANSWER_TIMEOUT_SECS = 180
# The profile sets a Russian locale, and PAM follows it.
PASSWORD_PROMPT = rb"(Password|\xd0\x9f\xd0\xb0\xd1\x80\xd0\xbe\xd0\xbb\xd1\x8c): ?$"
SHELL_PROMPT = rb"\$ ?(\x1b\[[0-9;]*m)*$"


class Console:
    def __init__(self, path, log_path):
        self.started = time.monotonic()
        self.seen = b""
        self.log = open(log_path, "wb")
        self.sock = self._connect(path)

    def _connect(self, path):
        for _ in range(100):
            try:
                sock = socket.socket(socket.AF_UNIX)
                sock.connect(path)
                sock.settimeout(1)
                return sock
            except OSError:
                time.sleep(0.2)
        raise SystemExit("no serial socket")

    def _read(self):
        try:
            data = self.sock.recv(4096)
        except TimeoutError:
            return
        except socket.timeout:
            return
        if not data:
            raise SystemExit("the serial console closed")
        self.log.write(data)
        self.log.flush()
        self.seen += data

    def expect(self, pattern, timeout):
        """Waits for the pattern in what came after the previous match; None on timeout."""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self._read()
            match = re.search(pattern, self.seen)
            if match:
                before = self.seen[: match.start()]
                self.seen = self.seen[match.end() :]
                return before, match
        return None

    def send(self, text):
        self.sock.sendall(text.encode() + b"\r")

    def elapsed(self):
        return int(time.monotonic() - self.started)


def fail(message):
    print(f"FAIL console: {message}", flush=True)
    return 1


def main():
    path, log_path, user, hostname, timeout = sys.argv[1:6]
    console = Console(path, log_path)

    first = console.expect(rb"([a-zA-Z0-9._-]+) login: ?$", int(timeout))
    if first is None:
        print(f"UNVERIFIED console: no login prompt in {timeout}s")
        return 3
    before, match = first
    print(f"INFO console: first login prompt after {console.elapsed()}s")
    if b"autologin" in before.lower() or b"Automatic login" in before:
        return fail("root is logged in automatically")
    if match.group(1).decode() != hostname:
        return fail(f"the prompt shows {match.group(1).decode()!r}, not the new host name")
    print("PASS console: the first prompt already shows the new host name")

    console.send(user)
    if console.expect(PASSWORD_PROMPT, ANSWER_TIMEOUT_SECS) is None:
        return fail("no password prompt")
    console.send(os.environ["E2E_PASSWORD"])
    shell = console.expect(SHELL_PROMPT, ANSWER_TIMEOUT_SECS)
    if shell is None:
        return fail("no shell after the password")
    if any(mark in shell[0] for mark in WIZARD_MARKS):
        return fail("the first login started the wizard")
    # The marker is split in the command so that its echo cannot match.
    console.send("echo CONSOLE-OK-$(id -un)-$(hostname)-'E'ND")
    reply = console.expect(rb"CONSOLE-OK-([^-\s]+)-([^\s]+)-END", ANSWER_TIMEOUT_SECS)
    if reply is None:
        return fail("the shell does not answer")
    _, match = reply
    if match.group(1).decode() != user:
        return fail(f"logged in as {match.group(1).decode()!r}")
    print("PASS console: first login reaches a shell as the new user, no wizard")
    console.send("exit")
    return 0


if __name__ == "__main__":
    sys.exit(main())

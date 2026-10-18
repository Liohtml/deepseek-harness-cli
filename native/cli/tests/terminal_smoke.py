"""POSIX PTY acceptance for the built Rust frontend, without API calls."""
import errno
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import tempfile
import termios
import time
import unittest

ROOT = Path(__file__).resolve().parents[3]
BINARY = Path(os.environ.get("DSH_TEST_BINARY", ROOT / "native/cli/target/debug/dsh"))


class TerminalTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="dsh native project ")
        self.record = Path(self.directory.name) / "launch.json"
        self.buffer = b""
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            env = os.environ.copy()
            env.pop("NODE_OPTIONS", None)
            env["DSH_RUNTIME_JS"] = str(ROOT / "native/cli/tests/fake-runtime.mjs")
            env["DSH_TEST_RECORD"] = str(self.record)
            os.execve(BINARY, [str(BINARY), "--cwd", self.directory.name], env)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 100, 0, 0))
        self.until("dsh> ")

    def tearDown(self):
        if self.pid:
            try:
                os.kill(self.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.waitpid(self.pid, 0)
        os.close(self.fd)
        self.directory.cleanup()

    def until(self, text, timeout=10):
        wanted = text.encode()
        deadline = time.monotonic() + timeout
        while wanted not in self.buffer:
            if time.monotonic() > deadline:
                self.fail(f"No {text!r} in terminal: {self.buffer!r}")
            if select.select([self.fd], [], [], .05)[0]:
                try:
                    chunk = os.read(self.fd, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        self.fail(f"Terminal exited before {text!r}: {self.buffer!r}")
                    raise
                self.buffer += chunk
        before, self.buffer = self.buffer.split(wanted, 1)
        return before

    def send(self, text):
        os.write(self.fd, text.encode())

    def finish(self):
        self.send("/exit\r")
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            pid, status = os.waitpid(self.pid, os.WNOHANG)
            if pid:
                self.pid = 0
                self.assertEqual(os.waitstatus_to_exitcode(status), 0)
                self.assertTrue(termios.tcgetattr(self.fd)[3] & termios.ICANON)
                self.assertTrue(termios.tcgetattr(self.fd)[3] & termios.ECHO)
                return
            time.sleep(.03)
        self.fail("Native CLI did not exit")

    def test_profile_launch_unicode_followup_history_and_exit(self):
        launch = json.loads(self.record.read_text())
        self.assertEqual(launch["args"], ["headless", "--terminal-bridge"])
        self.assertEqual(launch["cwd"], self.directory.name)
        self.send("Grüße 世界\r")
        self.until("Answer: Grüße 世界")
        self.until("dsh> ")
        self.send("\x1b[A\r")
        self.until("Answer: Grüße 世界")
        self.until("dsh> ")
        self.finish()

    def test_typeahead_cannot_grant_permission(self):
        self.send("permission\r")
        self.until("Working before permission")
        self.send("yes\r")
        self.until("[y/N] ")
        self.send("no\r")
        self.until("Approval answer: no")
        self.until("dsh> ")
        self.finish()

    def test_ctrl_c_cancels_work_and_allows_followup(self):
        self.send("wait\r")
        self.until("Working until interrupt")
        self.send("\x03")
        self.until("Turn cancelled")
        self.until("dsh> ")
        self.finish()


if __name__ == "__main__":
    unittest.main()

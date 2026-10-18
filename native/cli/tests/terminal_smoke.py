"""POSIX PTY acceptance for the built Rust frontend, without API calls."""
import codecs
import pyte
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
        self.screen = pyte.Screen(100, 24)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            env = os.environ.copy()
            env.pop("NODE_OPTIONS", None)
            env["DSH_RUNTIME_JS"] = str(ROOT / "native/cli/tests/fake-runtime.mjs")
            env["DSH_TEST_RECORD"] = str(self.record)
            os.execve(BINARY, [str(BINARY), "--cwd", self.directory.name], env)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 100, 0, 0))
        self.until("● Ready")

    def tearDown(self):
        if self.pid:
            try:
                os.kill(self.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.waitpid(self.pid, 0)
        os.close(self.fd)
        self.directory.cleanup()

    def until(self, text, timeout=10, absent=False):
        deadline = time.monotonic() + timeout
        while True:
            if select.select([self.fd], [], [], .05)[0]:
                try:
                    chunk = os.read(self.fd, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        self.fail(f"Terminal exited before {text!r}: {self.screen.display!r}")
                    raise
                self.buffer += chunk
                self.stream.feed(self.decoder.decode(chunk))
            visible = "\n".join(self.screen.display)
            if (text in visible) != absent:
                return visible
            if time.monotonic() > deadline:
                self.fail(f"No {text!r} in terminal:\n{visible}")

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
        self.until("● Ready")
        self.send("/clear\r")
        self.until("Your project. Your next move.")
        self.send("\x1b[A\r")
        self.until("Answer: Grüße 世界")
        self.until("● Ready")
        self.finish()

    def test_typeahead_cannot_grant_permission(self):
        self.send("permission\r")
        self.until("Working before permission")
        self.send("yes\r")
        self.until("Waiting for your answer")
        self.send("no\r")
        self.until("Approval answer: no")
        self.until("● Ready")
        self.finish()

    def test_help_keeps_draft_and_multiline_paste_waits_for_enter(self):
        self.send("draft")
        self.until("draft")
        self.send("\x1bOP")
        self.until("Work from your terminal")
        self.send("\x1b")
        self.until("Work from your terminal", absent=True)
        self.assertIn("draft", "\n".join(self.screen.display))
        self.send("\x15\x1b[200~line one\nline two\x1b[201~")
        self.until("line two")
        self.assertNotIn("Answer: line one", "\n".join(self.screen.display))
        self.send("\r")
        self.until("Answer: line one")
        self.until("● Ready")
        self.finish()

    def test_command_completion_and_resize(self):
        self.send("/he\t\r")
        self.until("Work from your terminal")
        self.send("\x1b")
        self.until("Work from your terminal", absent=True)
        self.screen.resize(lines=18, columns=40)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 18, 40, 0, 0))
        os.kill(self.pid, signal.SIGWINCH)
        self.send("resized task\r")
        self.until("Answer: resized task")
        self.until("● Ready")
        self.finish()

    def test_ctrl_c_cancels_work_and_allows_followup(self):
        self.send("wait\r")
        self.until("Working until interrupt")
        self.send("\x03")
        self.until("Turn cancelled")
        self.until("● Ready")
        self.finish()


if __name__ == "__main__":
    unittest.main()

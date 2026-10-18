"""Record README media from the real DSH --demo terminal (POSIX, no model calls).

Install tools/requirements.txt, build native/cli in release mode, then run this
script from any directory. DejaVu Sans Mono must be installed, or supply --font
and --bold-font. Output contains sample data from the explicitly labeled demo.
"""
import argparse
import codecs
import errno
import fcntl
import math
import os
from pathlib import Path
import pty
import select
import signal
import struct
import termios
import time

import pyte
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[3]
COLS, ROWS = 100, 36
BACKGROUND = '#10151c'


class Recording:
    def __init__(self, binary, font, bold_font):
        self.font = ImageFont.truetype(str(font), 17)
        self.bold = ImageFont.truetype(str(bold_font), 17)
        self.cell = math.ceil(self.font.getlength('M'))
        self.line = 23
        self.screen = pyte.Screen(COLS, ROWS)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder('utf-8')('replace')
        self.frames, self.durations = [], []
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            env = os.environ.copy()
            for name in ('NO_COLOR', 'NODE_OPTIONS', 'DSH_RUNTIME_JS'):
                env.pop(name, None)
            env.update(TERM='xterm-256color', COLORTERM='truecolor')
            try:
                os.execve(binary, [str(binary), '--demo'], env)
            except OSError:
                os._exit(127)
        try:
            fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack('HHHH', ROWS, COLS, 0, 0))
        except OSError:
            self.close()
            raise

    def read(self, timeout):
        if not select.select([self.fd], [], [], timeout)[0]:
            return False
        try:
            data = os.read(self.fd, 65536)
        except OSError as error:
            if error.errno == errno.EIO:
                raise RuntimeError('Demo exited before the recording completed') from error
            raise
        if not data:
            raise RuntimeError('Demo closed its terminal before the recording completed')
        self.stream.feed(self.decoder.decode(data))
        return True

    def wait(self, text, absent=False):
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            self.read(.05)
            if (text in '\n'.join(self.screen.display)) != absent:
                # A PTY read may stop in the middle of one Ratatui screen draw.
                while self.read(.1):
                    if time.monotonic() >= deadline:
                        raise RuntimeError('Demo did not settle after rendering')
                return
        raise RuntimeError(f'Demo did not render {text!r}:\n' + '\n'.join(self.screen.display))

    def send(self, text):
        os.write(self.fd, text.encode())

    def image(self):
        pad, title_height = 24, 44
        image = Image.new('RGB', (COLS * self.cell + pad * 2, ROWS * self.line + pad * 2 + title_height), BACKGROUND)
        draw = ImageDraw.Draw(image)
        draw.rounded_rectangle((8, 8, image.width - 9, image.height - 9), radius=12, outline='#2c3948', width=1)
        draw.text((pad, 20), 'dsh  /  terminal preview', font=self.font, fill='#8597ad')
        draw.text((image.width - 222, 20), '100 cols  ×  36 rows', font=self.font, fill='#8597ad')
        draw.line((pad, 54, image.width - pad, 54), fill='#2c3948')
        origin_y = pad + title_height
        for y in range(ROWS):
            for x in range(COLS):
                cell = self.screen.buffer[y][x]
                bg = '#' + cell.bg if len(cell.bg) == 6 else BACKGROUND
                draw.rectangle((pad + x * self.cell, origin_y + y * self.line,
                                pad + (x + 1) * self.cell - 1, origin_y + (y + 1) * self.line - 1), fill=bg)
        for y in range(ROWS):
            for x in range(COLS):
                cell = self.screen.buffer[y][x]
                fg = '#' + cell.fg if len(cell.fg) == 6 else '#e4edf5'
                draw.text((pad + x * self.cell, origin_y + y * self.line), cell.data,
                          font=self.bold if cell.bold else self.font, fill=fg)
        if not self.screen.cursor.hidden:
            x, y = self.screen.cursor.x, self.screen.cursor.y
            draw.line((pad + x * self.cell, origin_y + y * self.line + 3,
                       pad + x * self.cell, origin_y + (y + 1) * self.line - 2), fill='#67b7ff', width=2)
        return image

    def hold(self, milliseconds):
        self.frames.append(self.image())
        self.durations.append(milliseconds)

    def close(self):
        try:
            self.send('\x04')
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline:
                pid, _ = os.waitpid(self.pid, os.WNOHANG)
                if pid:
                    self.pid = 0
                    return
                if select.select([self.fd], [], [], .05)[0]:
                    try:
                        os.read(self.fd, 65536)
                    except OSError as error:
                        if error.errno != errno.EIO:
                            raise
        finally:
            if self.pid:
                try:
                    os.kill(self.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                os.waitpid(self.pid, 0)
                self.pid = 0
            os.close(self.fd)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'native/cli/target/release/dsh')
    parser.add_argument('--output', type=Path, default=ROOT / 'docs/assets/cli')
    parser.add_argument('--font', type=Path, default=Path('/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf'))
    parser.add_argument('--bold-font', type=Path, default=Path('/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf'))
    args = parser.parse_args()
    if not args.binary.is_file():
        parser.error(f'Binary not found: {args.binary}. Build native/cli in release mode first.')
    args.output.mkdir(parents=True, exist_ok=True)
    recording = Recording(args.binary.resolve(), args.font, args.bold_font)
    try:
        recording.wait('● Ready')
        recording.image().save(args.output / 'welcome.png', optimize=True)
        recording.hold(1800)
        recording.send('/')
        recording.wait('Tab to complete')
        recording.hold(1600)
        recording.send('s')
        recording.wait('/session')
        recording.hold(250)
        recording.send('e')
        recording.wait('/se')
        recording.hold(500)
        recording.send('\t')
        recording.wait('/session')
        recording.hold(1000)
        recording.send('\x15')
        recording.wait('Tab to complete', absent=True)
        recording.send('2')
        recording.wait('Session restore is fixed')
        recording.image().save(args.output / 'conversation.png', optimize=True)
        recording.hold(2800)
        recording.send('\x14')
        recording.wait('12 passed; 0 failed')
        recording.hold(2200)
        recording.send('\x14')
        recording.wait('12 passed; 0 failed', absent=True)
        recording.send('3')
        recording.wait('Waiting for your answer')
        recording.image().save(args.output / 'approval.png', optimize=True)
        recording.hold(2400)
        recording.send('\x1bOP')
        recording.wait('Work from your terminal')
        recording.hold(2000)
        recording.send('\x1bOP')
        recording.wait('Work from your terminal', absent=True)
        recording.send('1')
        recording.wait('Your project. Your next move.')
        recording.hold(900)
    finally:
        recording.close()
    # One shared palette preserves the terminal colors between frames.
    palette = recording.frames[0].quantize(colors=128)
    frames = [frame.quantize(palette=palette, dither=Image.Dither.NONE) for frame in recording.frames]
    frames[0].save(args.output / 'dsh-demo.gif', save_all=True, append_images=frames[1:],
                   duration=recording.durations, loop=0, optimize=True, disposal=1)
    for path in sorted(args.output.iterdir()):
        if path.suffix in ('.png', '.gif'):
            print(f'{path.name}: {path.stat().st_size / 1024:.0f} KiB')


if __name__ == '__main__':
    main()

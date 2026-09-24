#!/usr/bin/env python3
"""Check that the machine resets ONCE and boots AGAIN (RFC-0.33-001 D15/D17).

Not part of `test-all`. The QEMU harness passes `-no-reboot` when it judges a
reset (`expect_shutdown`), so QEMU exits instead of rebooting and NO tier has
ever booted the machine a second time -- which is how a reset into a hung
machine (satp surviving the reset) went unseen until this was run by hand.

Runs the machine WITHOUT `-no-reboot`, optionally injects a console byte ONCE
after "driver-uart: ready", lets it run, and counts boots (one
"M3: endpoint created (id=0)" banner each).

  python3 tests/qemu/scripts/reset_boots_once.py 45 R    # expect boots=2
  python3 tests/qemu/scripts/reset_boots_once.py 25 -    # control: boots=1

Run from the repository root after `cargo xtask build`. Extra arguments are
passed to QEMU (e.g. `-d int,guest_errors -D q.log`). The serial output is left
in target/scratch/reset-boots-once.log.
"""
import os, subprocess, sys, threading, time

secs = float(sys.argv[1])
byte = sys.argv[2]
extra = sys.argv[3:]
cmd = ["qemu-system-riscv64", "-machine", "virt", "-bios", "none", "-nographic",
       "-kernel", "target/riscv64gc-unknown-none-elf/release/fjell-kernel",
       "-drive", "file=fjell-disk.img,format=raw,if=none,id=hd0",
       "-device", "virtio-blk-device,drive=hd0"] + extra
p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
buf = bytearray()
state = {"boots": 0, "injected": False}


def reader():
    line = bytearray()
    while True:
        b = p.stdout.read(1)
        if not b:
            return
        buf.extend(b)
        line.extend(b)
        if b == b"\n":
            t = bytes(line)
            line.clear()
            if b"M3: endpoint created (id=0)" in t:
                state["boots"] += 1
            if byte != "-" and not state["injected"] and b"driver-uart: ready" in t:
                state["injected"] = True
                p.stdin.write(byte.encode())
                p.stdin.flush()


threading.Thread(target=reader, daemon=True).start()
time.sleep(secs)
p.kill()
p.wait()
os.makedirs("target/scratch", exist_ok=True)
open("target/scratch/reset-boots-once.log", "wb").write(bytes(buf))
print(f"seconds={secs} injected={state['injected']} boots={state['boots']}")

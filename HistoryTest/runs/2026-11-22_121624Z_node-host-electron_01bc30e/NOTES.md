# Notes for 2026-11-22_121624Z_node-host-electron_01bc30e

## 2026-11-22 12:18 UTC

This is the control run: on the plain Node host the abi-probe extension is expected to fail (NODE_MODULE_VERSION 148 vs 137), and it did. The script exits 0 for that expected outcome; the UI smoke inside it shows 1 failed check (the error in the host log), which is that failure.

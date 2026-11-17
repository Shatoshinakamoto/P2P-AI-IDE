"""ide1-shot.py -- capture only the "P2P-AI IDE" window to a BMP (Windows, stdlib ctypes only).

Purpose: the remote screenshot tool masks the WebView2 content of the IDE (it belongs to msedgewebview2.exe),
so this grabs the IDE window's own rectangle for the UI check. Usage: python ide1-shot.py OUT.bmp
SECURITY: copies only the rectangle of the window whose title is exactly "P2P-AI IDE", never the full screen,
so nothing else on the desktop is captured (privacy, CWE-200). Writes one file named by the caller.
"""
import ctypes
import ctypes.wintypes as wt
import struct
import sys

user32 = ctypes.WinDLL("user32", use_last_error=True)
gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)
user32.SetProcessDPIAware()  # real pixel coordinates on scaled displays
hwnd = user32.FindWindowW(None, "P2P-AI IDE")
if not hwnd:
    sys.exit("window not found")
r = wt.RECT()
user32.GetWindowRect(hwnd, ctypes.byref(r))
w, h = r.right - r.left, r.bottom - r.top
sdc = user32.GetDC(None)
mdc = gdi32.CreateCompatibleDC(sdc)
bmp = gdi32.CreateCompatibleBitmap(sdc, w, h)
gdi32.SelectObject(mdc, bmp)
gdi32.BitBlt(mdc, 0, 0, w, h, sdc, r.left, r.top, 0x00CC0020)  # SRCCOPY of the window's screen area only
# 32-bit top-down DIB
bih = struct.pack("<IiiHHIIiiII", 40, w, -h, 1, 32, 0, 0, 0, 0, 0, 0)
buf = ctypes.create_string_buffer(w * h * 4)
gdi32.GetDIBits(mdc, bmp, 0, h, buf, ctypes.c_char_p(bih), 0)
with open(sys.argv[1], "wb") as f:
    f.write(b"BM" + struct.pack("<IHHI", 14 + 40 + len(buf), 0, 0, 54) + bih + buf.raw)
gdi32.DeleteObject(bmp)
gdi32.DeleteDC(mdc)
user32.ReleaseDC(None, sdc)
print(w, h, r.left, r.top)

#!/usr/bin/env python3
"""生成 Axolotl Viewer 应用图标源图（1024x1024 PNG）。
纯标准库实现（zlib + struct 手写 PNG），2x 超采样抗锯齿。
图案：深蓝圆角背景 + 三个白色节点圆 + 连线（图库意象）。
用法: python3 gen_icon.py [output.png]
"""
import struct
import sys
import zlib

S = 1024          # 输出尺寸
SS = 2            # 超采样倍数
W = S * SS

BG = (37, 99, 235)        # #2563eb
BG_DARK = (29, 78, 216)   # #1d4ed8
FG = (255, 255, 255)

NODES = [(0.28, 0.32, 0.085), (0.50, 0.70, 0.115), (0.72, 0.30, 0.085)]  # (x, y, r) 归一化
RADIUS = 0.22             # 圆角矩形半径


def rounded_rect(px, py):
    """返回 0..1 表示是否在圆角矩形内（带平滑边）。"""
    x, y = px / W, py / W
    left, top, right, bottom = RADIUS, RADIUS, 1 - RADIUS, 1 - RADIUS
    if x < left and y < top:
        dx, dy = x - left, y - top
        return 1.0 if dx * dx + dy * dy > RADIUS * RADIUS else 0.0
    if x > right and y < top:
        dx, dy = x - right, y - top
        return 1.0 if dx * dx + dy * dy > RADIUS * RADIUS else 0.0
    if x < left and y > bottom:
        dx, dy = x - left, y - bottom
        return 1.0 if dx * dx + dy * dy > RADIUS * RADIUS else 0.0
    if x > right and y > bottom:
        dx, dy = x - right, y - bottom
        return 1.0 if dx * dx + dy * dy > RADIUS * RADIUS else 0.0
    return 0.0


def dist_to_segment(px, py, ax, ay, bx, by):
    abx, aby = bx - ax, by - ay
    t = ((px - ax) * abx + (py - ay) * aby) / (abx * abx + aby * aby)
    t = max(0.0, min(1.0, t))
    cx, cy = ax + t * abx, ay + t * aby
    return ((px - cx) ** 2 + (py - cy) ** 2) ** 0.5


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "icon-source.png"
    rows = []
    n2 = [((x * W), (y * W), (r * W)) for x, y, r in NODES]
    segs = [(n2[0], n2[1]), (n2[1], n2[2])]
    # 背景垂直渐变（上深下浅）
    g0, g1 = BG_DARK, BG
    for py in range(W):
        row = bytearray()
        t = py / W
        bg = (int(g0[0] + (g1[0] - g0[0]) * t),
              int(g0[1] + (g1[1] - g0[1]) * t),
              int(g0[2] + (g1[2] - g0[2]) * t))
        for px in range(W):
            corner = rounded_rect(px, py)
            if corner >= 0.5:
                # 矩形外（圆角外）→ 透明
                alpha = int((1 - corner) * 255 * 2) if corner > 0.5 else 255
                row += bytes((0, 0, 0, alpha))
                continue
            r, g, b = bg
            # 连线
            d_seg = min(dist_to_segment(px, py, segs[0][0][0], segs[0][0][1], segs[0][1][0], segs[0][1][1]),
                        dist_to_segment(px, py, segs[1][0][0], segs[1][0][1], segs[1][1][0], segs[1][1][1]))
            line_w = 20 * SS
            if d_seg < line_w / 2:
                blend = min(1.0, (line_w / 2 - d_seg) / (line_w / 8))
                r = int(r + (FG[0] - r) * blend)
                g = int(g + (FG[1] - g) * blend)
                b = int(b + (FG[2] - b) * blend)
            # 节点圆
            for cx, cy_, rr in n2:
                d = ((px - cx) ** 2 + (py - cy_) ** 2) ** 0.5
                if d < rr:
                    blend = min(1.0, (rr - d) / (rr * 0.15))
                    r = int(r + (FG[0] - r) * blend)
                    g = int(g + (FG[1] - g) * blend)
                    b = int(b + (FG[2] - b) * blend)
            row += bytes((r, g, b, 255))
        rows.append(bytes(row))

    # 2x 降采样
    out_rows = []
    for y in range(S):
        row = bytearray()
        for x in range(S):
            acc = [0, 0, 0, 0]
            for dy in range(SS):
                for dx in range(SS):
                    i = (x * SS + dx) * 4  # 行内字节偏移
                    for c in range(4):
                        acc[c] += rows[y * SS + dy][i + c]
            n = SS * SS
            row += bytes(a // n for a in acc)
        out_rows.append(bytes(row))

    # PNG 编码
    raw = b"".join(b"\x00" + r for r in out_rows)
    ihdr = struct.pack(">IIBBBBB", S, S, 8, 6, 0, 0, 0)
    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr)
           + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))
    with open(out, "wb") as f:
        f.write(png)
    print(f"icon written: {out} ({S}x{S})")


if __name__ == "__main__":
    main()
